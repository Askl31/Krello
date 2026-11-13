//! Board CRUD, duplication, board activity feed.

use std::path::PathBuf;
use rusqlite::{params, OptionalExtension};

use super::{now_updated, Service};
use crate::error::{Error, Result};
use crate::events::Event;
use crate::ids::new_id;
use crate::models::*;
use crate::time::{now_rfc3339, parse_date};
use crate::undo::*;

pub const DEFAULT_BACKGROUND: &str = r##"{"type":"color","value":"#101828"}"##;

impl Service {
    pub fn create_board(&self, c: CreateBoard) -> Result<Board> {
        if c.title.trim().is_empty() {
            return Err(Error::Validation("le titre du board est requis".into()));
        }
        if let Some(d) = &c.start_date {
            parse_date(d).ok_or_else(|| Error::Validation(format!("start_date invalide: {d}")))?;
        }
        if let Some(d) = &c.deadline {
            parse_date(d).ok_or_else(|| Error::Validation(format!("deadline invalide: {d}")))?;
        }
        let now = now_rfc3339();
        let board = Board {
            id: new_id("board"),
            title: c.title.trim().to_string(),
            description: c.description,
            background: c.background.unwrap_or_else(|| DEFAULT_BACKGROUND.into()),
            calendar_id: Some(c.calendar_id.unwrap_or_else(|| "default".into())),
            start_date: c.start_date,
            deadline: c.deadline,
            settings: "{}".into(),
            archived: false,
            pinned: false,
            pinned_at: None,
            is_board_template: false,
            template_description: None,
            created_at: now.clone(),
            updated_at: now,
        };
        {
            let db = self.lock_db()?;
            insert_board_row(&db.conn, &board)?;
        }
        self.push_undo(UndoEntry {
            label: format!("Création du board « {} »", board.title),
            actions: vec![UndoAction::BoardCreated(board.clone())],
        });
        self.emit(Event::BoardCreated {
            board_id: board.id.clone(),
        });
        Ok(board)
    }

    pub fn get_board(&self, id: &str) -> Result<Board> {
        let db = self.lock_db()?;
        query_board(&db.conn, id)?.ok_or_else(|| Error::NotFound(format!("board {id}")))
    }

    pub fn list_boards(&self, include_archived: bool) -> Result<Vec<Board>> {
        let db = self.lock_db()?;
        let sql = if include_archived {
            "SELECT * FROM boards WHERE is_board_template = 0 ORDER BY pinned DESC, pinned_at DESC, created_at"
        } else {
            "SELECT * FROM boards WHERE archived = 0 AND is_board_template = 0 ORDER BY pinned DESC, pinned_at DESC, created_at"
        };
        let mut stmt = db.conn.prepare_cached(sql)?;
        let boards = stmt
            .query_map([], row_board)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(boards)
    }

    /// Number of non-archived boards — cheap COUNT for /health (no full load).
    pub fn count_boards(&self) -> Result<i64> {
        let db = self.lock_db()?;
        Ok(db.conn.query_row("SELECT COUNT(*) FROM boards WHERE archived=0", [], |r| r.get(0))?)
    }

