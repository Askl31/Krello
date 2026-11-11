//! List CRUD with floating positions and WIP limits.



use super::{next_list_position, now_updated, renormalize_lists, Service};
use crate::error::{Error, Result};
use crate::events::Event;
use crate::ids::new_id;
use crate::models::*;
use crate::time::now_rfc3339;
use crate::undo::*;

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
            position: c.position.unwrap_or_else(|| 0.0), // fixed below
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
        let mut stmt = db.conn.prepare(sql)?;
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

    pub fn delete_list(&self, id: &str) -> Result<()> {
        let db = self.lock_db()?;
        let list =
            query_list(&db.conn, id)?.ok_or_else(|| Error::NotFound(format!("list {id}")))?;
        let board_id = list.board_id.clone();
        let mut cards = Vec::new();
        {
            let mut stmt = db.conn.prepare("SELECT id FROM cards WHERE list_id=?1")?;
            let ids: Vec<String> = {
                let v = stmt
                    .query_map([id], |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<_>>()?;
                v
            };
            drop(stmt);
            for cid in ids {
                cards.push(snapshot_card(&db.conn, &cid)?);
            }
        }
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
