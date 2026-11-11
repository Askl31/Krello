//! Labels: CRUD + card attach/detach with undo.

use rusqlite::{params, OptionalExtension};

use super::Service;
use crate::error::{Error, Result};
use crate::events::Event;
use crate::ids::new_id;
use crate::models::*;
use crate::time::now_rfc3339;
use crate::undo::*;

impl Service {
    pub fn create_label(&self, board_id: &str, c: CreateLabel) -> Result<Label> {
        let label = Label {
            id: new_id("label"),
            board_id: board_id.to_string(),
            name: c.name,
            color: validate_color(&c.color)?,
            created_at: now_rfc3339(),
        };
        {
            let db = self.lock_db()?;
            if query_board(&db.conn, board_id)?.is_none() {
                return Err(Error::NotFound(format!("board {board_id}")));
            }
            insert_label_row(&db.conn, &label)?;
        }
        self.push_undo(UndoEntry {
            label: format!("Création du label « {} »", label.name),
            actions: vec![UndoAction::LabelCreated(label.clone())],
        });
        self.emit(Event::LabelsUpdated {
            board_id: board_id.to_string(),
            card_id: None,
        });
        Ok(label)
    }

    pub fn update_label(&self, id: &str, u: UpdateLabel) -> Result<Label> {
        let db = self.lock_db()?;
        let before = db
            .conn
            .query_row("SELECT * FROM labels WHERE id=?1", [id], row_label)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("label {id}")))?;
        let after = Label {
            name: u.name.unwrap_or_else(|| before.name.clone()),
            color: match &u.color {
                Some(c) => validate_color(c)?,
                None => before.color.clone(),
            },
            ..before.clone()
        };
        update_label_row(&db.conn, &after)?;
        drop(db);
        self.push_undo(UndoEntry {
            label: format!("Modification du label « {} »", after.name),
            actions: vec![UndoAction::LabelUpdated {
                before,
                after: after.clone(),
            }],
        });
        self.emit(Event::LabelsUpdated {
            board_id: after.board_id.clone(),
            card_id: None,
        });
        Ok(after)
    }

    pub fn delete_label(&self, id: &str) -> Result<()> {
        let db = self.lock_db()?;
        let label = db
            .conn
            .query_row("SELECT * FROM labels WHERE id=?1", [id], row_label)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("label {id}")))?;
        db.conn.execute("DELETE FROM labels WHERE id=?1", [id])?;
        drop(db);
        self.push_undo(UndoEntry {
            label: format!("Suppression du label « {} »", label.name),
            actions: vec![UndoAction::LabelDeleted(label.clone())],
        });
        self.emit(Event::LabelsUpdated {
            board_id: label.board_id.clone(),
            card_id: None,
        });
        Ok(())
    }

    pub fn list_labels(&self, board_id: &str) -> Result<Vec<Label>> {
        let db = self.lock_db()?;
        let mut stmt =
            db.conn.prepare_cached("SELECT * FROM labels WHERE board_id=?1 ORDER BY created_at")?;
        let labels = stmt
            .query_map([board_id], row_label)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(labels)
    }

    pub fn attach_label(&self, card_id: &str, label_id: &str) -> Result<()> {
        let (before, board_id) = {
            let db = self.lock_db()?;
            let card =
                query_card(&db.conn, card_id)?.ok_or_else(|| Error::NotFound(format!("card {card_id}")))?;
            let owner: Option<String> = db
                .conn
                .query_row("SELECT board_id FROM labels WHERE id=?1", [label_id], |r| r.get(0))
                .optional()?;
            if owner.as_deref() != Some(card.board_id.as_str()) {
                return Err(Error::Validation("label hors board de la carte".into()));
            }
            let before = label_ids_for_card(&db.conn, card_id)?;
            db.conn.execute(
                "INSERT OR IGNORE INTO card_labels(card_id,label_id) VALUES(?1,?2)",
                params![card_id, label_id],
            )?;
            (before, card.board_id.clone())
        };
        let after = {
            let db = self.lock_db()?;
            label_ids_for_card(&db.conn, card_id)?
        };
        if before != after {
            self.push_undo(UndoEntry {
                label: "Ajout d'un label".into(),
                actions: vec![UndoAction::LabelsChanged {
                    card_id: card_id.to_string(),
                    before,
                    after: after.clone(),
                }],
            });
            self.emit(Event::LabelsUpdated {
                board_id: board_id.clone(),
                card_id: Some(card_id.to_string()),
            });
            self.run_automations(
                &board_id,
                "card_labeled",
                &serde_json::json!({ "card_id": card_id, "label_id": label_id }),
            );
        }
        Ok(())
    }

    pub fn detach_label(&self, card_id: &str, label_id: &str) -> Result<()> {
        let (before, board_id) = {
            let db = self.lock_db()?;
            let card =
                query_card(&db.conn, card_id)?.ok_or_else(|| Error::NotFound(format!("card {card_id}")))?;
            let before = label_ids_for_card(&db.conn, card_id)?;
            db.conn.execute(
                "DELETE FROM card_labels WHERE card_id=?1 AND label_id=?2",
                params![card_id, label_id],
            )?;
            (before, card.board_id.clone())
        };
        let after = {
            let db = self.lock_db()?;
            label_ids_for_card(&db.conn, card_id)?
        };
        if before != after {
            self.push_undo(UndoEntry {
                label: "Retrait d'un label".into(),
                actions: vec![UndoAction::LabelsChanged {
                    card_id: card_id.to_string(),
                    before,
                    after: after.clone(),
                }],
            });
            self.emit(Event::LabelsUpdated {
                board_id: board_id.clone(),
                card_id: Some(card_id.to_string()),
            });
            self.run_automations(
                &board_id,
                "card_unlabeled",
                &serde_json::json!({ "card_id": card_id, "label_id": label_id }),
            );
        }
        Ok(())
    }

    /// Replace the whole label set of a card (used by the card modal).
    pub fn set_card_labels(&self, card_id: &str, label_ids: Vec<String>) -> Result<()> {
        let before = {
            let db = self.lock_db()?;
            let card =
                query_card(&db.conn, card_id)?.ok_or_else(|| Error::NotFound(format!("card {card_id}")))?;
            let before = label_ids_for_card(&db.conn, card_id)?;
            db.conn.execute("DELETE FROM card_labels WHERE card_id=?1", [card_id])?;
            for lid in &label_ids {
                let owner: Option<String> = db
                    .conn
                    .query_row("SELECT board_id FROM labels WHERE id=?1", [lid], |r| r.get(0))
                    .optional()?;
                if owner.as_deref() != Some(card.board_id.as_str()) {
                    // restore
                    db.conn.execute(
                        "DELETE FROM card_labels WHERE card_id=?1",
                        [card_id],
                    )?;
                    for b in &before {
                        db.conn.execute(
                            "INSERT OR IGNORE INTO card_labels(card_id,label_id) VALUES(?1,?2)",
                            params![card_id, b],
                        )?;
                    }
                    return Err(Error::Validation(format!("label {lid} hors board")));
                }
                db.conn.execute(
                    "INSERT OR IGNORE INTO card_labels(card_id,label_id) VALUES(?1,?2)",
                    params![card_id, lid],
                )?;
            }
            before
        };
        let board_id = self.get_card(card_id)?.board_id;
        if before != label_ids {
            self.push_undo(UndoEntry {
                label: "Modification des labels".into(),
                actions: vec![UndoAction::LabelsChanged {
                    card_id: card_id.to_string(),
                    before,
                    after: label_ids.clone(),
                }],
            });
            self.emit(Event::LabelsUpdated {
                board_id,
                card_id: Some(card_id.to_string()),
            });
        }
        Ok(())
    }
}

fn validate_color(color: &str) -> Result<String> {
    let c = color.trim().to_lowercase();
    if c.is_empty() {
        return Err(Error::Validation("couleur requise".into()));
    }
    let ok = (c.starts_with('#') && (c.len() == 7 || c.len() == 4))
        || c.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-');
    if !ok {
        return Err(Error::Validation(format!("couleur invalide: {color}")));
    }
    Ok(c)
}

pub(crate) fn update_label_row(conn: &rusqlite::Connection, l: &Label) -> Result<()> {
    conn.execute(
        "UPDATE labels SET name=?2, color=?3 WHERE id=?1",
        params![l.id, l.name, l.color],
    )?;
    Ok(())
}
