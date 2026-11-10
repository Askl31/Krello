pub mod attachments;
pub mod automations;
pub mod baselines;
pub mod boards;
pub mod calendars;
pub mod cards;
pub mod checklists;
pub mod comments;
pub mod deps;
pub mod fields;
pub mod labels;
pub mod lists;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::{params, Connection};
use tokio::sync::broadcast;

use crate::db::Db;
use crate::error::{Error, Result};
use crate::events::Event;
use crate::undo::{UndoEntry, UndoState, UNDO_CAP};

/// Thread-safe façade over SQLite. All mutations are transactional, emit events,
/// and push declarative undo entries (capped stack, redo cleared on new edits).
pub struct Service {
    pub(crate) db: Mutex<Db>,
    pub(crate) events: broadcast::Sender<Event>,
    pub(crate) attachments_dir: PathBuf,
    pub(crate) undo: Mutex<UndoState>,
}

impl Service {
    pub fn new(db: Db, attachments_dir: PathBuf) -> Service {
        let (events, _) = broadcast::channel(512);
        Service {
            db: Mutex::new(db),
            events,
            attachments_dir,
            undo: Mutex::new(UndoState::default()),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    pub(crate) fn lock_db(&self) -> Result<MutexGuard<'_, Db>> {
        self.db.lock().map_err(|_| Error::Lock)
    }

    pub(crate) fn emit(&self, event: Event) {
        let _ = self.events.send(event);
    }

    pub(crate) fn push_undo(&self, entry: UndoEntry) {
        if let Ok(mut st) = self.undo.lock() {
            st.undo.push(entry);
            if st.undo.len() > UNDO_CAP {
                st.undo.remove(0);
            }
            st.redo.clear();
        }
    }

    /// Undo the last operation. Returns its label, or None when the stack is empty.
    pub fn undo(&self) -> Result<Option<String>> {
        let entry = {
            let mut st = self.undo.lock().map_err(|_| Error::Lock)?;
            st.undo.pop()
        };
        let Some(entry) = entry else {
            return Ok(None);
        };
        {
            let mut db = self.lock_db()?;
            let tx = db.conn.transaction()?;
            entry.apply_undo(&tx)?;
            for b in entry.schedule_board_ids(&tx) {
                crate::scheduler::recompute(&tx, &b)?;
            }
            tx.commit()?;
        }
        let label = entry.label.clone();
        if let Ok(mut st) = self.undo.lock() {
            st.redo.push(entry.clone());
        }
        self.emit(Event::UndoApplied {
            board_id: entry.schedule_board_ids(&self.lock_db()?.conn).first().cloned(),
        });
        Ok(Some(label))
    }

    /// Redo the last undone operation. Returns its label, or None when empty.
    pub fn redo(&self) -> Result<Option<String>> {
        let entry = {
            let mut st = self.undo.lock().map_err(|_| Error::Lock)?;
            st.redo.pop()
        };
        let Some(entry) = entry else {
            return Ok(None);
        };
        {
            let mut db = self.lock_db()?;
            let tx = db.conn.transaction()?;
            entry.apply_redo(&tx)?;
            for b in entry.schedule_board_ids(&tx) {
                crate::scheduler::recompute(&tx, &b)?;
            }
            tx.commit()?;
        }
        let label = entry.label.clone();
        if let Ok(mut st) = self.undo.lock() {
            st.undo.push(entry.clone());
        }
        self.emit(Event::UndoApplied {
            board_id: entry.schedule_board_ids(&self.lock_db()?.conn).first().cloned(),
        });
        Ok(Some(label))
    }

    /// Labels of undoable operations, oldest first (for the UI history panel).
    pub fn undo_labels(&self) -> Vec<String> {
        self.undo
            .lock()
            .map(|st| st.undo.iter().map(|e| e.label.clone()).collect())
            .unwrap_or_default()
    }

    pub fn attachments_dir(&self) -> &PathBuf {
        &self.attachments_dir
    }

    /// Read-only access to the SQLite connection (search, exports, dashboards).
    pub fn with_db<T>(&self, f: impl FnOnce(&rusqlite::Connection) -> Result<T>) -> Result<T> {
        let db = self.lock_db()?;
        f(&db.conn)
    }

    /// Persist per-view client settings (filters, zoom, sort…).
    pub fn set_view_settings(&self, board_id: &str, view: &str, settings: &str) -> Result<()> {
        let db = self.lock_db()?;
        db.conn.execute(
            "INSERT INTO board_views(board_id, view, settings) VALUES(?1, ?2, ?3)
             ON CONFLICT(board_id, view) DO UPDATE SET settings = excluded.settings",
            [board_id, view, settings],
        )?;
        Ok(())
    }

    pub fn get_view_settings(&self, board_id: &str) -> Result<std::collections::HashMap<String, String>> {
        let db = self.lock_db()?;
        let mut stmt = db.conn.prepare("SELECT view, settings FROM board_views WHERE board_id=?1")?;
        let v = stmt
            .query_map([board_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<(String, String)>>>()?;
        Ok(v.into_iter().collect())
    }

    /// Public recompute entry point used by the HTTP API.
    pub fn recompute_and_notify_public(&self, board_id: &str) -> Result<crate::scheduler::ScheduleSummary> {
        self.recompute_and_notify(board_id)?;
        crate::scheduler::recompute_service(self, board_id)
    }

    /// Drop the whole undo/redo history (e.g. on import).
    pub fn clear_undo(&self) {
        if let Ok(mut st) = self.undo.lock() {
            st.undo.clear();
            st.redo.clear();
        }
    }

    /// Shared Arc constructor convenience for the server.
    pub fn shared(self) -> Arc<Service> {
        Arc::new(self)
    }
}

pub(crate) fn now_updated() -> String {
    crate::time::now_rfc3339()
}

pub(crate) fn next_card_position(conn: &Connection, list_id: &str) -> Result<f64> {
    let p: f64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1.0) + 1.0 FROM cards WHERE list_id = ?1",
        [list_id],
        |r| r.get(0),
    )?;
    Ok(p)
}

pub(crate) fn next_list_position(conn: &Connection, board_id: &str) -> Result<f64> {
    let p: f64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1.0) + 1.0 FROM lists WHERE board_id = ?1 AND archived = 0",
        [board_id],
        |r| r.get(0),
    )?;
    Ok(p)
}

