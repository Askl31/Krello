//! Custom fields: definitions and per-card values.

use rusqlite::{params, OptionalExtension};

use super::{Service};
use super::cards::set_field_value_tx;
use crate::error::{Error, Result};
use crate::events::Event;
use crate::ids::new_id;
use crate::models::*;
use crate::time::now_rfc3339;
use crate::undo::*;

pub const FIELD_TYPES: [&str; 5] = ["text", "number", "date", "checkbox", "select"];

impl Service {
    pub fn create_field_def(&self, board_id: &str, c: CreateFieldDef) -> Result<CustomFieldDef> {
        if !FIELD_TYPES.contains(&c.kind.as_str()) {
            return Err(Error::Validation(format!(
                "type de champ invalide: {} (attendus: text, number, date, checkbox, select)",
                c.kind
            )));
        }
        let db = self.lock_db()?;
        if query_board(&db.conn, board_id)?.is_none() {
            return Err(Error::NotFound(format!("board {board_id}")));
        }
        let pos: f64 = db.conn.query_row(
            "SELECT COALESCE(MAX(position), -1.0) + 1.0 FROM custom_field_defs WHERE board_id=?1",
            [board_id],
            |r| r.get(0),
        )?;
        let kind = c.kind.clone();
        let def = CustomFieldDef {
            id: new_id("cf"),
            board_id: board_id.to_string(),
            name: c.name.trim().to_string(),
            kind,
            options: validate_options(&c.kind, c.options.as_deref().unwrap_or("[]"))?,
            position: pos,
            created_at: now_rfc3339(),
        };
        if def.name.is_empty() {
            return Err(Error::Validation("nom de champ requis".into()));
        }
        insert_field_def_row(&db.conn, &def)?;
        drop(db);
        self.push_undo(UndoEntry {
            label: format!("Ajout du champ « {} »", def.name),
            actions: vec![UndoAction::FieldDefCreated(def.clone())],
        });
        self.emit(Event::FieldsUpdated {
            board_id: board_id.to_string(),
            card_id: None,
        });
        Ok(def)
    }

    pub fn update_field_def(&self, id: &str, u: UpdateFieldDef) -> Result<CustomFieldDef> {
        let db = self.lock_db()?;
        let before = db
            .conn
            .query_row("SELECT * FROM custom_field_defs WHERE id=?1", [id], row_field_def)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("field {id}")))?;
        let after = CustomFieldDef {
            name: u.name.unwrap_or_else(|| before.name.clone()),
            options: match &u.options {
                Some(o) => validate_options(&before.kind, o)?,
                None => before.options.clone(),
            },
            position: u.position.unwrap_or(before.position),
            ..before.clone()
        };
        update_field_def_row(&db.conn, &after)?;
        drop(db);
        self.push_undo(UndoEntry {
            label: format!("Modification du champ « {} »", after.name),
            actions: vec![UndoAction::FieldDefUpdated { before, after: after.clone() }],
        });
        self.emit(Event::FieldsUpdated {
            board_id: after.board_id.clone(),
            card_id: None,
        });
        Ok(after)
    }

    pub fn delete_field_def(&self, id: &str) -> Result<()> {
        let db = self.lock_db()?;
        let def = db
            .conn
            .query_row("SELECT * FROM custom_field_defs WHERE id=?1", [id], row_field_def)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("field {id}")))?;
        db.conn.execute("DELETE FROM custom_field_defs WHERE id=?1", [id])?;
        drop(db);
        self.push_undo(UndoEntry {
            label: format!("Suppression du champ « {} »", def.name),
            actions: vec![UndoAction::FieldDefDeleted(def.clone())],
        });
        self.emit(Event::FieldsUpdated {
            board_id: def.board_id.clone(),
            card_id: None,
        });
        Ok(())
    }

    pub fn list_field_defs(&self, board_id: &str) -> Result<Vec<CustomFieldDef>> {
        let db = self.lock_db()?;
        let mut stmt = db.conn.prepare_cached("SELECT * FROM custom_field_defs WHERE board_id=?1 ORDER BY position")?;
        let defs = stmt
            .query_map([board_id], row_field_def)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(defs)
    }

    /// Set (or clear with empty value) a custom field value on a card.
    pub fn set_field_value(&self, card_id: &str, field_id: &str, value: &str) -> Result<()> {
        let before: Option<String> = {
            let db = self.lock_db()?;
            let card =
                query_card(&db.conn, card_id)?.ok_or_else(|| Error::NotFound(format!("card {card_id}")))?;
            let prev: Option<String> = db
                .conn
                .query_row(
                    "SELECT value FROM custom_field_values WHERE card_id=?1 AND field_id=?2",
                    params![card_id, field_id],
                    |r| r.get(0),
                )
                .optional()?;
            set_field_value_tx(&db.conn, &card.board_id, card_id, field_id, value)?;
            prev
        };
        if before.as_deref() != Some(value) {
            self.push_undo(UndoEntry {
                label: "Modification d'un champ personnalisé".into(),
                actions: vec![UndoAction::FieldSet {
                    card_id: card_id.to_string(),
                    field_id: field_id.to_string(),
                    before,
                    after: if value.is_empty() { None } else { Some(value.to_string()) },
                }],
            });
            let board_id = self.get_card(card_id)?.board_id;
            self.emit(Event::FieldsUpdated {
                board_id,
                card_id: Some(card_id.to_string()),
            });
        }
        Ok(())
    }
}

pub(crate) fn insert_field_def_row(conn: &rusqlite::Connection, d: &CustomFieldDef) -> Result<()> {
    conn.execute(
        "INSERT INTO custom_field_defs(id,board_id,name,type,options,position,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![d.id, d.board_id, d.name, d.kind, d.options, d.position, d.created_at],
    )?;
    Ok(())
}

pub(crate) fn update_field_def_row(conn: &rusqlite::Connection, d: &CustomFieldDef) -> Result<()> {
    conn.execute(
        "UPDATE custom_field_defs SET name=?2, options=?3, position=?4 WHERE id=?1",
        params![d.id, d.name, d.options, d.position],
    )?;
    Ok(())
}

fn validate_options(kind: &str, options: &str) -> Result<String> {
    if kind != "select" {
        return Ok("[]".into());
    }
    let parsed: Result<Vec<String>, _> = serde_json::from_str(options);
    match parsed {
        Ok(opts) if !opts.is_empty() => Ok(serde_json::to_string(&opts)?),
        Ok(_) => Err(Error::Validation(
            "un champ de type select nécessite au moins une option".into(),
        )),
        Err(e) => Err(Error::Validation(format!("options JSON invalides: {e}"))),
    }
}
