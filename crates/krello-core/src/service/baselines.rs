//! Baselines: schedule snapshots for variance tracking (MS Project style).

use rusqlite::{params, OptionalExtension};

use super::Service;
use crate::error::{Error, Result};
use crate::events::Event;
use crate::ids::new_id;
use crate::models::*;
use crate::time::{days_between, now_rfc3339, parse_date};
use crate::undo::{query_board, row_baseline, row_baseline_snapshot, row_card, UndoAction, UndoEntry};

#[derive(Debug, Clone, serde::Serialize)]
pub struct BaselineDiff {
    pub card_id: String,
    pub title: String,
    pub start_baseline: Option<String>,
    pub start_now: Option<String>,
    pub due_baseline: Option<String>,
    pub due_now: Option<String>,
    pub duration_baseline: Option<f64>,
    pub duration_now: Option<f64>,
    pub work_baseline: Option<f64>,
    pub work_now: Option<f64>,
    /// > 0 = late versus baseline (calendar days on `due`).
    pub delta_due_days: Option<i64>,
}

impl Service {
    pub fn create_baseline(&self, board_id: &str, name: &str) -> Result<Baseline> {
        let baseline = Baseline {
            id: new_id("base"),
            board_id: board_id.to_string(),
            name: name.trim().to_string(),
            created_at: now_rfc3339(),
        };
        if baseline.name.is_empty() {
            return Err(Error::Validation("nom de baseline requis".into()));
        }
        let snapshots: Vec<BaselineSnapshot>;
        {
            let db = self.lock_db()?;
            if query_board(&db.conn, board_id)?.is_none() {
                return Err(Error::NotFound(format!("board {board_id}")));
            }
            db.conn.execute(
                "INSERT INTO baselines(id,board_id,name,created_at) VALUES(?1,?2,?3,?4)",
                params![baseline.id, board_id, baseline.name, baseline.created_at],
            )?;
            db.conn.execute(
                "INSERT INTO baseline_snapshots(baseline_id, card_id, start, due, duration_days, work_hours)
                 SELECT ?2, id, start, due, duration_days, work_hours FROM cards
                 WHERE board_id=?1 AND archived=0 AND is_template=0",
                params![board_id, baseline.id],
            )?;
            snapshots = {
                let mut stmt = db
                    .conn
                    .prepare("SELECT * FROM baseline_snapshots WHERE baseline_id=?1")?;
                let v = stmt
                    .query_map([&baseline.id], row_baseline_snapshot)?
                    .collect::<rusqlite::Result<_>>()?;
                v
            };
        }
        self.push_undo(UndoEntry {
            label: format!("Création de la baseline « {} »", baseline.name),
            actions: vec![UndoAction::BaselineCreated {
                baseline: baseline.clone(),
                snapshots,
            }],
        });
        self.emit(Event::BaselinesUpdated {
            board_id: board_id.to_string(),
        });
        Ok(baseline)
    }

    pub fn list_baselines(&self, board_id: &str) -> Result<Vec<Baseline>> {
        let db = self.lock_db()?;
        let mut stmt =
            db.conn.prepare("SELECT * FROM baselines WHERE board_id=?1 ORDER BY created_at DESC")?;
        let bases = stmt
            .query_map([board_id], row_baseline)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(bases)
    }

    pub fn delete_baseline(&self, id: &str) -> Result<()> {
        let db = self.lock_db()?;
        let baseline = db
            .conn
            .query_row("SELECT * FROM baselines WHERE id=?1", [id], row_baseline)
            .optional()?;
        // Lire les snapshots AVANT le DELETE (cascade FK).
        let snapshots: Vec<BaselineSnapshot> = {
            let mut stmt = db
                .conn
                .prepare("SELECT * FROM baseline_snapshots WHERE baseline_id=?1")?;
            let v = stmt
                .query_map([id], row_baseline_snapshot)?
                .collect::<rusqlite::Result<_>>()?;
            v
        };
        let n = db.conn.execute("DELETE FROM baselines WHERE id=?1", [id])?;
        if n == 0 {
            return Err(Error::NotFound(format!("baseline {id}")));
        }
        drop(db);
        let Some(baseline) = baseline else { return Ok(()) };
        self.push_undo(UndoEntry {
            label: format!("Suppression de la baseline « {} »", baseline.name),
            actions: vec![UndoAction::BaselineDeleted {
                baseline: baseline.clone(),
                snapshots,
            }],
        });
        self.emit(Event::BaselinesUpdated {
            board_id: baseline.board_id,
        });
        Ok(())
    }

    /// Compare a baseline against current card data.
    pub fn compare_baseline(&self, baseline_id: &str) -> Result<Vec<BaselineDiff>> {
        let db = self.lock_db()?;
        let exists: Option<String> = db
            .conn
            .query_row("SELECT id FROM baselines WHERE id=?1", [baseline_id], |r| r.get(0))
            .optional()?;
        if exists.is_none() {
            return Err(Error::NotFound(format!("baseline {baseline_id}")));
        }
        let mut stmt = db.conn.prepare(
            "SELECT s.card_id, s.start, s.due, s.duration_days, s.work_hours,
                    c.start, c.due, c.duration_days, c.work_hours, c.title
             FROM baseline_snapshots s
             LEFT JOIN cards c ON c.id = s.card_id
             WHERE s.baseline_id=?1
             ORDER BY s.due NULLS LAST",
        )?;
        let rows = stmt.query_map([baseline_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<f64>>(3)?,
                r.get::<_, Option<f64>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<f64>>(7)?,
                r.get::<_, Option<f64>>(8)?,
                r.get::<_, Option<String>>(9)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (card_id, sb, dbase, durb, wb, sn, dn, durn, wn, title) = row?;
            let delta_due = match (&dbase, &dn) {
                (Some(b), Some(n)) => match (parse_date(b), parse_date(n)) {
                    (Some(bd), Some(nd)) => Some(days_between(bd, nd)),
                    _ => None,
                },
                _ => None,
            };
            out.push(BaselineDiff {
                card_id,
                title: title.unwrap_or_else(|| "(carte supprimée)".into()),
                start_baseline: sb,
                start_now: sn,
                due_baseline: dbase,
                due_now: dn,
                duration_baseline: durb,
                duration_now: durn,
                work_baseline: wb,
                work_now: wn,
                delta_due_days: delta_due,
            });
        }
        drop(stmt);
        Ok(out)
    }

    /// Convenience: current cards of a board, sans modèles (export/dashboard).
    pub fn cards_for_dashboard(&self, board_id: &str) -> Result<Vec<Card>> {
        let db = self.lock_db()?;
        let mut stmt = db.conn.prepare(
            "SELECT * FROM cards WHERE board_id=?1 AND archived=0 AND is_template=0 ORDER BY due NULLS LAST",
        )?;
        let cards = stmt
            .query_map([board_id], row_card)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(cards)
    }
}
