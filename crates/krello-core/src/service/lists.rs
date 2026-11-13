//! List CRUD with floating positions and WIP limits.



use super::{log_activity, next_list_position, now_updated, renormalize_lists, Service};
use crate::error::{Error, Result};
use crate::events::Event;
use crate::ids::new_id;
use crate::models::*;
use crate::time::now_rfc3339;
use crate::undo::*;
use rusqlite::{params, OptionalExtension};

impl Service {
    pub fn create_list(&self, board_id: &str, c: CreateList) -> Result<List> {
        if c.title.trim().is_empty() {
            return Err(Error::Validation("le titre de la liste est requis".into()));
        }
        let now = now_rfc3339();
        let list = List {
            id: new_id("list"),
            board_id: board_id.to_string(),
            title: c.title.trim().to_string(),
            position: c.position.unwrap_or(0.0), // fixed below
            wip_limit: c.wip_limit,
            archived: false,
            created_at: now.clone(),
            updated_at: now,
        };
        {
            let db = self.lock_db()?;
            if query_board(&db.conn, board_id)?.is_none() {
                return Err(Error::NotFound(format!("board {board_id}")));
            }
            let mut list = list;
            list.position = match c.position {
                Some(p) => p,
                None => next_list_position(&db.conn, board_id)?,
            };
            insert_list_row(&db.conn, &list)?;
            drop(db);
            self.push_undo(UndoEntry {
                label: format!("Ajout de la liste « {} »", list.title),
                actions: vec![UndoAction::ListCreated(list.clone())],
            });
            self.emit(Event::ListCreated {
                board_id: board_id.to_string(),
                list_id: list.id.clone(),
            });
            Ok(list)
        }
    }

    pub fn get_list(&self, id: &str) -> Result<List> {
        let db = self.lock_db()?;
        query_list(&db.conn, id)?.ok_or_else(|| Error::NotFound(format!("list {id}")))
    }

    pub fn list_lists(&self, board_id: &str, include_archived: bool) -> Result<Vec<List>> {
        let db = self.lock_db()?;
        let sql = if include_archived {
            "SELECT * FROM lists WHERE board_id=?1 ORDER BY position"
        } else {
            "SELECT * FROM lists WHERE board_id=?1 AND archived=0 ORDER BY position"
        };
        let mut stmt = db.conn.prepare_cached(sql)?;
        let lists = stmt
            .query_map([board_id], row_list)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(lists)
    }

    pub fn update_list(&self, id: &str, u: UpdateList) -> Result<List> {
        let db = self.lock_db()?;
        let before =
            query_list(&db.conn, id)?.ok_or_else(|| Error::NotFound(format!("list {id}")))?;
        let mut after = before.clone();
        if let Some(t) = &u.title {
            if t.trim().is_empty() {
                return Err(Error::Validation("le titre ne peut pas être vide".into()));
            }
            after.title = t.trim().to_string();
        }
        if u.wip_limit.is_some() {
            after.wip_limit = u.wip_limit;
        }
        if let Some(p) = u.position {
            after.position = p;
        }
        if let Some(a) = u.archived {
            after.archived = a;
        }
        after.updated_at = now_updated();
        update_list_row(&db.conn, &after)?;
        if u.position.is_some() {
            let board_id = after.board_id.clone();
            drop(db);
            let db = self.lock_db()?;
            renormalize_lists(&db.conn, &board_id)?;
            drop(db);
        } else {
            drop(db);
        }
        self.push_undo(UndoEntry {
            label: format!("Modification de la liste « {} »", after.title),
            actions: vec![UndoAction::ListUpdated {
                before,
                after: after.clone(),
            }],
        });
        self.emit(Event::ListUpdated {
            board_id: after.board_id.clone(),
            list_id: after.id.clone(),
        });
        Ok(after)
    }