/// Rewrite card positions in a list as 0.0, 1.0, 2.0… preserving visible order.
pub(crate) fn renormalize_cards(conn: &Connection, list_id: &str) -> Result<()> {
    let ids: Vec<String> = {
        let mut stmt =
            conn.prepare("SELECT id FROM cards WHERE list_id = ?1 ORDER BY position, created_at")?;
        let v = stmt
            .query_map([list_id], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE cards SET position = ?2 WHERE id = ?1",
            params![id, i as f64],
        )?;
    }
    Ok(())
}

/// Rewrite list positions in a board as 0.0, 1.0, 2.0…
pub(crate) fn renormalize_lists(conn: &Connection, board_id: &str) -> Result<()> {
    let ids: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM lists WHERE board_id = ?1 AND archived = 0 ORDER BY position, created_at",
        )?;
        let v = stmt
            .query_map([board_id], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE lists SET position = ?2 WHERE id = ?1",
            params![id, i as f64],
        )?;
    }
    Ok(())
}

pub(crate) fn log_activity(
    conn: &Connection,
    board_id: &str,
    card_id: Option<&str>,
    kind: &str,
    payload: &serde_json::Value,
) -> Result<()> {
    conn.execute(
        "INSERT INTO activity(id, board_id, card_id, kind, payload, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            crate::ids::new_id("act"),
            board_id,
            card_id,
            kind,
            payload.to_string(),
            crate::time::now_rfc3339()
        ],
    )?;
    Ok(())
}
