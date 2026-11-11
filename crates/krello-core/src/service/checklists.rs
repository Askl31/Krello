//! Checklists and items with ordered positions.

use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};

use super::{Service};
use crate::error::{Error, Result};
use crate::events::Event;
use crate::ids::new_id;
use crate::models::*;
use crate::time::now_rfc3339;
use crate::undo::*;

impl Service {
    pub fn create_checklist(&self, card_id: &str, c: CreateChecklist) -> Result<Checklist> {
        let db = self.lock_db()?;
        let card =
            query_card(&db.conn, card_id)?.ok_or_else(|| Error::NotFound(format!("card {card_id}")))?;
        let pos: f64 = db.conn.query_row(
            "SELECT COALESCE(MAX(position), -1.0) + 1.0 FROM checklists WHERE card_id=?1",
            [card_id],
            |r| r.get(0),
        )?;
        let cl = Checklist {
            id: new_id("cl"),
            card_id: card_id.to_string(),
            title: c.title.trim().to_empty_checked()?,
            position: c.position.unwrap_or(pos),
            created_at: now_rfc3339(),
        };
        insert_checklist_row(&db.conn, &cl)?;
        drop(db);
        self.push_undo(UndoEntry {
            label: format!("Ajout de la checklist « {} »", cl.title),
            actions: vec![UndoAction::ChecklistCreated(cl.clone())],
        });
        self.emit(Event::ChecklistsUpdated {
            board_id: card.board_id,
            card_id: card_id.to_string(),
        });
        Ok(cl)
    }

    pub fn delete_checklist(&self, id: &str) -> Result<()> {
        let db = self.lock_db()?;
        let cl = db
            .conn
            .query_row("SELECT * FROM checklists WHERE id=?1", [id], row_checklist)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("checklist {id}")))?;
        let mut stmt =
            db.conn.prepare("SELECT * FROM checklist_items WHERE checklist_id=?1 ORDER BY position")?;
        let items: Vec<ChecklistItem> = stmt
            .query_map([id], row_checklist_item)?
            .collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        let card =
            query_card(&db.conn, &cl.card_id)?.ok_or_else(|| Error::NotFound("card".into()))?;
        db.conn.execute("DELETE FROM checklists WHERE id=?1", [id])?;
        drop(db);
        self.push_undo(UndoEntry {
            label: format!("Suppression de la checklist « {} »", cl.title),
            actions: vec![UndoAction::ChecklistDeleted { checklist: cl, items }],
        });
        self.emit(Event::ChecklistsUpdated {
            board_id: card.board_id,
            card_id: card.id,
        });
        Ok(())
    }

    pub fn list_checklists(&self, card_id: &str) -> Result<Vec<(Checklist, Vec<ChecklistItem>)>> {
        let db = self.lock_db()?;
        let mut stmt =
            db.conn.prepare("SELECT * FROM checklists WHERE card_id=?1 ORDER BY position")?;
        let cls: Vec<Checklist> = stmt
            .query_map([card_id], row_checklist)?
            .collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        let ids: Vec<String> = cls.iter().map(|c| c.id.clone()).collect();
        let mut items_by_cl = items_for_checklists(&db.conn, &ids)?;
        let out = cls
            .into_iter()
            .map(|cl| {
                let items = items_by_cl.remove(&cl.id).unwrap_or_default();
                (cl, items)
            })
            .collect();
        Ok(out)
    }

    pub fn create_checklist_item(&self, checklist_id: &str, c: CreateChecklistItem) -> Result<ChecklistItem> {
        let db = self.lock_db()?;
        let cl = db
            .conn
            .query_row("SELECT * FROM checklists WHERE id=?1", [checklist_id], row_checklist)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("checklist {checklist_id}")))?;
        if c.title.trim().is_empty() {
            return Err(Error::Validation("titre d'élément requis".into()));
        }
        let pos: f64 = db.conn.query_row(
            "SELECT COALESCE(MAX(position), -1.0) + 1.0 FROM checklist_items WHERE checklist_id=?1",
            [checklist_id],
            |r| r.get(0),
        )?;
        let item = ChecklistItem {
            id: new_id("cli"),
            checklist_id: checklist_id.to_string(),
            title: c.title.trim().to_string(),
            checked: false,
            position: c.position.unwrap_or(pos),
            due: c.due,
            created_at: now_rfc3339(),
        };
        insert_checklist_item_row(&db.conn, &item)?;
        let card = query_card(&db.conn, &cl.card_id)?
            .ok_or_else(|| Error::NotFound("card".into()))?;
        drop(db);
        self.push_undo(UndoEntry {
            label: "Ajout d'un élément de checklist".into(),
            actions: vec![UndoAction::ChecklistItemCreated(item.clone())],
        });
        self.emit(Event::ChecklistsUpdated {
            board_id: card.board_id,
            card_id: card.id,
        });
        Ok(item)
    }

    pub fn update_checklist_item(&self, id: &str, u: UpdateChecklistItem) -> Result<ChecklistItem> {
        let db = self.lock_db()?;
        let before = db
            .conn
            .query_row("SELECT * FROM checklist_items WHERE id=?1", [id], row_checklist_item)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("item {id}")))?;
        let mut after = before.clone();
        if let Some(t) = &u.title {
            if t.trim().is_empty() {
                return Err(Error::Validation("titre d'élément requis".into()));
            }
            after.title = t.trim().to_string();
        }
        if let Some(v) = u.checked {
            after.checked = v;
        }
        if let Some(p) = u.position {
            after.position = p;
        }
        if u.due.is_some() {
            after.due = u.due;
        }
        update_checklist_item_row(&db.conn, &after)?;
        let checked_now = !before.checked && after.checked;
        let cl = db
            .conn
            .query_row("SELECT * FROM checklists WHERE id=?1", [&after.checklist_id], row_checklist)
            .optional()?
            .ok_or_else(|| Error::NotFound("checklist".into()))?;
        let card = query_card(&db.conn, &cl.card_id)?
            .ok_or_else(|| Error::NotFound("card".into()))?;
        drop(db);
        self.push_undo(UndoEntry {
            label: "Modification d'un élément".into(),
            actions: vec![UndoAction::ChecklistItemUpdated { before, after: after.clone() }],
        });
        self.emit(Event::ChecklistsUpdated {
            board_id: card.board_id.clone(),
            card_id: card.id.clone(),
        });
        if checked_now {
            self.run_automations(
                &card.board_id,
                "checklist_item_checked",
                &serde_json::json!({
                    "card_id": card.id,
                    "item_id": after.id,
                    "item_title": after.title,
                }),
            );
        }
        Ok(after)
    }

    pub fn delete_checklist_item(&self, id: &str) -> Result<()> {
        let db = self.lock_db()?;
        let item = db
            .conn
            .query_row("SELECT * FROM checklist_items WHERE id=?1", [id], row_checklist_item)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("item {id}")))?;
        db.conn.execute("DELETE FROM checklist_items WHERE id=?1", [id])?;
        let cl = db
            .conn
            .query_row("SELECT * FROM checklists WHERE id=?1", [&item.checklist_id], row_checklist)
            .optional()?
            .ok_or_else(|| Error::NotFound("checklist".into()))?;
        let card = query_card(&db.conn, &cl.card_id)?
            .ok_or_else(|| Error::NotFound("card".into()))?;
        drop(db);
        self.push_undo(UndoEntry {
            label: "Suppression d'un élément".into(),
            actions: vec![UndoAction::ChecklistItemDeleted(item)],
        });
        self.emit(Event::ChecklistsUpdated {
            board_id: card.board_id,
            card_id: card.id,
        });
        Ok(())
    }
}