    /// Move a list one slot left (dir = -1) or right (dir = +1) among the
    /// board's non-archived lists: swaps positions with the immediate
    /// neighbor. At the edges this is a no-op. Undoable (one entry).
    pub fn move_list(&self, id: &str, dir: i8) -> Result<List> {
        if dir != -1 && dir != 1 {
            return Err(Error::Validation(format!(
                "direction invalide: {dir} (attendus: -1 gauche, 1 droite)"
            )));
        }
        let db = self.lock_db()?;
        let list =
            query_list(&db.conn, id)?.ok_or_else(|| Error::NotFound(format!("list {id}")))?;
        let neighbor: Option<List> = if dir == -1 {
            db.conn
                .query_row(
                    "SELECT * FROM lists WHERE board_id=?1 AND archived=0 AND position < ?2
                     ORDER BY position DESC LIMIT 1",
                    params![list.board_id, list.position],
                    row_list,
                )
                .optional()?
        } else {
            db.conn
                .query_row(
                    "SELECT * FROM lists WHERE board_id=?1 AND archived=0 AND position > ?2
                     ORDER BY position ASC LIMIT 1",
                    params![list.board_id, list.position],
                    row_list,
                )
                .optional()?
        };
        let Some(mut neighbor) = neighbor else {
            return Ok(list); // bord: aucun voisin → no-op
        };
        let before = list.clone();
        let neighbor_before = neighbor.clone();
        let mut after = list;
        let neighbor_pos = neighbor_before.position;
        neighbor.position = after.position;
        after.position = neighbor_pos;
        update_list_row(&db.conn, &after)?;
        update_list_row(&db.conn, &neighbor)?;
        drop(db);
        self.push_undo(UndoEntry {
            label: format!("Déplacement de la liste « {} »", after.title),
            actions: vec![
                UndoAction::ListUpdated {
                    before,
                    after: after.clone(),
                },
                UndoAction::ListUpdated {
                    before: neighbor_before,
                    after: neighbor,
                },
            ],
        });
        self.emit(Event::ListUpdated {
            board_id: after.board_id.clone(),
            list_id: after.id.clone(),
        });
        Ok(after)
    }

