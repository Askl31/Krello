//! CPM scheduler: forward/backward pass over the dependency network,
//! working-day calendars, critical path, total float, WBS codes.

use std::collections::{HashMap, HashSet, VecDeque};

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{Error, Result};
use crate::models::*;
use crate::time::{add_days, fmt_date, parse_date, today, weekday};
use crate::undo::{row_calendar, row_card, row_dependency, row_schedule};

#[derive(Debug, Clone)]
pub struct WorkingCal {
    pub weekends: HashSet<u32>,
    pub holidays: HashSet<NaiveDate>,
    pub hours_per_day: f64,
}

impl WorkingCal {
    pub fn is_working(&self, d: NaiveDate) -> bool {
        !self.weekends.contains(&weekday(d)) && !self.holidays.contains(&d)
    }

    pub fn next_working(&self, d: NaiveDate) -> NaiveDate {
        let mut x = d;
        while !self.is_working(x) {
            x = add_days(x, 1);
        }
        x
    }

    pub fn prev_working(&self, d: NaiveDate) -> NaiveDate {
        let mut x = d;
        while !self.is_working(x) {
            x = add_days(x, -1);
        }
        x
    }

    /// Advance n working days from d (d itself not counted). n may be negative.
    pub fn add_working(&self, d: NaiveDate, n: i64) -> NaiveDate {
        if n == 0 {
            return d;
        }
        let step = if n > 0 { 1 } else { -1 };
        let mut remaining = n.abs();
        let mut x = d;
        while remaining > 0 {
            x = add_days(x, step);
            if self.is_working(x) {
                remaining -= 1;
            }
        }
        x
    }

    /// Working days in [a, b) (0 if b <= a).
    pub fn count_between(&self, a: NaiveDate, b: NaiveDate) -> i64 {
        if b <= a {
            return 0;
        }
        let mut count = 0;
        let mut x = a;
        while x < b {
            if self.is_working(x) {
                count += 1;
            }
            x = add_days(x, 1);
        }
        count
    }
}

fn parse_json_f64_list(json: &str) -> Result<Vec<u32>> {
    serde_json::from_str(json).map_err(|e| Error::Validation(format!("calendrier invalide: {e}")))
}

