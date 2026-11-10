//! Dependencies (FS/SS/FF/SF + lag) with cycle detection and schedule recompute.

use std::collections::{HashMap, HashSet, VecDeque};

use rusqlite::{params, OptionalExtension};

use super::{log_activity, Service};
use crate::error::{Error, Result};
use crate::events::Event;
use crate::ids::new_id;
use crate::models::*;
use crate::time::now_rfc3339;
use crate::undo::*;

impl Service {
    pub fn add_dependency(&self, board_id: &str, c: CreateDependency) -> Result<Dependency> {
        let kind = normalize_dep_type(&c.kind)?;
        let lag = c.lag_days.unwrap_or(0.0);
        if lag.abs() > 365.0 {
            return Err(Error::Validation("lag hors limites (±365 j)".into()));
        }
        let dep = {
            let db = self.lock_db()?;
            let pred = query_card(&db.conn, &c.predecessor_id)?
                .ok_or_else(|| Error::NotFound(format!("card {}", c.predecessor_id)))?;
            let succ = query_card(&db.conn, &c.successor_id)?
                .ok_or_else(|| Error::NotFound(format!("card {}", c.successor_id)))?;
            if pred.board_id != board_id || succ.board_id != board_id {
                return Err(Error::Validation("dépendance hors board".into()));
            }
            if pred.id == succ.id {
                return Err(Error::Validation("une carte ne peut pas dépendre d'elle-même".into()));
            }
            let exists: i64 = db.conn.query_row(
                "SELECT COUNT(*) FROM dependencies WHERE predecessor_id=?1 AND successor_id=?2 AND type=?3",
                params![c.predecessor_id, c.successor_id, kind],
                |r| r.get(0),
            )?;
            if exists > 0 {
                return Err(Error::Conflict("cette dépendance existe déjà".into()));
            }
            // Cycle check: can succ already reach pred through successor edges?
            detect_cycle(&db.conn, board_id, &c.successor_id, &c.predecessor_id, &kind)?;

            let dep = Dependency {
                id: new_id("dep"),
                board_id: board_id.to_string(),
                predecessor_id: c.predecessor_id,
                successor_id: c.successor_id,
                kind,
                lag_days: lag,
                created_at: now_rfc3339(),
            };
            insert_dependency_row(&db.conn, &dep)?;
            log_activity(
                &db.conn,
                board_id,
                Some(&dep.successor_id),
                "dependency_added",
                &serde_json::json!({ "pred": dep.predecessor_id, "type": dep.kind, "lag": dep.lag_days }),
            )?;
            dep
        };
        self.push_undo(UndoEntry {
            label: format!("Ajout d'une dépendance {}", dep.kind),
            actions: vec![UndoAction::DependencyCreated(dep.clone())],
        });
        self.recompute_and_notify(board_id)?;
        self.emit(Event::DependenciesUpdated {
            board_id: board_id.to_string(),
        });
        Ok(dep)
    }

    pub fn update_dependency(&self, id: &str, u: UpdateDependency) -> Result<Dependency> {
        let db = self.lock_db()?;
        let before = db
            .conn
            .query_row("SELECT * FROM dependencies WHERE id=?1", [id], row_dependency)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("dependency {id}")))?;
        let after = Dependency {
            kind: match &u.kind {
                Some(k) => normalize_dep_type(k)?,
                None => before.kind.clone(),
            },
            lag_days: u.lag_days.unwrap_or(before.lag_days),
            ..before.clone()
        };
        if after.lag_days.abs() > 365.0 {
            return Err(Error::Validation("lag hors limites (±365 j)".into()));
        }
        update_dependency_row(&db.conn, &after)?;
        let board_id = after.board_id.clone();
        drop(db);
        self.push_undo(UndoEntry {
            label: format!("Modification d'une dépendance {}", after.kind),
            actions: vec![UndoAction::DependencyUpdated { before, after: after.clone() }],
        });
        self.recompute_and_notify(&board_id)?;
        self.emit(Event::DependenciesUpdated { board_id });
        Ok(after)
    }

    pub fn remove_dependency(&self, id: &str) -> Result<()> {
        let db = self.lock_db()?;
        let dep = db
            .conn
            .query_row("SELECT * FROM dependencies WHERE id=?1", [id], row_dependency)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("dependency {id}")))?;
        db.conn.execute("DELETE FROM dependencies WHERE id=?1", [id])?;
        let board_id = dep.board_id.clone();
        drop(db);
        self.push_undo(UndoEntry {
            label: "Suppression d'une dépendance".into(),
            actions: vec![UndoAction::DependencyDeleted(dep)],
        });
        self.recompute_and_notify(&board_id)?;
        self.emit(Event::DependenciesUpdated { board_id });
        Ok(())
    }

    pub fn list_dependencies(&self, board_id: &str) -> Result<Vec<Dependency>> {
        let db = self.lock_db()?;
        let mut stmt =
            db.conn.prepare("SELECT * FROM dependencies WHERE board_id=?1 ORDER BY created_at")?;
        let deps = stmt
            .query_map([board_id], row_dependency)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(deps)
    }

    pub fn schedule_for_board(&self, board_id: &str) -> Result<crate::scheduler::ScheduleSummary> {
        let db = self.lock_db()?;
        crate::scheduler::recompute(&db.conn, board_id)
    }
}

pub(crate) fn normalize_dep_type(k: &str) -> Result<String> {
    let upper = k.trim().to_uppercase();
    if !DEP_TYPES.contains(&upper.as_str()) {
        return Err(Error::Validation(format!(
            "type de dépendance invalide: {k} (attendus: FS, SS, FF, SF)"
        )));
    }
    Ok(upper)
}

pub(crate) fn update_dependency_row(conn: &rusqlite::Connection, d: &Dependency) -> Result<()> {
    conn.execute(
        "UPDATE dependencies SET type=?2, lag_days=?3 WHERE id=?1",
        params![d.id, d.kind, d.lag_days],
    )?;
    Ok(())
}

/// BFS from `from` over successor edges; error if `to` is reachable
/// (i.e. adding edge to->from would close a cycle).
fn detect_cycle(
    conn: &rusqlite::Connection,
    board_id: &str,
    from: &str,
    to: &str,
    _kind: &str,
) -> Result<()> {
    let mut stmt = conn.prepare("SELECT predecessor_id, successor_id FROM dependencies WHERE board_id=?1")?;
    let edges: Vec<(String, String)> = stmt
        .query_map([board_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for (p, s) in edges {
        adj.entry(p).or_default().push(s);
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, Vec<String>)> = VecDeque::new();
    queue.push_back((from.to_string(), vec![from.to_string()]));
    while let Some((node, path)) = queue.pop_front() {
        if node == to {
            return Err(Error::Cycle(path.join(" → ")));
        }
        if !seen.insert(node.clone()) {
            continue;
        }
        for next in adj.get(&node).cloned().unwrap_or_default() {
            let mut p = path.clone();
            p.push(next.clone());
            queue.push_back((next, p));
        }
    }
    Ok(())
}
