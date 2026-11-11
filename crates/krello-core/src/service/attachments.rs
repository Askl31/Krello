//! Local file attachments stored under the data directory.

use std::path::PathBuf;

use rusqlite::OptionalExtension;

use super::{log_activity, Service};
use crate::error::{Error, Result};
use crate::events::Event;
use crate::ids::new_id;
use crate::models::*;
use crate::time::now_rfc3339;
use crate::undo::*;

pub const MAX_ATTACHMENT_BYTES: i64 = 64 * 1024 * 1024;

impl Service {
    pub fn add_attachment(
        &self,
        card_id: &str,
        filename: &str,
        mime: &str,
        data: &[u8],
    ) -> Result<Attachment> {
        if data.len() as i64 > MAX_ATTACHMENT_BYTES {
            return Err(Error::Validation(format!(
                "pièce jointe trop volumineuse (max {} Mo)",
                MAX_ATTACHMENT_BYTES / 1024 / 1024
            )));
        }
        if filename.trim().is_empty() {
            return Err(Error::Validation("nom de fichier requis".into()));
        }
        std::fs::create_dir_all(&self.attachments_dir)?;
        let id = new_id("att");
        let ext = std::path::Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e.to_lowercase()))
            .unwrap_or_default();
        let stored_name = format!("{id}{ext}");
        let path = self.attachments_dir.join(&stored_name);
        std::fs::write(&path, data)?;
        let att = Attachment {
            id: id.clone(),
            card_id: card_id.to_string(),
            filename: filename.trim().to_string(),
            stored_path: path.to_string_lossy().into_owned(),
            mime: if mime.is_empty() { "application/octet-stream".into() } else { mime.into() },
            size: data.len() as i64,
            is_cover: false,
            created_at: now_rfc3339(),
        };
        {
            let db = self.lock_db()?;
            let card =
                query_card(&db.conn, card_id)?.ok_or_else(|| Error::NotFound(format!("card {card_id}")))?;
            insert_attachment_row(&db.conn, &att)?;
            log_activity(
                &db.conn,
                &card.board_id,
                Some(card_id),
                "attachment_added",
                &serde_json::json!({ "filename": att.filename, "size": att.size }),
            )?;
            drop(db);
            self.emit(Event::AttachmentsUpdated {
                board_id: card.board_id,
                card_id: card_id.to_string(),
            });
        }
        self.push_undo(UndoEntry {
            label: format!("Ajout de la pièce jointe « {} »", att.filename),
            actions: vec![UndoAction::AttachmentCreated(att.clone())],
        });
        Ok(att)
    }

    pub fn delete_attachment(&self, id: &str) -> Result<()> {
        let db = self.lock_db()?;
        let att = db
            .conn
            .query_row("SELECT * FROM attachments WHERE id=?1", [id], row_attachment)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("attachment {id}")))?;
        // Unset cover if it references this attachment.
        let cover_json = serde_json::json!({"type": "attachment", "value": id}).to_string();
        db.conn.execute(
            "UPDATE cards SET cover=NULL WHERE cover=?1",
            [&cover_json],
        )?;
        db.conn.execute("DELETE FROM attachments WHERE id=?1", [id])?;
        drop(db);
        // Best-effort file removal.
        let _ = std::fs::remove_file(&att.stored_path);
        let board_id = {
            let db = self.lock_db()?;
            query_card(&db.conn, &att.card_id)?
                .map(|c| c.board_id)
                .ok_or_else(|| Error::NotFound("card".into()))?
        };
        self.push_undo(UndoEntry {
            label: format!("Suppression de la pièce jointe « {} »", att.filename),
            actions: vec![UndoAction::AttachmentDeleted(att.clone())],
        });
        self.emit(Event::AttachmentsUpdated {
            board_id,
            card_id: att.card_id.clone(),
        });
        Ok(())
    }

    pub fn get_attachment(&self, id: &str) -> Result<Attachment> {
        let db = self.lock_db()?;
        db.conn
            .query_row("SELECT * FROM attachments WHERE id=?1", [id], row_attachment)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("attachment {id}")))
    }

    pub fn list_attachments(&self, card_id: &str) -> Result<Vec<Attachment>> {
        let db = self.lock_db()?;
        let mut stmt =
            db.conn.prepare_cached("SELECT * FROM attachments WHERE card_id=?1 ORDER BY created_at")?;
        let atts = stmt
            .query_map([card_id], row_attachment)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(atts)
    }

    /// Mark an attachment as the card cover.
    pub fn set_cover(&self, card_id: &str, attachment_id: &str) -> Result<()> {
        let db = self.lock_db()?;
        let card =
            query_card(&db.conn, card_id)?.ok_or_else(|| Error::NotFound(format!("card {card_id}")))?;
        let att = db
            .conn
            .query_row("SELECT * FROM attachments WHERE id=?1", [attachment_id], row_attachment)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("attachment {attachment_id}")))?;
        if att.card_id != card_id {
            return Err(Error::Validation("pièce jointe hors carte".into()));
        }
        db.conn.execute(
            "UPDATE attachments SET is_cover=0 WHERE card_id=?1",
            [card_id],
        )?;
        db.conn.execute(
            "UPDATE attachments SET is_cover=1 WHERE id=?1",
            [attachment_id],
        )?;
        let mut after = card.clone();
        after.cover = Some(serde_json::json!({"type": "attachment", "value": attachment_id}).to_string());
        after.updated_at = crate::time::now_rfc3339();
        update_card_row(&db.conn, &after)?;
        let board_id = after.board_id.clone();
        drop(db);
        self.push_undo(UndoEntry {
            label: "Modification de la couverture".into(),
            actions: vec![UndoAction::CardUpdated { before: card, after }],
        });
        self.emit(Event::CardUpdated {
            board_id,
            card_id: card_id.to_string(),
        });
        Ok(())
    }

    pub fn set_color_cover(&self, card_id: &str, color: Option<&str>) -> Result<()> {
        let db = self.lock_db()?;
        let card =
            query_card(&db.conn, card_id)?.ok_or_else(|| Error::NotFound(format!("card {card_id}")))?;
        let mut after = card.clone();
        after.cover = color.map(|c| serde_json::json!({"type": "color", "value": c}).to_string());
        after.updated_at = crate::time::now_rfc3339();
        update_card_row(&db.conn, &after)?;
        let board_id = card.board_id.clone();
        drop(db);
        self.push_undo(UndoEntry {
            label: "Modification de la couverture".into(),
            actions: vec![UndoAction::CardUpdated { before: card, after }],
        });
        self.emit(Event::CardUpdated {
            board_id,
            card_id: card_id.to_string(),
        });
        Ok(())
    }

    /// Resolve a stored_path to a canonical path, refusing anything outside
    /// the attachments directory (path-traversal guard).
    pub fn attachment_path_checked(&self, stored_path: &str) -> Result<PathBuf> {
        let dir = self.attachments_dir.canonicalize()?;
        let canonical =
            PathBuf::from(stored_path).canonicalize().map_err(|_| Error::NotFound("file".into()))?;
        if !canonical.starts_with(&dir) {
            return Err(Error::Validation(
                "chemin hors répertoire des pièces jointes".into(),
            ));
        }
        Ok(canonical)
    }

    pub fn read_attachment_file(&self, stored_path: &str) -> Result<Vec<u8>> {
        // Only serve files inside the attachments directory.
        Ok(std::fs::read(self.attachment_path_checked(stored_path)?)?)
    }
}