/// Load all items of the given checklists in ONE query, grouped by checklist_id.
pub(crate) fn items_for_checklists(
    conn: &rusqlite::Connection,
    checklist_ids: &[String],
) -> Result<HashMap<String, Vec<ChecklistItem>>> {
    let mut map: HashMap<String, Vec<ChecklistItem>> = HashMap::new();
    if checklist_ids.is_empty() {
        return Ok(map);
    }
    let placeholders = checklist_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT * FROM checklist_items WHERE checklist_id IN ({placeholders}) ORDER BY checklist_id, position"
    );
    let mut stmt = conn.prepare(&sql)?;
    let ids: Vec<&str> = checklist_ids.iter().map(|s| s.as_str()).collect();
    let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), row_checklist_item)?;
    for r in rows {
        let it: ChecklistItem = r?;
        map.entry(it.checklist_id.clone()).or_default().push(it);
    }
    Ok(map)
}

trait NonEmpty {
    fn to_empty_checked(&self) -> Result<String>;
}

impl NonEmpty for str {
    fn to_empty_checked(&self) -> Result<String> {
        if self.trim().is_empty() {
            Err(crate::error::Error::Validation("titre requis".into()))
        } else {
            Ok(self.trim().to_string())
        }
    }
}

pub(crate) fn update_checklist_item_row(conn: &rusqlite::Connection, i: &ChecklistItem) -> Result<()> {
    conn.execute(
        "UPDATE checklist_items SET title=?2, checked=?3, position=?4, due=?5 WHERE id=?1",
        params![i.id, i.title, i.checked as i64, i.position, i.due],
    )?;
    Ok(())
}