pub fn calendar_for(conn: &Connection, calendar_id: Option<&str>) -> Result<WorkingCal> {
    let wanted = calendar_id.unwrap_or("default");
    let cal = conn
        .query_row("SELECT * FROM calendars WHERE id=?1", [wanted], row_calendar)
        .optional()?
        .or_else(|| {
            conn.query_row("SELECT * FROM calendars WHERE id='default'", [], row_calendar)
                .optional()
                .ok()
                .flatten()
        });
    match cal {
        Some(c) => Ok(WorkingCal {
            weekends: parse_json_f64_list(&c.weekends)?.into_iter().collect(),
            holidays: serde_json::from_str::<Vec<String>>(&c.holidays)
                .unwrap_or_default()
                .iter()
                .filter_map(|s| parse_date(s))
                .collect(),
            hours_per_day: if c.hours_per_day > 0.0 { c.hours_per_day } else { 8.0 },
        }),
        None => Ok(WorkingCal {
            weekends: [6, 0].into_iter().collect(),
            holidays: HashSet::new(),
            hours_per_day: 8.0,
        }),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScheduleSummary {
    pub board_id: String,
    pub start: Option<String>,
    pub finish: Option<String>,
    pub deadline: Option<String>,
    pub deadline_met: Option<bool>,
    pub cards: Vec<ScheduleEntry>,
}

struct Node {
    card: Card,
    span: i64,
    es: Option<NaiveDate>,
    ef: Option<NaiveDate>,
    ls: Option<NaiveDate>,
    lf: Option<NaiveDate>,
    preds: Vec<(String, String, f64)>, // (pred_id, type, lag)
    succs: Vec<(String, String, f64)>, // (succ_id, type, lag)
}

fn effective_span(card: &Card, cal: &WorkingCal) -> i64 {
    if card.is_milestone {
        return 0;
    }
    match card.task_type.as_str() {
        // work = duration × units × hours_per_day  →  duration = work / (units × hpd)
        "fixed_work" => {
            if let Some(work) = card.work_hours {
                let per_day = (card.units.max(0.01) * cal.hours_per_day.max(0.5)).max(0.01);
                ((work / per_day).ceil() as i64).max(0)
            } else {
                default_span(card)
            }
        }
        _ => default_span(card),
    }
}

fn default_span(card: &Card) -> i64 {
    if let Some(d) = card.duration_days {
        return d.ceil() as i64;
    }
    if let (Some(s), Some(e)) = (&card.start, &card.due) {
        if let (Some(sd), Some(ed)) = (parse_date(s), parse_date(e)) {
            return (ed - sd).num_days().max(0) as i64;
        }
    }
    1
}

/// Recompute the full schedule of a board and persist it into schedule_cache.
pub fn recompute(conn: &Connection, board_id: &str) -> Result<ScheduleSummary> {
    let board = crate::undo::query_board(conn, board_id)?
        .ok_or_else(|| Error::NotFound(format!("board {board_id}")))?;
    let cal = calendar_for(conn, board.calendar_id.as_deref())?;
    let mut cards: Vec<Card> = {
        let mut stmt =
            conn.prepare("SELECT * FROM cards WHERE board_id=?1 AND archived=0 AND is_template=0")?;
        let v = stmt
            .query_map([board_id], row_card)?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    let deps: Vec<Dependency> = {
        let mut stmt = conn.prepare("SELECT * FROM dependencies WHERE board_id=?1")?;
        let v = stmt
            .query_map([board_id], row_dependency)?
            .collect::<rusqlite::Result<_>>()?;
        v
    };

    // WBS: list order then card position.
    let list_order: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM lists WHERE board_id=?1 AND archived=0 ORDER BY position",
        )?;
        let v = stmt
            .query_map([board_id], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    let list_rank: HashMap<&String, usize> = list_order
        .iter()
        .enumerate()
        .map(|(i, id)| (id, i))
        .collect();
    cards.sort_by(|a, b| {
        let la = list_rank.get(&a.list_id).copied().unwrap_or(usize::MAX);
        let lb = list_rank.get(&b.list_id).copied().unwrap_or(usize::MAX);
        (la, a.position)
            .partial_cmp(&(lb, b.position))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    {
        let mut stmt = conn.prepare("UPDATE cards SET wbs=?2 WHERE id=?1")?;
        for (i, c) in cards.iter().enumerate() {
            stmt.execute(params![c.id, (i + 1).to_string()])?;
        }
    }

    let mut nodes: HashMap<String, Node> = cards
        .into_iter()
        .map(|c| {
            let span = effective_span(&c, &cal);
            (
                c.id.clone(),
                Node {
                    card: c,
                    span,
                    es: None,
                    ef: None,
                    ls: None,
                    lf: None,
                    preds: Vec::new(),
                    succs: Vec::new(),
                },
            )
        })
        .collect();

    for d in &deps {
        if let Some(p) = nodes.get_mut(&d.predecessor_id) {
            p.succs.push((d.successor_id.clone(), d.kind.clone(), d.lag_days));
        }
        if let Some(s) = nodes.get_mut(&d.successor_id) {
            s.preds.push((d.predecessor_id.clone(), d.kind.clone(), d.lag_days));
        }
    }

    // Topological order (Kahn).
    let mut indegree: HashMap<String, usize> = nodes
        .iter()
        .map(|(id, n)| (id.clone(), n.preds.len()))
        .collect();
    let mut queue: VecDeque<String> = nodes
        .iter()
        .filter(|(_, n)| n.preds.is_empty())
        .map(|(id, _)| id.clone())
        .collect();
    let mut topo: Vec<String> = Vec::with_capacity(nodes.len());
    while let Some(id) = queue.pop_front() {
        topo.push(id.clone());
        let succs = match nodes.get(&id) {
            Some(n) => n.succs.iter().map(|(s, _, _)| s.clone()).collect::<Vec<_>>(),
            None => continue,
        };
        for s in succs {
            let e = indegree.get_mut(&s).unwrap();
            *e -= 1;
            if *e == 0 {
                queue.push_back(s);
            }
        }
    }
    if topo.len() != nodes.len() {
        let stuck: Vec<String> = nodes
            .keys()
            .filter(|id| indegree.contains_key(*id) && *indegree.get(*id).unwrap_or(&0) > 0)
            .cloned()
            .collect();
        return Err(Error::Cycle(format!(
            "réseau de dépendances cyclique impliquant: {}",
            stuck.join(", ")
        )));
    }

    // Project start: board.start_date, else earliest explicit start, else today.
    let explicit_starts: Vec<NaiveDate> = nodes
        .values()
        .filter_map(|n| n.card.start.as_ref().and_then(|s| parse_date(s)))
        .collect();
    let project_start = board
        .start_date
        .as_ref()
        .and_then(|s| parse_date(s))
        .or_else(|| explicit_starts.iter().copied().min())
        .unwrap_or_else(today);
    let project_start = cal.next_working(project_start);

    // Forward pass.
    for id in &topo {
        let preds = nodes.get(id).unwrap().preds.clone();
        let constraint_start = {
            let n = nodes.get(id).unwrap();
            match (n.card.constraint_type.as_str(), &n.card.constraint_date) {
                ("start_not_before", Some(d)) => parse_date(d).map(|d| cal.next_working(d)),
                _ => None,
            }
        };
        let explicit_start = nodes
            .get(id)
            .unwrap()
            .card
            .start
            .as_ref()
            .and_then(|s| parse_date(s))
            .map(|d| cal.next_working(d));

        let mut es_candidates: Vec<NaiveDate> = vec![project_start];
        if let Some(c) = constraint_start {
            es_candidates.push(c);
        }
        // Respect explicitly set starts (no forced ASAP unless dependencies exist).
        if let Some(s) = explicit_start {
            es_candidates.push(s);
        }
        let span = nodes.get(id).unwrap().span;
        for (pred_id, kind, lag) in &preds {
            let p = nodes.get(pred_id).unwrap();
            let lag_i = *lag as i64;
            match kind.as_str() {
                "FS" => {
                    if let Some(pef) = p.ef {
                        es_candidates.push(cal.add_working(pef, 1 + lag_i));
                    }
                }
                "SS" => {
                    if let Some(pes) = p.es {
                        es_candidates.push(cal.add_working(pes, lag_i));
                    }
                }
                "FF" => {
                    if let Some(pef) = p.ef {
                        let cef = cal.add_working(pef, lag_i);
                        es_candidates.push(if span == 0 {
                            cef
                        } else {
                            cal.add_working(cef, -(span - 1))
                        });
                    }
                }
                "SF" => {
                    if let Some(pes) = p.es {
                        let cef = cal.add_working(pes, lag_i);
                        es_candidates.push(if span == 0 {
                            cef
                        } else {
                            cal.add_working(cef, -(span - 1))
                        });
                    }
                }
                _ => {}
            }
        }
        let es = es_candidates
            .into_iter()
            .max()
            .map(|d| cal.next_working(d));
        let node = nodes.get_mut(id).unwrap();
        node.es = es;
        node.ef = es.map(|e| {
            if span == 0 {
                e
            } else {
                cal.add_working(e, span - 1)
            }
        });
    }

    let project_finish = nodes
        .values()
        .filter_map(|n| n.ef)
        .max()
        .unwrap_or(project_start);

    // Backward pass (reverse topological order).
    for id in topo.iter().rev() {
        let succs = nodes.get(id).unwrap().succs.clone();
        let span = nodes.get(id).unwrap().span;
        let mut lf_candidates: Vec<NaiveDate> = vec![project_finish];
        for (succ_id, kind, lag) in &succs {
            let s = nodes.get(succ_id).unwrap();
            let lag_i = *lag as i64;
            match kind.as_str() {
                "FS" => {
                    if let Some(sls) = s.ls {
                        lf_candidates.push(cal.add_working(sls, -(1 + lag_i)));
                    }
                }
                "SS" => {
                    if let Some(sls) = s.ls {
                        let pls = cal.add_working(sls, -lag_i);
                        lf_candidates.push(if span == 0 {
                            pls
                        } else {
                            cal.add_working(pls, span - 1)
                        });
                    }
                }
                "FF" => {
                    if let Some(slf) = s.lf {
                        lf_candidates.push(cal.add_working(slf, -lag_i));
                    }
                }
                "SF" => {
                    if let Some(slf) = s.lf {
                        let pls = cal.add_working(slf, -lag_i);
                        lf_candidates.push(if span == 0 {
                            pls
                        } else {
                            cal.add_working(pls, span - 1)
                        });
                    }
                }
                _ => {}
            }
        }
        let lf = lf_candidates.into_iter().min().map(|d| cal.prev_working(d));
        let node = nodes.get_mut(id).unwrap();
        node.lf = lf;
        node.ls = lf.map(|l| {
            if span == 0 {
                l
            } else {
                cal.prev_working(cal.add_working(l, -(span - 1)))
            }
        });
    }

    // Persist schedule cache + summary.
    conn.execute("DELETE FROM schedule_cache WHERE card_id IN (SELECT id FROM cards WHERE board_id=?1)", [board_id])?;
    {
        let mut stmt = conn.prepare(
            "INSERT INTO schedule_cache(card_id, es, ef, ls, lf, total_float_days, is_critical, computed_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        )?;
        let mut entries = Vec::with_capacity(nodes.len());
        for (_, n) in &nodes {
            let (es, ef, ls, lf) = (n.es, n.ef, n.ls, n.lf);
            let float = match (ef, lf) {
                (Some(e), Some(l)) => {
                    if l < e {
                        -(cal.count_between(add_days(l, 1), add_days(e, 1)) as f64)
                    } else {
                        cal.count_between(add_days(e, 1), add_days(l, 1)) as f64
                    }
                }
                _ => 0.0,
            };
            let critical = float.abs() < 0.5;
            let now = crate::time::now_rfc3339();
            stmt.execute(params![
                n.card.id,
                es.map(fmt_date),
                ef.map(fmt_date),
                ls.map(fmt_date),
                lf.map(fmt_date),
                float,
                critical as i64,
                now
            ])?;
            entries.push(ScheduleEntry {
                card_id: n.card.id.clone(),
                es: es.map(fmt_date),
                ef: ef.map(fmt_date),
                ls: ls.map(fmt_date),
                lf: lf.map(fmt_date),
                total_float_days: Some(float),
                is_critical: critical,
            });
        }
        let deadline = board.deadline.as_ref().and_then(|d| parse_date(d));
        let deadline_met = deadline.map(|d| project_finish <= d);
        return Ok(ScheduleSummary {
            board_id: board_id.to_string(),
            start: Some(fmt_date(project_start)),
            finish: Some(fmt_date(project_finish)),
            deadline: deadline.map(fmt_date),
            deadline_met,
            cards: entries,
        });
    }
}

/// Service-level convenience: lock the DB and recompute.
pub fn recompute_service(service: &crate::service::Service, board_id: &str) -> Result<ScheduleSummary> {
    let db = service.lock_db()?;
    recompute(&db.conn, board_id)
}

/// Read the persisted schedule without recomputing.
pub fn cached_schedule(conn: &Connection, board_id: &str) -> Result<Vec<ScheduleEntry>> {
    let mut stmt = conn.prepare(
        "SELECT s.* FROM schedule_cache s JOIN cards c ON c.id = s.card_id
         WHERE c.board_id=?1 ORDER BY c.position",
    )?;
    let entries = {
        let v = stmt
            .query_map([board_id], row_schedule)?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    Ok(entries)
}