    pub fn delete_list(&self, id: &str) -> Result<()> {
        let db = self.lock_db()?;
        let list =
            query_list(&db.conn, id)?.ok_or_else(|| Error::NotFound(format!("list {id}")))?;
        let board_id = list.board_id.clone();
        // Bulk: un jeu fixe de requêtes au lieu d'un snapshot par carte.
        let ids: Vec<String> = {
            let mut stmt = db.conn.prepare_cached("SELECT id FROM cards WHERE list_id=?1")?;
            let v = stmt
                .query_map([id], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<_>>()?;
            v
        };
        let cards = snapshot_cards_bulk(&db.conn, &ids)?;
        let entry = UndoEntry {
            label: format!("Suppression de la liste « {} »", list.title),
            actions: vec![UndoAction::ListDeleted { list, cards }],
        };
        db.conn.execute("DELETE FROM lists WHERE id=?1", [id])?;
        drop(db);
        self.push_undo(entry);
        self.emit(Event::ListDeleted {
            board_id,
            list_id: id.to_string(),
        });
        Ok(())
    }

    /// Deep-copy a list (title + "(copie)", all non-archived cards with labels,
    /// checklists, custom field values and attachments — attachment FILES are
    /// physically duplicated so deletion of one list never breaks the other).
    /// Dependencies are NOT copied. Undoable as one operation.
    pub fn duplicate_list(&self, id: &str) -> Result<List> {
        let mut guard = self.lock_db()?;
        let tx = guard.conn.transaction()?;
        let db = &*tx;
        let src =
            query_list(db, id)?.ok_or_else(|| Error::NotFound(format!("list {id}")))?;

        let new_list = List {
            id: new_id("list"),
            board_id: src.board_id.clone(),
            title: format!("{} (copie)", src.title),
            position: src.position + 0.5, // renormalisé juste après
            wip_limit: src.wip_limit,
            archived: false,
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
        };
        insert_list_row(db, &new_list)?;
        renormalize_lists(db, &src.board_id)?;

        // Snapshot bulk des cartes non-archivées (labels/checklists/champs/PJ inclus).
        let ids: Vec<String> = {
            let mut stmt =
                db.prepare_cached("SELECT id FROM cards WHERE list_id=?1 AND archived=0 ORDER BY position")?;
            let v = stmt
                .query_map([id], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<_>>()?;
            v
        };
        let snaps = snapshot_cards_bulk(db, &ids)?;

        let mut actions = vec![UndoAction::ListCreated(new_list.clone())];
        for (i, snap) in snaps.iter().enumerate() {
            let mut card = snap.card.clone();
            card.id = new_id("card");
            card.list_id = new_list.id.clone();
            card.position = i as f64;
            card.completed_at = None;
            card.reminders_sent = serde_json::Value::Null;
            card.created_at = now_rfc3339();
            card.updated_at = now_rfc3339();
            insert_card_row(db, &card)?;
            actions.push(UndoAction::CardCreated(card.clone()));

            for lid in &snap.label_ids {
                db.execute(
                    "INSERT OR IGNORE INTO card_labels(card_id,label_id) VALUES(?1,?2)",
                    params![&card.id, lid],
                )?;
            }
            for (cl, items) in &snap.checklists {
                let ncl = Checklist {
                    id: new_id("cl"),
                    card_id: card.id.clone(),
                    ..cl.clone()
                };
                insert_checklist_row(db, &ncl)?;
                for it in items {
                    insert_checklist_item_row(
                        db,
                        &ChecklistItem {
                            id: new_id("cli"),
                            checklist_id: ncl.id.clone(),
                            ..it.clone()
                        },
                    )?;
                }
            }
            for fv in &snap.field_values {
                db.execute(
                    "INSERT OR IGNORE INTO custom_field_values(card_id,field_id,value) VALUES(?1,?2,?3)",
                    params![&card.id, fv.field_id, fv.value],
                )?;
            }
            // Les fichiers de pièces jointes sont physiquement dupliqués: deux
            // lignes qui partageaient un même stored_path se briseraient l'une
            // l'autre à la suppression (best-effort si le fichier manque).
            for att in &snap.attachments {
                let new_att_id = new_id("att");
                let ext = std::path::Path::new(&att.stored_path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| format!(".{e}"))
                    .unwrap_or_default();
                let new_name = format!("{new_att_id}{ext}");
                let new_path = self.attachments_dir.join(&new_name);
                if std::fs::copy(&att.stored_path, &new_path).is_err() {
                    tracing::warn!(
                        "duplicate_list: fichier source manquant pour la PJ {} ({})",
                        att.id,
                        att.stored_path
                    );
                }
                let na = Attachment {
                    id: new_att_id,
                    card_id: card.id.clone(),
                    filename: att.filename.clone(),
                    stored_path: new_path.to_string_lossy().into_owned(),
                    mime: att.mime.clone(),
                    size: att.size,
                    is_cover: att.is_cover,
                    created_at: now_rfc3339(),
                };
                insert_attachment_row(db, &na)?;
            }
        }
        log_activity(
            db,
            &src.board_id,
            None,
            "list_duplicated",
            &serde_json::json!({ "source": src.id, "cards": snaps.len() }),
        )?;
        tx.commit()?;
        drop(guard);

        self.push_undo(UndoEntry {
            label: format!("Duplication de la liste « {} »", src.title),
            actions,
        });
        self.emit(Event::ListCreated {
            board_id: src.board_id.clone(),
            list_id: new_list.id.clone(),
        });
        // Le planning du board a changé (les copies ont des échéances).
        self.recompute_and_notify(&src.board_id)?;
        Ok(new_list)
    }

    /// Number of active (non-archived) cards in a list — used for WIP warnings.
    pub fn list_card_count(&self, list_id: &str) -> Result<i64> {
        let db = self.lock_db()?;
        let n: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM cards WHERE list_id=?1 AND archived=0 AND is_template=0",
            [list_id],
            |r| r.get(0),
        )?;
        Ok(n)
    }

    /// Does adding one more card exceed the WIP limit?
    pub fn wip_exceeded(&self, list_id: &str) -> Result<bool> {
        let db = self.lock_db()?;
        let limit: Option<i64> = db
            .conn
            .query_row(
                "SELECT wip_limit FROM lists WHERE id=?1",
                [list_id],
                |r| r.get::<_, Option<i64>>(0),
            )?;
        let Some(limit) = limit else {
            return Ok(false);
        };
        let n: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM cards WHERE list_id=?1 AND archived=0 AND is_template=0",
            [list_id],
            |r| r.get(0),
        )?;
        Ok(n > limit)
    }
}
