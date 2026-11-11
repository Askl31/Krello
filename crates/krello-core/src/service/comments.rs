//! Comments (local) + activity logging.

use rusqlite::OptionalExtension;

use super::{log_activity, Service};
use crate::error::{Error, Result};
use crate::events::Event;
use crate::ids::new_id;
use crate::models::*;
use crate::time::now_rfc3339;
use crate::undo::*;

impl Service {
    pub fn add_comment(&self, card_id: &str, body: &str) -> Result<Comment> {
        if body.trim().is_empty() {
            return Err(Error::Validation("commentaire vide".into()));
        }
        let comment = Comment {
            id: new_id("com"),
            card_id: card_id.to_string(),
            body: body.trim().to_string(),
            created_at: now_rfc3339(),
        };
        {
            let db = self.lock_db()?;
            let card =
                query_card(&db.conn, card_id)?.ok_or_else(|| Error::NotFound(format!("card {card_id}")))?;
            insert_comment_row(&db.conn, &comment)?;
            log_activity(
                &db.conn,
                &card.board_id,
                Some(card_id),
                "comment_added",
                &serde_json::json!({ "comment_id": comment.id, "excerpt": truncate(&comment.body, 80) }),
            )?;
            drop(db);
            self.emit(Event::CommentsUpdated {
                board_id: card.board_id.clone(),
                card_id: card_id.to_string(),
            });
            self.run_automations(
                &card.board_id,
                "card_commented",
                &serde_json::json!({ "card_id": card_id, "comment_id": comment.id }),
            );
        }
        self.push_undo(UndoEntry {
            label: "Ajout d'un commentaire".into(),
            actions: vec![UndoAction::CommentCreated(comment.clone())],
        });
        Ok(comment)
    }

    pub fn delete_comment(&self, id: &str) -> Result<()> {
        let db = self.lock_db()?;
        let comment = db
            .conn
            .query_row("SELECT * FROM comments WHERE id=?1", [id], row_comment)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("comment {id}")))?;
        db.conn.execute("DELETE FROM comments WHERE id=?1", [id])?;
        let card = query_card(&db.conn, &comment.card_id)?
            .ok_or_else(|| Error::NotFound("card".into()))?;
        drop(db);
        self.push_undo(UndoEntry {
            label: "Suppression d'un commentaire".into(),
            actions: vec![UndoAction::CommentDeleted(comment.clone())],
        });
        self.emit(Event::CommentsUpdated {
            board_id: card.board_id,
            card_id: comment.card_id.clone(),
        });
        Ok(())
    }

    pub fn list_comments(&self, card_id: &str) -> Result<Vec<Comment>> {
        let db = self.lock_db()?;
        let mut stmt =
            db.conn.prepare("SELECT * FROM comments WHERE card_id=?1 ORDER BY created_at")?;
        let comments = stmt
            .query_map([card_id], row_comment)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(comments)
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}