    /// Update board partial (title, description, background, calendar, dates…).
    pub fn update_board(&self, id: &str, u: UpdateBoard) -> Result<Board> {
        if let Some(t) = &u.title {
            if t.trim().is_empty() {
                return Err(Error::Validation("le titre ne peut pas être vide".into()));
            }
        }
        if let Some(d) = &u.start_date {
            if parse_date(d).is_none() {
                return Err(Error::Validation(format!("start_date invalide: {d}")));
            }
        }
        if let Some(d) = &u.deadline {
            if parse_date(d).is_none() {
                return Err(Error::Validation(format!("deadline invalide: {d}")));
            }
        }
        let db = self.lock_db()?;
        let before = query_board(&db.conn, id)?
            .ok_or_else(|| Error::NotFound(format!("board {id}")))?;
        if let Some(cal) = &u.calendar_id {
            let exists: Option<String> = db
                .conn
                .query_row("SELECT id FROM calendars WHERE id=?1", [cal], |r| r.get(0))
                .optional()?;
            if exists.is_none() {
                return Err(Error::Validation(format!("calendrier inconnu: {cal}")));
            }
        }
        let mut after = before.clone();
        if let Some(v) = u.title {
            after.title = v.trim().to_string();
        }
        if let Some(v) = u.description {
            after.description = v;
        }
        if let Some(v) = u.background {
            after.background = v;
        }
        if let Some(v) = u.calendar_id {
            after.calendar_id = Some(v);
        }
        if let Some(v) = u.start_date {
            after.start_date = Some(v);
        }
        if let Some(v) = u.deadline {
            after.deadline = Some(v);
        }
        if let Some(v) = u.settings {
            serde_json::from_str::<serde_json::Value>(&v)
                .map_err(|e| Error::Validation(format!("settings JSON invalide: {e}")))?;
            after.settings = v;
        }
        if let Some(v) = u.archived {
            after.archived = v;
        }
        if let Some(v) = u.pinned {
            after.pinned = v;
            after.pinned_at = if v { Some(now_rfc3339()) } else { None };
        }
        after.updated_at = now_updated();
        update_board_row(&db.conn, &after)?;
        let schedule_changed = before.start_date != after.start_date
            || before.deadline != after.deadline
            || before.calendar_id != after.calendar_id;
        let tx_undo = UndoEntry {
            label: format!("Modification du board « {} »", after.title),
            actions: vec![UndoAction::BoardUpdated {
                before,
                after: after.clone(),
            }],
        };
        if schedule_changed {
            drop(db);
            self.recompute_and_notify(id)?;
        }
        self.push_undo(tx_undo);
        self.emit(Event::BoardUpdated {
            board_id: id.to_string(),
        });
        Ok(after)
    }

    /// Store a board background IMAGE: validated (mime + extension), written as
    /// `<backgrounds>/<id><ext>`. Returns the background JSON to store on the
    /// board (`{"type":"image","value":"<filename>"}`) — undo goes through the
    /// regular BoardUpdated entry.
    pub fn set_board_background_image(
        &self,
        board_id: &str,
        filename: &str,
        mime: &str,
        data: &[u8],
    ) -> Result<String> {
        const MAX_BACKGROUND_BYTES: i64 = 8 * 1024 * 1024;
        const ALLOWED: [(&str, &str); 6] = [
            ("image/png", ".png"),
            ("image/jpeg", ".jpg"),
            ("image/webp", ".webp"),
            ("image/gif", ".gif"),
            ("image/avif", ".avif"),
            ("image/svg+xml", ".svg"),
        ];
        if data.len() as i64 > MAX_BACKGROUND_BYTES {
            return Err(Error::Validation(format!(
                "image trop volumineuse (max {} Mo)",
                MAX_BACKGROUND_BYTES / 1024 / 1024
            )));
        }
        let Some((_, ext)) = ALLOWED.iter().find(|(m, _)| *m == mime) else {
            return Err(Error::Validation(format!(
                "type d'image non supporté: {mime} (attendus: png, jpeg, webp, gif, avif, svg)"
            )));
        };
        let lower = filename.to_lowercase();
        let looks_image = ALLOWED.iter().any(|(_, e)| lower.ends_with(e));
        if !looks_image {
            return Err(Error::Validation("nom de fichier image attendu".into()));
        }

        let db = self.lock_db()?;
        query_board(&db.conn, board_id)?
            .ok_or_else(|| Error::NotFound(format!("board {board_id}")))?;
        std::fs::create_dir_all(&self.backgrounds_dir)?;
        let bg_id = new_id("bg");
        let stored = self.backgrounds_dir.join(format!("{bg_id}{ext}"));
        std::fs::write(&stored, data)?;
        drop(db);
        Ok(serde_json::json!({ "type": "image", "value": format!("{bg_id}{ext}") }).to_string())
    }

    /// Resolve a stored background filename to a canonical path INSIDE the
    /// backgrounds dir (path-traversal guard).
    pub fn background_path_checked(&self, stored_name: &str) -> Result<PathBuf> {
        // Seuls les noms générés par le serveur sont servis (id + extension).
        let ok = stored_name.starts_with("bg_")
            && !stored_name.contains(['/', '\\', '\0'])
            && std::path::Path::new(stored_name)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| [".png", ".jpg", ".webp", ".gif", ".avif", ".svg"].contains(&format!(".{e}").as_str()))
                .unwrap_or(false);
        if !ok {
            return Err(Error::Validation("nom de fond de board invalide".into()));
        }
        let dir = self.backgrounds_dir.canonicalize()?;
        let canonical = std::path::Path::new(stored_name)
            .canonicalize()
            .map_err(|_| Error::NotFound("fond de board".into()))?;
        if !canonical.starts_with(&dir) {
            return Err(Error::Validation(
                "chemin hors répertoire des fonds de board".into(),
            ));
        }
        Ok(canonical)
    }

    pub fn delete_board(&self, id: &str) -> Result<()> {
        let n = self.delete_boards_inner(&[id.to_string()])?;
        if n == 0 {
            return Err(Error::NotFound(format!("board {id}")));
        }
        Ok(())
    }

    /// Delete ALL boards (including archived), purging attachment files and run logs.
    /// Single undo entry containing every BoardDeleted snapshot. Returns the count.
    pub fn delete_all_boards(&self) -> Result<usize> {
        let boards = self.list_boards(true)?;
        let ids: Vec<String> = boards.iter().map(|b| b.id.clone()).collect();
        self.delete_boards_inner(&ids)
    }

    fn delete_boards_inner(&self, ids: &[String]) -> Result<usize> {
        let db = self.lock_db()?;
        let mut actions: Vec<UndoAction> = Vec::new();
        let mut deleted: Vec<String> = Vec::new();
        for id in ids {
            let Some(board) = query_board(&db.conn, id)? else {
                continue;
            };
            let lists: Vec<List> = {
                let mut stmt =
                    db.conn.prepare_cached("SELECT * FROM lists WHERE board_id=?1 ORDER BY position")?;
                let v = stmt
                    .query_map([id], row_list)?
                    .collect::<rusqlite::Result<_>>()?;
                v
            };
            // Bulk: un jeu fixe de requêtes au lieu d'un snapshot par carte.
            let cids: Vec<String> = {
                let mut stmt = db.conn.prepare_cached("SELECT id FROM cards WHERE board_id=?1")?;
                let v = stmt
                    .query_map([id], |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<_>>()?;
                v
            };
            let cards = snapshot_cards_bulk(&db.conn, &cids)?;
            // purge des fichiers pièces jointes du board
            let mut st = db.conn.prepare_cached("SELECT stored_path FROM attachments WHERE card_id IN (SELECT id FROM cards WHERE board_id=?1)")?;
            let paths: Vec<String> = st
                .query_map([id], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<_>>()?;
            drop(st);
            for p in paths {
                let _ = std::fs::remove_file(p);
            }
            db.conn.execute("DELETE FROM boards WHERE id=?1", [id])?;
            db.conn.execute("DELETE FROM automation_runs WHERE board_id=?1", [id])?;
            actions.push(UndoAction::BoardDeleted {
                board: board.clone(),
                lists,
                cards,
            });
            deleted.push(id.clone());
        }
        drop(db);
        if actions.is_empty() {
            return Ok(0);
        }
        let label = if actions.len() == 1 {
            match &actions[0] {
                UndoAction::BoardDeleted { board, .. } => {
                    format!("Suppression du board « {} »", board.title)
                }
                _ => "Suppression".into(),
            }
        } else {
            format!("Suppression de {} boards", actions.len())
        };
        self.push_undo(UndoEntry { label, actions });
        for id in &deleted {
            self.emit(Event::BoardDeleted {
                board_id: id.clone(),
            });
        }
        Ok(deleted.len())
    }

    /// Activity feed for ONE card (newest first) — used by the card modal.
    pub fn card_activity(&self, board_id: &str, card_id: &str, limit: i64) -> Result<Vec<Activity>> {
        let db = self.lock_db()?;
        let mut stmt = db.conn.prepare_cached("SELECT * FROM activity WHERE board_id=?1 AND card_id=?2 ORDER BY created_at DESC LIMIT ?3")?;
        let acts = stmt
            .query_map(
                params![board_id, card_id, limit.clamp(1, 500)],
                row_activity,
            )?
            .collect::<rusqlite::Result<_>>()?;
        Ok(acts)
    }

    /// Deep-copy a board (labels, lists, non-archived cards with children,
    /// custom fields, dependencies, automations). Returns the new board.
    /// `title_override` renames the copy; `keep_template` preserves the
    /// template flag (duplicating a template) — instantiation passes false.
    pub fn duplicate_board_opts(
        &self,
        id: &str,
        title_override: Option<String>,
        keep_template: bool,
    ) -> Result<Board> {
        let mut guard = self.lock_db()?;
        let tx = guard.conn.transaction()?;
        let db = &*tx;
        let src = query_board(db, id)?
            .ok_or_else(|| Error::NotFound(format!("board {id}")))?;
        let now = now_rfc3339();
        let mut new_board = src.clone();
        new_board.id = new_id("board");
        new_board.title =
            title_override.unwrap_or_else(|| format!("{} (copie)", src.title));
        // La copie n'hérite pas du statut épinglé.
        new_board.pinned = false;
        new_board.pinned_at = None;
        if !keep_template {
            new_board.is_board_template = false;
            new_board.template_description = None;
        }
        new_board.created_at = now.clone();
        new_board.updated_at = now;
        insert_board_row(db, &new_board)?;

        let mut label_map: std::collections::HashMap<String, String> = Default::default();
        {
            let mut stmt = db.prepare_cached("SELECT * FROM labels WHERE board_id=?1")?;
            let labels: Vec<Label> = stmt
                .query_map([id], row_label)?
                .collect::<rusqlite::Result<_>>()?;
            for l in labels {
                let nid = new_id("label");
                label_map.insert(l.id.clone(), nid.clone());
                insert_label_row(
                    db,
                    &Label {
                        id: nid,
                        board_id: new_board.id.clone(),
                        ..l
                    },
                )?;
            }
        }
        let mut list_map: std::collections::HashMap<String, String> = Default::default();
        {
            let mut stmt =
                db.prepare_cached("SELECT * FROM lists WHERE board_id=?1 AND archived=0 ORDER BY position")?;
            let lists: Vec<List> = stmt
                .query_map([id], row_list)?
                .collect::<rusqlite::Result<_>>()?;
            for l in lists {
                let nid = new_id("list");
                list_map.insert(l.id.clone(), nid.clone());
                insert_list_row(
                    db,
                    &List {
                        id: nid,
                        board_id: new_board.id.clone(),
                        ..l
                    },
                )?;
            }
        }
        let mut card_map: std::collections::HashMap<String, String> = Default::default();
        let cards: Vec<Card> = {
            let mut stmt = db.prepare_cached("SELECT * FROM cards WHERE board_id=?1 AND archived=0 ORDER BY position")?;
            let v = stmt
                .query_map([id], row_card)?
                .collect::<rusqlite::Result<_>>()?;
            v
        };
        for c in &cards {
            let nid = new_id("card");
            card_map.insert(c.id.clone(), nid.clone());
            let mut nc = c.clone();
            nc.id = nid;
            nc.board_id = new_board.id.clone();
            nc.list_id = list_map
                .get(&c.list_id)
                .cloned()
                .unwrap_or_else(|| c.list_id.clone());
            insert_card_row(db, &nc)?;
        }
        // labels, checklists, fields — réutilise `cards` déjà chargé (pas de re-SELECT)
        for c in &cards {
            let new_cid = &card_map[&c.id];
            let lids = label_ids_for_card(db, &c.id)?;
            for lid in lids {
                if let Some(nl) = label_map.get(&lid) {
                    db.execute(
                        "INSERT OR IGNORE INTO card_labels(card_id,label_id) VALUES(?1,?2)",
                        params![new_cid, nl],
                    )?;
                }
            }
            let mut stmt = db.prepare_cached("SELECT * FROM checklists WHERE card_id=?1 ORDER BY position")?;
            let cls: Vec<Checklist> = stmt
                .query_map([&c.id], row_checklist)?
                .collect::<rusqlite::Result<_>>()?;
            drop(stmt);
            for cl in cls {
                let ncl_id = new_id("cl");
                insert_checklist_row(
                    db,
                    &Checklist {
                        id: ncl_id.clone(),
                        card_id: new_cid.clone(),
                        ..cl.clone()
                    },
                )?;
                let mut stmt = db.prepare_cached("SELECT * FROM checklist_items WHERE checklist_id=?1 ORDER BY position")?;
                let items: Vec<ChecklistItem> = stmt
                    .query_map([&cl.id], row_checklist_item)?
                    .collect::<rusqlite::Result<_>>()?;
                drop(stmt);
                for it in items {
                    insert_checklist_item_row(
                        db,
                        &ChecklistItem {
                            id: new_id("cli"),
                            checklist_id: ncl_id.clone(),
                            ..it
                        },
                    )?;
                }
            }
            let mut stmt = db.prepare_cached("SELECT card_id, field_id, value FROM custom_field_values WHERE card_id=?1")?;
            let fvs: Vec<CustomFieldValue> = stmt
                .query_map([&c.id], row_field_value)?
                .collect::<rusqlite::Result<_>>()?;
            drop(stmt);
            for fv in fvs {
                db.execute(
                    "INSERT OR IGNORE INTO custom_field_values(card_id,field_id,value) VALUES(?1,?2,?3)",
                    params![new_cid, fv.field_id, fv.value],
                )?;
            }
        }
        // custom field defs belong to the board: copy as-is (same ids keep values linked)
        db.execute(
            "INSERT INTO custom_field_defs(id,board_id,name,type,options,position,created_at)
             SELECT id, ?2, name, type, options, position, created_at FROM custom_field_defs WHERE board_id=?1",
            params![id, new_board.id],
        )?;
        // dependencies between copied cards
        {
            let mut stmt = db.prepare_cached("SELECT * FROM dependencies WHERE board_id=?1")?;
            let deps: Vec<Dependency> = stmt
                .query_map([id], row_dependency)?
                .collect::<rusqlite::Result<_>>()?;
            drop(stmt);
            for d in deps {
                let (Some(p), Some(s)) = (card_map.get(&d.predecessor_id), card_map.get(&d.successor_id))
                else {
                    continue;
                };
                insert_dependency_row(
                    db,
                    &Dependency {
                        id: new_id("dep"),
                        board_id: new_board.id.clone(),
                        predecessor_id: p.clone(),
                        successor_id: s.clone(),
                        ..d
                    },
                )?;
            }
        }
        // automations
        db.execute(
            "INSERT INTO automations(id,board_id,name,enabled,trigger,conditions,actions,created_at)
             SELECT ?2 || '__' || id, ?2, name, enabled, trigger, conditions, actions, ?3
             FROM automations WHERE board_id=?1",
            params![id, new_board.id, now_rfc3339()],
        )?;
        tx.commit()?;
        drop(guard);
        crate::scheduler::recompute_service(self, &new_board.id)?;
        self.push_undo(UndoEntry {
            label: if keep_template {
                format!("Duplication du board « {} »", src.title)
            } else {
                format!("Instanciation du modèle « {} »", src.title)
            },
            actions: vec![UndoAction::BoardCreated(new_board.clone())],
        });
        self.emit(Event::BoardCreated {
            board_id: new_board.id.clone(),
        });
        Ok(new_board)
    }

    /// Duplicating keeps the template flag (gallery variant management).
    pub fn duplicate_board(&self, id: &str) -> Result<Board> {
        self.duplicate_board_opts(id, None, true)
    }

    /// Saved board templates (gallery), oldest first.
    pub fn list_board_templates(&self) -> Result<Vec<Board>> {
        let db = self.lock_db()?;
        let mut stmt = db.conn.prepare_cached(
            "SELECT * FROM boards WHERE is_board_template=1 AND archived=0 ORDER BY created_at",
        )?;
        let boards = stmt.query_map([], row_board)?.collect::<rusqlite::Result<_>>()?;
        Ok(boards)
    }

    /// Toggle the template flag on a board ("Enregistrer comme modèle").
    pub fn set_board_template(&self, id: &str, s: SetBoardTemplate) -> Result<Board> {
        let db = self.lock_db()?;
        let before =
            query_board(&db.conn, id)?.ok_or_else(|| Error::NotFound(format!("board {id}")))?;
        let mut after = before.clone();
        after.is_board_template = s.is_template;
        after.template_description = if s.is_template {
            s.template_description.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
        } else {
            None
        };
        after.updated_at = now_updated();
        update_board_row(&db.conn, &after)?;
        self.push_undo(UndoEntry {
            label: if after.is_board_template {
                format!("Conversion du board « {} » en modèle", after.title)
            } else {
                format!("Reconversion du board « {} » en tableau", after.title)
            },
            actions: vec![UndoAction::BoardUpdated {
                before,
                after: after.clone(),
            }],
        });
        self.emit(Event::BoardUpdated {
            board_id: id.to_string(),
        });
        Ok(after)
    }

    /// Instantiate a saved board template: deep copy, renamed (template title
    /// by default), and never a template itself.
    pub fn instantiate_board_template(&self, id: &str, title: Option<String>) -> Result<Board> {
        let src = self.get_board(id)?;
        if !src.is_board_template {
            return Err(Error::Validation(format!(
                "le board « {} » n'est pas un modèle",
                src.title
            )));
        }
        let title = title
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .or_else(|| Some(src.title.clone()));
        self.duplicate_board_opts(id, title, false)
    }

    /// Recent activity entries for a board (global feed, newest first).
    pub fn board_activity(&self, board_id: &str, limit: i64) -> Result<Vec<Activity>> {
        let db = self.lock_db()?;
        let mut stmt = db.conn.prepare_cached("SELECT * FROM activity WHERE board_id=?1 ORDER BY created_at DESC LIMIT ?2")?;
        let acts = stmt
            .query_map(params![board_id, limit.clamp(1, 500)], row_activity)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(acts)
    }

    /// Cross-board overview: totals, per-board rollup and recent activity.
    pub fn global_stats(&self) -> Result<serde_json::Value> {
        let db = self.lock_db()?;
        let today_nd = crate::time::today();
        let today = today_nd.to_string();
        let in7 = crate::time::add_days(today_nd, 7).to_string();
        let in7ago = crate::time::add_days(today_nd, -7).to_string();

        let (boards, cards_active, overdue, due_7d, done_7d) = db.conn.query_row(
            "SELECT
               (SELECT COUNT(*) FROM boards WHERE archived=0 AND is_board_template=0),
               (SELECT COUNT(*) FROM cards WHERE archived=0 AND is_template=0),
               (SELECT COUNT(*) FROM cards WHERE archived=0 AND is_template=0 AND due_complete=0
                  AND due IS NOT NULL AND substr(due,1,10) < ?1),
               (SELECT COUNT(*) FROM cards WHERE archived=0 AND is_template=0 AND due_complete=0
                  AND due IS NOT NULL AND substr(due,1,10) >= ?1 AND substr(due,1,10) <= ?2),
               (SELECT COUNT(*) FROM cards WHERE completed_at IS NOT NULL
                  AND substr(completed_at,1,10) >= ?3)",
            params![today, in7, in7ago],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            },
        )?;

        let mut stmt = db.conn.prepare_cached(
            "SELECT b.id, b.title,
               SUM(CASE WHEN c.id IS NOT NULL THEN 1 ELSE 0 END) AS active,
               SUM(CASE WHEN c.due IS NOT NULL AND c.due_complete=0
                         AND substr(c.due,1,10) < ?1 THEN 1 ELSE 0 END) AS overdue
             FROM boards b
             LEFT JOIN cards c ON c.board_id = b.id AND c.archived=0 AND c.is_template=0
             WHERE b.archived=0 AND b.is_board_template=0
             GROUP BY b.id
             ORDER BY b.pinned DESC, b.pinned_at DESC, b.created_at",
        )?;
        let per_board: Vec<serde_json::Value> = stmt
            .query_map(params![today], |r| {
                Ok(serde_json::json!({
                    "id": r.get::<_, String>(0)?,
                    "title": r.get::<_, String>(1)?,
                    "active": r.get::<_, i64>(2)?,
                    "overdue": r.get::<_, i64>(3)?,
                }))
            })?
            .collect::<rusqlite::Result<_>>()?;
        drop(stmt);

        let mut stmt = db
            .conn
            .prepare_cached("SELECT * FROM activity ORDER BY created_at DESC LIMIT 20")?;
        let activity: Vec<Activity> = stmt
            .query_map([], row_activity)?
            .collect::<rusqlite::Result<_>>()?;
        drop(stmt);

        // Cartes urgentes actives (priorité urgent/high) des boards visibles.
        let urgent_filter = "FROM cards c JOIN boards b ON b.id = c.board_id
             WHERE c.archived=0 AND c.is_template=0 AND c.due_complete=0
               AND c.priority IN ('urgent','high')
               AND b.archived=0 AND b.is_board_template=0";
        let mut stmt = db.conn.prepare_cached(&format!(
            "SELECT c.* {urgent_filter}
             ORDER BY CASE c.priority WHEN 'urgent' THEN 0 ELSE 1 END, c.due IS NULL, c.due
             LIMIT 12"
        ))?;
        let urgent: Vec<Card> = stmt.query_map([], row_card)?.collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        let urgent_total: i64 = db
            .conn
            .query_row(&format!("SELECT COUNT(*) {urgent_filter}"), [], |r| r.get(0))?;

        // Échéances proches : aujourd'hui ≤ due ≤ aujourd'hui+2 (date-only).
        let in2 = crate::time::add_days(today_nd, 2).to_string();
        let due_filter = "FROM cards c JOIN boards b ON b.id = c.board_id
             WHERE c.archived=0 AND c.is_template=0 AND c.due_complete=0
               AND c.due IS NOT NULL AND substr(c.due,1,10) >= ?1 AND substr(c.due,1,10) <= ?2
               AND b.archived=0 AND b.is_board_template=0";
        let mut stmt = db.conn.prepare_cached(&format!(
            "SELECT c.* {due_filter} ORDER BY c.due LIMIT 12"
        ))?;
        let due_soon: Vec<Card> =
            stmt.query_map(params![today, in2], row_card)?.collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        let due_soon_total: i64 = db
            .conn
            .query_row(&format!("SELECT COUNT(*) {due_filter}"), params![today, in2], |r| r.get(0))?;

        Ok(serde_json::json!({
            "boards": boards,
            "cards_active": cards_active,
            "overdue": overdue,
            "due_7d": due_7d,
            "done_7d": done_7d,
            "per_board": per_board,
            "activity": activity,
            "urgent": urgent,
            "urgent_total": urgent_total,
            "due_soon": due_soon,
            "due_soon_total": due_soon_total,
        }))
    }

    pub(crate) fn recompute_and_notify(&self, board_id: &str) -> Result<()> {
        crate::scheduler::recompute_service(self, board_id)?;
        self.emit(Event::ScheduleUpdated {
            board_id: board_id.to_string(),
        });
        Ok(())
    }
}
