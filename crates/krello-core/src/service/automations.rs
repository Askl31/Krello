//! Automation engine (Butler-like, local): rules with triggers, conditions and actions.
//!
//! - Rules live on a board but actions may target **any** board (copy / create_card / move).
//! - Event triggers are fired by domain services; scheduled triggers by `run_scheduled_sweep`.
//! - Every execution is logged in `automation_runs` (audit + per-day dedupe).
//! - Actions that create/move cards cascade into the target board's rules, capped at
//!   `MAX_DEPTH` so cross-board loops always terminate.
//! - Automation actions are applied with direct SQL: they are **not** undoable
//!   (consistent with the rest of the engine) but fully journaled.

use rusqlite::{params, OptionalExtension};
use chrono::Datelike;


use super::{log_activity, next_card_position, Service};
use crate::error::{Error, Result};
use crate::events::Event;
use crate::ids::new_id;
use crate::models::*;
use crate::time::{add_days, days_between, now_rfc3339, parse_date, today};
use crate::undo::*;

/// All valid triggers. Event triggers are fired by services; sweep triggers by the timer.
pub const TRIGGERS: [&str; 13] = [
    "card_created",
    "card_moved",
    "card_labeled",
    "card_unlabeled",
    "card_archived",
    "card_commented",
    "checklist_item_checked",
    "card_due_soon",
    "card_due",
    "button",
    "scheduled_daily",
    "scheduled_weekly",
    "scheduled_monthly",
];

/// Triggers evaluated by the periodic sweep (server timer, every minute).
pub const SWEEP_TRIGGERS: [&str; 5] = [
    "card_due_soon",
    "card_due",
    "scheduled_daily",
    "scheduled_weekly",
    "scheduled_monthly",
];

/// All valid action types.
pub const ACTIONS: [&str; 15] = [
    "move",
    "copy",
    "create_card",
    "label",
    "unlabel",
    "due",
    "clear_due",
    "priority",
    "check",
    "check_item",
    "archive",
    "unarchive",
    "comment",
    "set_title",
    "set_field",
];

/// Max cascade depth for cross-board rule chaining (loop guard).
const MAX_DEPTH: usize = 4;
/// Rows kept per automation in the run log.
const RUNS_KEEP: i64 = 500;

/// A follow-up event produced by an action (card created/moved on another board).
struct FollowUp {
    board_id: String,
    trigger: &'static str,
    ctx: serde_json::Value,
}

/// Template rendering context.
struct TplCtx<'a> {
    card: &'a Card,
    list_title: String,
    board_title: String,
    trigger: String,
    label_name: String,
}

impl<'a> TplCtx<'a> {
    fn new(conn: &rusqlite::Connection, card: &'a Card, ctx: &serde_json::Value) -> Self {
        let list_title = query_list(conn, &card.list_id)
            .ok()
            .flatten()
            .map(|l| l.title)
            .unwrap_or_default();
        let board_title = query_board(conn, &card.board_id)
            .ok()
            .flatten()
            .map(|b| b.title)
            .unwrap_or_default();
        let label_name = ctx
            .get("label_id")
            .and_then(|v| v.as_str())
            .and_then(|lid| {
                conn.query_row("SELECT name FROM labels WHERE id=?1", [lid], |r| r.get::<_, String>(0))
                    .optional()
                    .ok()
                    .flatten()
            })
            .unwrap_or_default();
        TplCtx {
            card,
            list_title,
            board_title,
            trigger: ctx
                .get("trigger")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            label_name,
        }
    }

    /// Replace `{{...}}` placeholders. Unknown placeholders are left as-is.
    fn render(&self, tpl: &str) -> String {
        let mut s = tpl.to_string();
        let date = today().to_string();
        let now = now_rfc3339();
        // Références empruntées : les champs ne sont remplacés que si le
        // placeholder est réellement présent dans le template.
        let pairs: [(&str, &str); 10] = [
            ("{{card.title}}", &self.card.title),
            ("{{card.id}}", &self.card.id),
            ("{{card.description}}", &self.card.description),
            ("{{card.priority}}", &self.card.priority),
            ("{{list.title}}", &self.list_title),
            ("{{board.title}}", &self.board_title),
            ("{{trigger}}", &self.trigger),
            ("{{label.name}}", &self.label_name),
            ("{{date}}", &date),
            ("{{now}}", &now),
        ];
        for (k, v) in pairs {
            if s.contains(k) {
                s = s.replace(k, v);
            }
        }
        s
    }
}

impl Service {
    pub fn create_automation(&self, board_id: &str, c: CreateAutomation) -> Result<Automation> {
        if !TRIGGERS.contains(&c.trigger.as_str()) {
            return Err(Error::Validation(format!(
                "déclencheur invalide: {} (attendus: {})",
                c.trigger,
                TRIGGERS.join(", ")
            )));
        }
        let actions = c.actions.unwrap_or_else(|| "[]".into());
        validate_actions(&actions)?;
        let conditions = c.conditions.unwrap_or_else(|| "{}".into());
        validate_conditions(&conditions)?;
        let a = Automation {
            id: new_id("auto"),
            board_id: board_id.to_string(),
            name: c.name.trim().to_string(),
            enabled: c.enabled.unwrap_or(true),
            trigger: c.trigger,
            conditions,
            actions,
            created_at: now_rfc3339(),
        };
        if a.name.is_empty() {
            return Err(Error::Validation("nom d'automatisation requis".into()));
        }
        {
            let db = self.lock_db()?;
            if query_board(&db.conn, board_id)?.is_none() {
                return Err(Error::NotFound(format!("board {board_id}")));
            }
            insert_automation_row(&db.conn, &a)?;
        }
        self.push_undo(UndoEntry {
            label: format!("Création de l'automatisation « {} »", a.name),
            actions: vec![UndoAction::AutomationCreated(a.clone())],
        });
        self.emit(Event::AutomationsUpdated {
            board_id: board_id.to_string(),
        });
        Ok(a)
    }

    pub fn update_automation(&self, id: &str, u: UpdateAutomation) -> Result<Automation> {
        let db = self.lock_db()?;
        let before = db
            .conn
            .query_row("SELECT * FROM automations WHERE id=?1", [id], row_automation)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("automation {id}")))?;
        if let Some(t) = &u.trigger {
            if !TRIGGERS.contains(&t.as_str()) {
                return Err(Error::Validation(format!(
                    "déclencheur invalide: {t} (attendus: {})",
                    TRIGGERS.join(", ")
                )));
            }
        }
        if let Some(a) = &u.actions {
            validate_actions(a)?;
        }
        if let Some(c) = &u.conditions {
            validate_conditions(c)?;
        }
        let after = Automation {
            name: u.name.unwrap_or_else(|| before.name.clone()),
            enabled: u.enabled.unwrap_or(before.enabled),
            trigger: u.trigger.unwrap_or_else(|| before.trigger.clone()),
            conditions: u.conditions.unwrap_or_else(|| before.conditions.clone()),
            actions: u.actions.unwrap_or_else(|| before.actions.clone()),
            ..before.clone()
        };
        update_automation_row(&db.conn, &after)?;
        drop(db);
        self.push_undo(UndoEntry {
            label: format!("Modification de l'automatisation « {} »", after.name),
            actions: vec![UndoAction::AutomationUpdated { before, after: after.clone() }],
        });
        self.emit(Event::AutomationsUpdated {
            board_id: after.board_id.clone(),
        });
        Ok(after)
    }

    pub fn delete_automation(&self, id: &str) -> Result<()> {
        let db = self.lock_db()?;
        let a = db
            .conn
            .query_row("SELECT * FROM automations WHERE id=?1", [id], row_automation)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("automation {id}")))?;
        db.conn.execute("DELETE FROM automations WHERE id=?1", [id])?;
        db.conn
            .execute("DELETE FROM automation_runs WHERE automation_id=?1", [id])?;
        drop(db);
        self.push_undo(UndoEntry {
            label: format!("Suppression de l'automatisation « {} »", a.name),
            actions: vec![UndoAction::AutomationDeleted(a.clone())],
        });
        self.emit(Event::AutomationsUpdated {
            board_id: a.board_id.clone(),
        });
        Ok(())
    }

    pub fn list_automations(&self, board_id: &str) -> Result<Vec<Automation>> {
        let db = self.lock_db()?;
        let mut stmt = db.conn.prepare_cached("SELECT * FROM automations WHERE board_id=?1 ORDER BY created_at")?;
        let autos = stmt
            .query_map([board_id], row_automation)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(autos)
    }

    /// Run log for one rule, most recent first.
    pub fn list_automation_runs(&self, automation_id: &str, limit: i64) -> Result<Vec<AutomationRun>> {
        let db = self.lock_db()?;
        let mut stmt = db.conn.prepare_cached("SELECT * FROM automation_runs WHERE automation_id=?1 ORDER BY created_at DESC, id DESC LIMIT ?2")?;
        let runs = stmt
            .query_map(params![automation_id, limit], row_automation_run)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(runs)
    }

    /// Run log for a whole board, most recent first.
    pub fn list_board_automation_runs(&self, board_id: &str, limit: i64) -> Result<Vec<AutomationRun>> {
        let db = self.lock_db()?;
        let mut stmt = db.conn.prepare_cached("SELECT * FROM automation_runs WHERE board_id=?1 ORDER BY created_at DESC, id DESC LIMIT ?2")?;
        let runs = stmt
            .query_map(params![board_id, limit], row_automation_run)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(runs)
    }

    /// Manually run a rule (Butler button). Targets are selected by conditions.
    pub fn run_automation(&self, id: &str) -> Result<usize> {
        let auto = {
            let db = self.lock_db()?;
            db.conn
                .query_row("SELECT * FROM automations WHERE id=?1", [id], row_automation)
                .optional()?
                .ok_or_else(|| Error::NotFound(format!("automation {id}")))?
        };
        if !auto.enabled {
            return Err(Error::Validation("automatisation désactivée".into()));
        }
        let targets = select_targets(self, &auto)?;
        let trigger = auto.trigger.clone();
        let count = self.execute_on_cards(&auto, &trigger, &targets, 0);
        Ok(count)
    }

    /// Fire all matching automations for an event trigger. Called from domain services.
    pub(crate) fn run_automations(&self, board_id: &str, trigger: &str, ctx: &serde_json::Value) {
        self.run_automations_depth(board_id, trigger, ctx, 0);
    }

    fn run_automations_depth(
        &self,
        board_id: &str,
        trigger: &str,
        ctx: &serde_json::Value,
        depth: usize,
    ) {
        if let Err(e) = self.run_automations_inner(board_id, trigger, ctx, depth) {
            tracing::warn!("automations: {e}");
        }
    }

    fn run_automations_inner(
        &self,
        board_id: &str,
        trigger: &str,
        ctx: &serde_json::Value,
        depth: usize,
    ) -> Result<()> {
        let autos = {
            let db = self.lock_db()?;
            let mut stmt = db.conn.prepare_cached("SELECT * FROM automations WHERE board_id=?1 AND enabled=1 AND trigger=?2")?;
            let v: Vec<Automation> = stmt
                .query_map(params![board_id, trigger], row_automation)?
                .collect::<rusqlite::Result<_>>()?;
            v
        };
        let card_id = ctx.get("card_id").and_then(|v| v.as_str());
        let mut jobs: Vec<(Automation, String)> = Vec::new();
        {
            let db = self.lock_db()?;
            for auto in autos {
                let conds: serde_json::Value =
                    serde_json::from_str(&auto.conditions).unwrap_or_default();
                let card = card_id.and_then(|cid| query_card(&db.conn, cid).ok().flatten());
                if !conditions_match(&db.conn, &conds, ctx, card.as_ref()) {
                    continue;
                }
                if let Some(cid) = card_id {
                    jobs.push((auto, cid.to_string()));
                }
            }
        }
        if jobs.is_empty() {
            return Ok(());
        }
        let mut followups: Vec<FollowUp> = Vec::new();
        let mut ran = false;
        {
            let db = self.lock_db()?;
            for (auto, cid) in &jobs {
                let Some(card) = query_card(&db.conn, cid)? else {
                    continue;
                };
                let mut ctx2 = ctx.clone();
                ctx2["trigger"] = serde_json::Value::String(trigger.to_string());
                let tpl = TplCtx::new(&db.conn, &card, &ctx2);
                let actions: Vec<serde_json::Value> =
                    serde_json::from_str(&auto.actions).unwrap_or_default();
                let mut ok = true;
                let mut detail = Vec::new();
                for action in &actions {
                    match apply_action(&db.conn, &card, action, &tpl) {
                        Ok(fus) => {
                            detail.push(serde_json::json!({ "action": action, "ok": true }));
                            followups.extend(fus);
                        }
                        Err(e) => {
                            ok = false;
                            detail.push(serde_json::json!({ "action": action, "error": e.to_string() }));
                        }
                    }
                }
                log_run(
                    &db.conn,
                    &auto.id,
                    &auto.board_id,
                    trigger,
                    Some(cid),
                    ok,
                    &serde_json::Value::Array(detail),
                )?;
                log_activity(
                    &db.conn,
                    board_id,
                    Some(cid),
                    "automation",
                    &serde_json::json!({ "automation": auto.name, "trigger": trigger }),
                )?;
                ran = true;
            }
        }
        for (_, cid) in &jobs {
            self.emit(Event::CardUpdated {
                board_id: board_id.to_string(),
                card_id: cid.clone(),
            });
        }
        if ran {
            self.emit(Event::AutomationRan {
                board_id: board_id.to_string(),
            });
        }
        for fu in followups {
            if depth < MAX_DEPTH {
                self.run_automations_depth(&fu.board_id, fu.trigger, &fu.ctx, depth + 1);
            } else {
                tracing::warn!(
                    "automations: profondeur max atteinte ({MAX_DEPTH}), cascade ignorée"
                );
            }
        }
        Ok(())
    }

    /// Resolve the moment a reminder should fire: an ISO datetime is absolute,
    /// "15m"|"1h"|"1d"|"2d" means N before the due date (local time).
    pub fn reminder_moment(reminder: &str, due: &str) -> Option<chrono::NaiveDateTime> {
        if let Some(dt) = crate::time::parse_dt(reminder) {
            return Some(dt);
        }
        let mins: i64 = if let Some(r) = reminder.trim().strip_suffix('m') {
            r.parse().ok()?
        } else if let Some(r) = reminder.trim().strip_suffix('h') {
            r.parse::<i64>().ok()? * 60
        } else if let Some(r) = reminder.trim().strip_suffix('d') {
            r.parse::<i64>().ok()? * 60 * 24
        } else {
            return None;
        };
        let due_dt = crate::time::parse_dt(due)?;
        Some(due_dt - chrono::Duration::minutes(mins))
    }

    /// Fire reminders whose moment is reached and not sent yet. One event per
    /// card; `reminder_sent_at` (reset whenever due/reminder changes) prevents
    /// duplicates. Archived cards, templates, done cards and cards without a
    /// parseable due/reminder are ignored. Returns the number of events fired.
    pub fn fire_due_reminders(&self) -> Result<usize> {
        let now = chrono::Local::now().naive_local();
        let db = self.lock_db()?;
        let mut stmt = db.conn.prepare_cached("SELECT id, board_id, title, due, reminder, reminder_sent_at FROM cards
         WHERE reminder IS NOT NULL AND reminder != '' AND due IS NOT NULL
           AND due_complete=0 AND archived=0 AND is_template=0")?;
        let targets: Vec<(String, String, String, String, String, Option<String>)> = {
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            })?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        drop(stmt);
        let mut fired = 0;
        for (id, board_id, title, due, reminder, sent) in targets {
            let Some(remind_at) = Self::reminder_moment(&reminder, &due) else {
                continue;
            };
            if let Some(sent_at) = sent.as_deref().and_then(crate::time::parse_dt) {
                if sent_at >= remind_at {
                    continue; // déjà envoyé pour cette échéance
                }
            }
            if now < remind_at {
                continue; // pas encore l'heure
            }
            db.conn.execute(
                "UPDATE cards SET reminder_sent_at=?2 WHERE id=?1",
                params![id, crate::time::now_rfc3339()],
            )?;
            self.emit(Event::ReminderDue {
                board_id,
                card_id: id,
                title,
                due,
            });
            fired += 1;
        }
        Ok(fired)
    }

    /// Apply a rule to a set of cards (button / scheduled triggers). Returns cards touched.
    fn execute_on_cards(
        &self,
        auto: &Automation,
        trigger: &str,
        targets: &[String],
        depth: usize,
    ) -> usize {
        let mut count = 0;
        let mut followups: Vec<FollowUp> = Vec::new();
        {
            let Ok(db) = self.lock_db() else { return 0 };
            for cid in targets {
                let Ok(Some(card)) = query_card(&db.conn, cid) else {
                    continue;
                };
                let ctx = serde_json::json!({
                    "card_id": card.id,
                    "list_id": card.list_id,
                    "trigger": trigger,
                });
                let tpl = TplCtx::new(&db.conn, &card, &ctx);
                let Ok(actions) = serde_json::from_str::<Vec<serde_json::Value>>(&auto.actions)
                else {
                    continue;
                };
                let mut ok = true;
                let mut detail = Vec::new();
                for action in &actions {
                    match apply_action(&db.conn, &card, action, &tpl) {
                        Ok(fus) => {
                            detail.push(serde_json::json!({ "action": action, "ok": true }));
                            followups.extend(fus);
                        }
                        Err(e) => {
                            ok = false;
                            detail.push(serde_json::json!({ "action": action, "error": e.to_string() }));
                            tracing::warn!("automation action: {e}");
                        }
                    }
                }
                let _ = log_run(
                    &db.conn,
                    &auto.id,
                    &auto.board_id,
                    trigger,
                    Some(cid),
                    ok,
                    &serde_json::Value::Array(detail),
                );
                let _ = log_activity(
                    &db.conn,
                    &auto.board_id,
                    Some(cid),
                    "automation",
                    &serde_json::json!({ "automation": auto.name, "trigger": trigger }),
                );
                count += 1;
            }
        }
        for cid in targets {
            self.emit(Event::CardUpdated {
                board_id: auto.board_id.clone(),
                card_id: cid.clone(),
            });
        }
        if count > 0 {
            self.emit(Event::AutomationRan {
                board_id: auto.board_id.clone(),
            });
        }
        for fu in followups {
            if depth < MAX_DEPTH {
                self.run_automations_depth(&fu.board_id, fu.trigger, &fu.ctx, depth + 1);
            }
        }
        count
    }

    /// From `candidates`, keep only the card ids the rule has NOT run on today.
    /// One grouped query (ran_today_set) instead of one COUNT per card.
    fn not_ran_today(
        &self,
        automation_id: &str,
        today_s: &str,
        candidates: Vec<String>,
    ) -> Result<Vec<String>> {
        if candidates.is_empty() {
            return Ok(candidates);
        }
        let ran = {
            let db = self.lock_db()?;
            let refs: Vec<&str> = candidates.iter().map(|s| s.as_str()).collect();
            ran_today_set(&db.conn, automation_id, today_s, &refs).unwrap_or_default()
        };
        Ok(candidates
            .into_iter()
            .filter(|cid| !ran.contains(cid))
            .collect())
    }

    /// Periodic sweep: scheduled_daily / scheduled_weekly / scheduled_monthly /
    /// card_due_soon / card_due. Called by the server on a timer (every minute).
    /// Scheduled triggers fire at most once per day per rule; due triggers once per
    /// day per card (dedupe via the `automation_runs` log).
    pub fn run_scheduled_sweep(&self) -> Result<usize> {
        let mut total = self.fire_due_reminders()?;
        let now_local = chrono::Local::now().naive_local();
        let t = today();
        let today_s = t.to_string();
        let boards = self.list_boards(false)?;
        for board in boards {
            let autos = {
                let db = self.lock_db()?;
                let mut stmt = db.conn.prepare_cached("SELECT * FROM automations WHERE board_id=?1 AND enabled=1 AND trigger IN
                 ('scheduled_daily','scheduled_weekly','scheduled_monthly','card_due_soon','card_due')")?;
                let v: Vec<Automation> = stmt
                    .query_map([&board.id], row_automation)?
                    .collect::<rusqlite::Result<_>>()?;
                v
            };
            for auto in autos {
                let conds: serde_json::Value =
                    serde_json::from_str(&auto.conditions).unwrap_or_default();
                let trigger = auto.trigger.clone();
                match trigger.as_str() {
                    "scheduled_daily" | "scheduled_weekly" | "scheduled_monthly" => {
                        // Time-of-day gate (default 00:00).
                        let gate = conds
                            .get("time")
                            .and_then(|v| v.as_str())
                            .and_then(|s| {
                                chrono::NaiveTime::parse_from_str(s, "%H:%M")
                                    .ok()
                                    .or_else(|| chrono::NaiveTime::parse_from_str(s, "%H:%M:%S").ok())
                            })
                            .unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
                        if now_local.time() < gate {
                            continue;
                        }
                        if trigger == "scheduled_weekly" {
                            // ISO weekday 1=Mon..7=Sun, default Monday.
                            let wd = conds
                                .get("weekday")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(1)
                                .clamp(1, 7) as u32;
                            if t.weekday().number_from_monday() != wd {
                                continue;
                            }
                        }
                        if trigger == "scheduled_monthly" {
                            let day = conds.get("day").and_then(|v| v.as_u64()).unwrap_or(1).clamp(1, 31) as u32;
                            if t.day() != day {
                                continue;
                            }
                        }
                        // Once per day per rule.
                        {
                            let db = self.lock_db()?;
                            if ran_today(&db.conn, &auto.id, None, &today_s)? {
                                continue;
                            }
                        }
                        let targets = select_targets(self, &auto)?;
                        let n = targets.len();
                        total += self.execute_on_cards(&auto, &trigger, &targets, 0);
                        let db = self.lock_db()?;
                        let _ = log_run(
                            &db.conn,
                            &auto.id,
                            &auto.board_id,
                            &trigger,
                            None,
                            true,
                            &serde_json::json!({ "targets": n }),
                        );
                    }
                    "card_due_soon" => {
                        let days = conds.get("days").and_then(|d| d.as_i64()).unwrap_or(1);
                        let cards = self.list_cards(&board.id, false)?;
                        let candidates: Vec<String> = cards
                            .into_iter()
                            .filter(|c| {
                                c.due
                                    .as_ref()
                                    .and_then(|d| parse_date(d))
                                    .map(|d| {
                                        let delta = days_between(t, d);
                                        delta >= 0 && delta <= days && !c.due_complete
                                    })
                                    .unwrap_or(false)
                            })
                            .map(|c| c.id)
                            .collect();
                        let targets = self.not_ran_today(&auto.id, &today_s, candidates)?;
                        total += self.execute_on_cards(&auto, &trigger, &targets, 0);
                    }
                    "card_due" => {
                        // Due date reached or passed (overdue), not completed.
                        let cards = self.list_cards(&board.id, false)?;
                        let candidates: Vec<String> = cards
                            .into_iter()
                            .filter(|c| {
                                c.due
                                    .as_ref()
                                    .and_then(|d| parse_date(d))
                                    .map(|d| days_between(t, d) <= 0 && !c.due_complete)
                                    .unwrap_or(false)
                            })
                            .map(|c| c.id)
                            .collect();
                        let targets = self.not_ran_today(&auto.id, &today_s, candidates)?;
                        total += self.execute_on_cards(&auto, &trigger, &targets, 0);
                    }
                    _ => {}
                }
            }
        }
        Ok(total)
    }
}

/// Cards selected by conditions (used by button/scheduled triggers).
fn select_targets(service: &Service, auto: &Automation) -> Result<Vec<String>> {
    let conds: serde_json::Value = serde_json::from_str(&auto.conditions).unwrap_or_default();
    let today_s = today().to_string();
    let mut sql = String::from(
        "SELECT id FROM cards WHERE board_id=?1 AND archived=0 AND is_template=0",
    );
    let mut p: Vec<String> = vec![auto.board_id.clone()];
    fn push(sql: &mut String, p: &mut Vec<String>, clause: &str, val: String) {
        p.push(val);
        sql.push_str(&format!(" AND {clause}"));
    }
    if let Some(l) = conds.get("list_id").and_then(|v| v.as_str()) {
        push(&mut sql, &mut p, "list_id=?", l.to_string());
    } else if let Some(arr) = conds.get("list_ids").and_then(|v| v.as_array()) {
        let ids: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        if !ids.is_empty() {
            let marks = vec!["?"; ids.len()].join(",");
            sql.push_str(&format!(" AND list_id IN ({marks})"));
            p.extend(ids);
        }
    }
    if let Some(l) = conds.get("label_id").and_then(|v| v.as_str()) {
        push(&mut sql, &mut p, "id IN (SELECT card_id FROM card_labels WHERE label_id=?)", l.to_string());
    }
    if let Some(l) = conds.get("without_label_id").and_then(|v| v.as_str()) {
        push(&mut sql, &mut p, "id NOT IN (SELECT card_id FROM card_labels WHERE label_id=?)", l.to_string());
    }
    if let Some(pr) = conds.get("priority").and_then(|v| v.as_str()) {
        push(&mut sql, &mut p, "priority=?", pr.to_string());
    }
    match conds.get("has_due").and_then(|v| v.as_bool()) {
        Some(true) => sql.push_str(" AND due IS NOT NULL"),
        Some(false) => sql.push_str(" AND due IS NULL"),
        None => {}
    }
    if conds.get("overdue").and_then(|v| v.as_bool()) == Some(true) {
        push(
            &mut sql,
            &mut p,
            "due IS NOT NULL AND substr(due,1,10)<=? AND due_complete=0",
            today_s.clone(),
        );
    }
    if let Some(sub) = conds.get("title_contains").and_then(|v| v.as_str()) {
        push(&mut sql, &mut p, "lower(title) LIKE ?", format!("%{}%", sub.to_lowercase()));
    }
    if let Some(sub) = conds.get("description_contains").and_then(|v| v.as_str()) {
        push(&mut sql, &mut p, "lower(description) LIKE ?", format!("%{}%", sub.to_lowercase()));
    }
    let db = service.lock_db()?;
    let mut stmt = db.conn.prepare_cached(&sql)?;
    let ids: Vec<String> = stmt
        .query_map(rusqlite::params_from_iter(p.iter()), |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ids)
}

pub(crate) fn insert_automation_row(conn: &rusqlite::Connection, a: &Automation) -> Result<()> {
    conn.execute(
        "INSERT INTO automations(id,board_id,name,enabled,trigger,conditions,actions,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![a.id, a.board_id, a.name, a.enabled as i64, a.trigger, a.conditions, a.actions, a.created_at],
    )?;
    Ok(())
}

pub(crate) fn update_automation_row(conn: &rusqlite::Connection, a: &Automation) -> Result<()> {
    conn.execute(
        "UPDATE automations SET name=?2, enabled=?3, trigger=?4, conditions=?5, actions=?6 WHERE id=?1",
        params![a.id, a.name, a.enabled as i64, a.trigger, a.conditions, a.actions],
    )?;
    Ok(())
}

pub(crate) fn row_automation_run(r: &rusqlite::Row<'_>) -> rusqlite::Result<AutomationRun> {
    Ok(AutomationRun {
        id: r.get("id")?,
        automation_id: r.get("automation_id")?,
        board_id: r.get("board_id")?,
        trigger: r.get("trigger")?,
        card_id: r.get("card_id")?,
        ok: r.get::<_, i64>("ok")? != 0,
        detail: r.get("detail")?,
        created_at: r.get("created_at")?,
    })
}

/// Append a run to the log and prune old rows (keep last RUNS_KEEP per rule).
fn log_run(
    conn: &rusqlite::Connection,
    automation_id: &str,
    board_id: &str,
    trigger: &str,
    card_id: Option<&str>,
    ok: bool,
    detail: &serde_json::Value,
) -> Result<()> {
    conn.execute(
        "INSERT INTO automation_runs(id,automation_id,board_id,trigger,card_id,ok,detail,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            new_id("run"),
            automation_id,
            board_id,
            trigger,
            card_id,
            ok as i64,
            detail.to_string(),
            now_rfc3339()
        ],
    )?;
    conn.execute(
        "DELETE FROM automation_runs WHERE automation_id=?1 AND id NOT IN
         (SELECT id FROM automation_runs WHERE automation_id=?1 ORDER BY created_at DESC, id DESC LIMIT ?2)",
        params![automation_id, RUNS_KEEP],
    )?;
    Ok(())
}

/// True if the rule already ran today (for this card, or board-level when card_id is None).
fn ran_today(
    conn: &rusqlite::Connection,
    automation_id: &str,
    card_id: Option<&str>,
    today_s: &str,
) -> Result<bool> {
    let n: i64 = match card_id {
        Some(cid) => conn.query_row(
            "SELECT COUNT(*) FROM automation_runs WHERE automation_id=?1 AND card_id=?2 AND substr(created_at,1,10)=?3",
            params![automation_id, cid, today_s],
            |r| r.get(0),
        )?,
        None => conn.query_row(
            "SELECT COUNT(*) FROM automation_runs WHERE automation_id=?1 AND card_id IS NULL AND substr(created_at,1,10)=?2",
            params![automation_id, today_s],
            |r| r.get(0),
        )?,
    };
    Ok(n > 0)
}

/// Card ids for which the rule already ran today — one grouped `IN (...)` query
/// instead of one `COUNT` per candidate card in the scheduled sweep.
fn ran_today_set(
    conn: &rusqlite::Connection,
    automation_id: &str,
    today_s: &str,
    card_ids: &[&str],
) -> Result<std::collections::HashSet<String>> {
    if card_ids.is_empty() {
        return Ok(Default::default());
    }
    let ph = vec!["?"; card_ids.len()].join(",");
    let sql = format!(
        "SELECT card_id FROM automation_runs WHERE automation_id=?1 AND substr(created_at,1,10)=?2 AND card_id IN ({ph})"
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let mut params: Vec<&dyn rusqlite::types::ToSql> = Vec::with_capacity(card_ids.len() + 2);
    params.push(&automation_id);
    params.push(&today_s);
    params.extend(card_ids.iter().map(|s| s as &dyn rusqlite::types::ToSql));
    let rows = stmt.query_map(params.as_slice(), |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn get_s<'a>(v: &'a serde_json::Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(|x| x.as_str())
}

/// Condition matching against the event context (+ card loaded from DB when available).
fn conditions_match(
    conn: &rusqlite::Connection,
    conds: &serde_json::Value,
    ctx: &serde_json::Value,
    card: Option<&Card>,
) -> bool {
    for key in ["list_id", "from_list_id", "to_list_id"] {
        if let Some(want) = get_s(conds, key) {
            let got = get_s(ctx, key).or_else(|| {
                if key == "list_id" { card.map(|c| c.list_id.as_str()) } else { None }
            });
            match got {
                Some(g) if g == want => {}
                _ => return false,
            }
        }
    }
    if let Some(want) = get_s(conds, "label_id") {
        // label présent dans le contexte (card_labeled/unlabeled) ou sur la carte elle-même.
        let in_ctx = get_s(ctx, "label_id") == Some(want);
        let in_card = card
            .map(|c| {
                label_ids_for_card(conn, &c.id)
                    .map(|ids| ids.iter().any(|l| l == want))
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        if !in_ctx && !in_card {
            return false;
        }
    }
    if let Some(arr) = conds.get("list_ids").and_then(|v| v.as_array()) {
        let cur = get_s(ctx, "list_id")
            .or_else(|| get_s(ctx, "to_list_id"))
            .unwrap_or("");
        if !arr.iter().any(|v| v.as_str() == Some(cur)) {
            return false;
        }
    }
    if let (Some(want), Some(c)) = (get_s(conds, "without_label_id"), card) {
        if let Ok(ids) = label_ids_for_card(conn, &c.id) {
            if ids.iter().any(|l| l == want) {
                return false;
            }
        }
    }
    let title = card.map(|c| c.title.as_str()).or_else(|| get_s(ctx, "title")).unwrap_or("");
    if let Some(sub) = get_s(conds, "title_contains") {
        if !title.to_lowercase().contains(&sub.to_lowercase()) {
            return false;
        }
    }
    if let (Some(sub), Some(c)) = (get_s(conds, "description_contains"), card) {
        if !c.description.to_lowercase().contains(&sub.to_lowercase()) {
            return false;
        }
    }
    if let (Some(want), Some(c)) = (get_s(conds, "priority"), card) {
        if c.priority != want {
            return false;
        }
    }
    if let (Some(want), Some(c)) = (conds.get("has_due").and_then(|v| v.as_bool()), card) {
        if c.due.is_some() != want {
            return false;
        }
    }
    if conds.get("overdue").and_then(|v| v.as_bool()) == Some(true) {
        if let Some(c) = card {
            let overdue = c
                .due
                .as_ref()
                .and_then(|d| parse_date(d))
                .map(|d| days_between(today(), d) <= 0 && !c.due_complete)
                .unwrap_or(false);
            if !overdue {
                return false;
            }
        }
    }
    true
}

fn req_s<'a>(action: &'a serde_json::Value, key: &str) -> Result<&'a str> {
    action
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::Validation(format!("action {}: « {key} » requis", action.get("type").and_then(|v| v.as_str()).unwrap_or("?"))))
}

/// Apply one automation action to a card, directly (no undo entries).
/// Returns follow-up events to cascade into other boards' rules.
fn apply_action(
    conn: &rusqlite::Connection,
    card: &Card,
    action: &serde_json::Value,
    tpl: &TplCtx,
) -> Result<Vec<FollowUp>> {
    let ty = action.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let now = now_rfc3339();
    match ty {
        "move" => {
            let list_id = req_s(action, "list_id")?;
            let target = query_list(conn, list_id)?
                .ok_or_else(|| Error::NotFound(format!("list {list_id}")))?;
            if target.archived {
                return Err(Error::Validation("liste archivée".into()));
            }
            let cross = target.board_id != card.board_id;
            let pos = match action.get("position") {
                Some(serde_json::Value::String(s)) if s == "top" => {
                    let min: Option<f64> = conn
                        .query_row(
                            "SELECT MIN(position) FROM cards WHERE list_id=?1",
                            [list_id],
                            |r| r.get(0),
                        )
                        .optional()?
                        .flatten();
                    min.unwrap_or(0.0) - 1.0
                }
                Some(v) => v.as_f64().unwrap_or_else(|| next_card_position(conn, list_id).unwrap_or(0.0)),
                None => next_card_position(conn, list_id)?,
            };
            if cross {
                // Cross-board move drops board-scoped data (same as manual move).
                conn.execute("DELETE FROM card_labels WHERE card_id=?1", [&card.id])?;
                conn.execute("DELETE FROM custom_field_values WHERE card_id=?1", [&card.id])?;
                conn.execute("DELETE FROM schedule_cache WHERE card_id=?1", [&card.id])?;
            }
            conn.execute(
                "UPDATE cards SET board_id=?2, list_id=?3, position=?4, updated_at=?5 WHERE id=?1",
                params![card.id, target.board_id, list_id, pos, now],
            )?;
            let mut fus = vec![FollowUp {
                board_id: target.board_id.clone(),
                trigger: "card_moved",
                ctx: serde_json::json!({
                    "card_id": card.id, "from_list_id": card.list_id, "to_list_id": list_id,
                    "from_board_id": card.board_id, "to_board_id": target.board_id,
                }),
            }];
            if cross {
                fus.push(FollowUp {
                    board_id: card.board_id.clone(),
                    trigger: "card_moved",
                    ctx: serde_json::json!({
                        "card_id": card.id, "from_list_id": card.list_id, "to_list_id": list_id,
                        "from_board_id": card.board_id, "to_board_id": target.board_id,
                    }),
                });
            }
            Ok(fus)
        }
        "copy" => {
            let list_id = req_s(action, "list_id")?;
            let target = query_list(conn, list_id)?
                .ok_or_else(|| Error::NotFound(format!("list {list_id}")))?;
            if target.archived {
                return Err(Error::Validation("liste archivée".into()));
            }
            let keep = action.get("keep").cloned().unwrap_or_else(|| serde_json::json!({}));
            let k_flag = |k: &str, d: bool| keep.get(k).and_then(|v| v.as_bool()).unwrap_or(d);
            let same_board = target.board_id == card.board_id;
            let title = action
                .get("title")
                .and_then(|v| v.as_str())
                .map(|t| tpl.render(t))
                .unwrap_or_else(|| card.title.clone());
            let new_card = Card {
                id: new_id("card"),
                board_id: target.board_id.clone(),
                list_id: list_id.to_string(),
                title,
                description: if k_flag("description", true) {
                    card.description.clone()
                } else {
                    String::new()
                },
                position: next_card_position(conn, list_id)?,
                archived: false,
                is_template: false,
                template_description: None,
                // Cover may reference a board-local attachment: only keep it same-board.
                cover: if same_board && k_flag("cover", false) {
                    card.cover.clone()
                } else {
                    None
                },
                // Dates are never copied (consistent with template instantiation).
                start: None,
                due: None,
                due_complete: false,
                reminder: None,
                reminder_sent_at: None,
                time_spent_minutes: 0,
                tracking_started_at: None,
                recurrence: None,
                priority: card.priority.clone(),
                is_milestone: false,
                duration_days: None,
                work_hours: None,
                pct_complete: 0,
                task_type: "fixed_duration".into(),
                units: 1.0,
                hours_per_day: 8.0,
                constraint_type: "asap".into(),
                constraint_date: None,
                wbs: String::new(),
                cost: 0.0,
                created_at: now.clone(),
                updated_at: now.clone(),
                completed_at: None,
            };
            insert_card_row(conn, &new_card)?;
            if k_flag("labels", true) {
                for lid in label_ids_for_card(conn, &card.id)? {
                    let Some(lbl) = conn
                        .query_row("SELECT * FROM labels WHERE id=?1", [&lid], row_label)
                        .optional()?
                    else {
                        continue;
                    };
                    // Find-or-create the label on the target board by (name, color).
                    let target_lid: String = conn
                        .query_row(
                            "SELECT id FROM labels WHERE board_id=?1 AND name=?2 AND color=?3",
                            params![target.board_id, lbl.name, lbl.color],
                            |r| r.get(0),
                        )
                        .optional()?
                        .unwrap_or_else(|| {
                            let nid = new_id("lbl");
                            conn.execute(
                                "INSERT INTO labels(id,board_id,name,color,created_at) VALUES(?1,?2,?3,?4,?5)",
                                params![nid, target.board_id, lbl.name, lbl.color, now],
                            )
                            .ok();
                            nid
                        });
                    conn.execute(
                        "INSERT OR IGNORE INTO card_labels(card_id,label_id) VALUES(?1,?2)",
                        params![new_card.id, target_lid],
                    )?;
                }
            }
            if k_flag("checklists", true) {
                let mut st = conn.prepare_cached("SELECT * FROM checklists WHERE card_id=?1 ORDER BY position")?;
                let cls: Vec<Checklist> = st
                    .query_map([&card.id], row_checklist)?
                    .collect::<rusqlite::Result<_>>()?;
                for cl in cls {
                    let ncl = new_id("cl");
                    insert_checklist_row(
                        conn,
                        &Checklist {
                            id: ncl.clone(),
                            card_id: new_card.id.clone(),
                            title: cl.title,
                            position: cl.position,
                            created_at: now.clone(),
                        },
                    )?;
                    let mut st2 =
                        conn.prepare_cached("SELECT * FROM checklist_items WHERE checklist_id=?1 ORDER BY position")?;
                    let items: Vec<ChecklistItem> = st2
                        .query_map([&cl.id], row_checklist_item)?
                        .collect::<rusqlite::Result<_>>()?;
                    for it in items {
                        insert_checklist_item_row(
                            conn,
                            &ChecklistItem {
                                id: new_id("cli"),
                                checklist_id: ncl.clone(),
                                title: it.title,
                                checked: it.checked,
                                position: it.position,
                                due: it.due,
                                created_at: now.clone(),
                            },
                        )?;
                    }
                }
            }
            if same_board && k_flag("fields", true) {
                conn.execute(
                    "INSERT OR IGNORE INTO custom_field_values(card_id,field_id,value)
                     SELECT ?1, field_id, value FROM custom_field_values WHERE card_id=?2",
                    params![new_card.id, card.id],
                )?;
            }
            log_activity(
                conn,
                &target.board_id,
                Some(&new_card.id),
                "automation",
                &serde_json::json!({ "copied_from": card.id, "copy": true }),
            )?;
            Ok(vec![FollowUp {
                board_id: target.board_id,
                trigger: "card_created",
                ctx: serde_json::json!({ "card_id": new_card.id, "list_id": list_id }),
            }])
        }
        "create_card" => {
            let list_id = req_s(action, "list_id")?;
            let title_tpl = req_s(action, "title")?;
            let target = query_list(conn, list_id)?
                .ok_or_else(|| Error::NotFound(format!("list {list_id}")))?;
            if target.archived {
                return Err(Error::Validation("liste archivée".into()));
            }
            let priority = action
                .get("priority")
                .and_then(|v| v.as_str())
                .unwrap_or("none");
            if !PRIORITIES.contains(&priority) {
                return Err(Error::Validation(format!("priorité invalide: {priority}")));
            }
            let new_card = Card {
                id: new_id("card"),
                board_id: target.board_id.clone(),
                list_id: list_id.to_string(),
                title: tpl.render(title_tpl),
                description: action
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(|d| tpl.render(d))
                    .unwrap_or_default(),
                position: next_card_position(conn, list_id)?,
                archived: false,
                is_template: false,
                template_description: None,
                cover: None,
                start: None,
                due: None,
                due_complete: false,
                reminder: None,
                reminder_sent_at: None,
                time_spent_minutes: 0,
                tracking_started_at: None,
                recurrence: None,
                priority: priority.to_string(),
                is_milestone: false,
                duration_days: None,
                work_hours: None,
                pct_complete: 0,
                task_type: "fixed_duration".into(),
                units: 1.0,
                hours_per_day: 8.0,
                constraint_type: "asap".into(),
                constraint_date: None,
                wbs: String::new(),
                cost: 0.0,
                created_at: now.clone(),
                updated_at: now.clone(),
                completed_at: None,
            };
            insert_card_row(conn, &new_card)?;
            log_activity(
                conn,
                &target.board_id,
                Some(&new_card.id),
                "automation",
                &serde_json::json!({ "created_by_automation": true }),
            )?;
            Ok(vec![FollowUp {
                board_id: target.board_id,
                trigger: "card_created",
                ctx: serde_json::json!({ "card_id": new_card.id, "list_id": list_id }),
            }])
        }
        "label" => {
            let label_id = req_s(action, "label_id")?;
            let owner: Option<String> = conn
                .query_row("SELECT board_id FROM labels WHERE id=?1", [label_id], |r| r.get(0))
                .optional()?;
            if owner.as_deref() != Some(card.board_id.as_str()) {
                return Err(Error::Validation("label hors board de la carte".into()));
            }
            conn.execute(
                "INSERT OR IGNORE INTO card_labels(card_id,label_id) VALUES(?1,?2)",
                params![card.id, label_id],
            )?;
            Ok(vec![])
        }
        "unlabel" => {
            let label_id = req_s(action, "label_id")?;
            conn.execute(
                "DELETE FROM card_labels WHERE card_id=?1 AND label_id=?2",
                params![card.id, label_id],
            )?;
            Ok(vec![])
        }
        "due" => {
            let due = if let Some(d) = get_s(action, "date") {
                match get_s(action, "time") {
                    Some(t) => crate::time::date_with_time(
                        parse_date(d).ok_or_else(|| Error::Validation(format!("date invalide: {d}")))?,
                        Some(t),
                    )
                    .format("%Y-%m-%dT%H:%M:%S")
                    .to_string(),
                    None => d.to_string(),
                }
            } else {
                let offset = action.get("offset_days").and_then(|v| v.as_i64()).unwrap_or(0);
                let d = add_days(today(), offset);
                match get_s(action, "time") {
                    Some(t) => crate::time::date_with_time(d, Some(t))
                        .format("%Y-%m-%dT%H:%M:%S")
                        .to_string(),
                    None => d.to_string(),
                }
            };
            conn.execute(
                "UPDATE cards SET due=?2, updated_at=?3 WHERE id=?1",
                params![card.id, due, now],
            )?;
            Ok(vec![])
        }
        "clear_due" => {
            conn.execute(
                "UPDATE cards SET due=NULL, due_complete=0, updated_at=?2 WHERE id=?1",
                params![card.id, now],
            )?;
            Ok(vec![])
        }
        "priority" => {
            let value = req_s(action, "value")?;
            if !PRIORITIES.contains(&value) {
                return Err(Error::Validation(format!("priorité invalide: {value}")));
            }
            conn.execute(
                "UPDATE cards SET priority=?2, updated_at=?3 WHERE id=?1",
                params![card.id, value, now],
            )?;
            Ok(vec![])
        }
        "check" => {
            let title = action
                .get("item_title")
                .and_then(|v| v.as_str())
                .unwrap_or("Action requise");
            let cl_id: Option<String> = conn
                .query_row(
                    "SELECT id FROM checklists WHERE card_id=?1 ORDER BY position LIMIT 1",
                    [&card.id],
                    |r| r.get(0),
                )
                .optional()?;
            let cl_id = match cl_id {
                Some(id) => id,
                None => {
                    let id = new_id("cl");
                    conn.execute(
                        "INSERT INTO checklists(id,card_id,title,position,created_at) VALUES(?1,?2,?3,0,?4)",
                        params![id, card.id, "Suivi", now],
                    )?;
                    id
                }
            };
            let pos: f64 = conn.query_row(
                "SELECT COALESCE(MAX(position), -1.0) + 1.0 FROM checklist_items WHERE checklist_id=?1",
                [&cl_id],
                |r| r.get(0),
            )?;
            conn.execute(
                "INSERT INTO checklist_items(id,checklist_id,title,checked,position,created_at)
                 VALUES(?1,?2,?3,0,?4,?5)",
                params![new_id("cli"), cl_id, title, pos, now],
            )?;
            Ok(vec![])
        }
        "check_item" => {
            // Check existing checklist items matching the title (case-insensitive).
            let title = action
                .get("item_title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Validation("check_item: « item_title » requis".into()))?;
            conn.execute(
                "UPDATE checklist_items SET checked=1 WHERE checklist_id IN
                 (SELECT id FROM checklists WHERE card_id=?1) AND lower(title)=lower(?2)",
                params![card.id, title],
            )?;
            Ok(vec![])
        }
        "archive" => {
            conn.execute(
                "UPDATE cards SET archived=1, updated_at=?2 WHERE id=?1",
                params![card.id, now],
            )?;
            Ok(vec![])
        }
        "unarchive" => {
            conn.execute(
                "UPDATE cards SET archived=0, updated_at=?2 WHERE id=?1",
                params![card.id, now],
            )?;
            Ok(vec![])
        }
        "comment" => {
            let body = tpl.render(req_s(action, "text")?);
            if body.trim().is_empty() {
                return Err(Error::Validation("comment: texte vide".into()));
            }
            insert_comment_row(
                conn,
                &Comment {
                    id: new_id("com"),
                    card_id: card.id.clone(),
                    body,
                    created_at: now.clone(),
                },
            )?;
            log_activity(
                conn,
                &card.board_id,
                Some(&card.id),
                "automation",
                &serde_json::json!({ "comment": true }),
            )?;
            Ok(vec![])
        }
        "set_title" => {
            let title = tpl.render(req_s(action, "value")?);
            if title.trim().is_empty() {
                return Err(Error::Validation("set_title: valeur vide".into()));
            }
            conn.execute(
                "UPDATE cards SET title=?2, updated_at=?3 WHERE id=?1",
                params![card.id, title, now],
            )?;
            Ok(vec![])
        }
        "set_field" => {
            let field_id = req_s(action, "field_id")?;
            let owner: Option<String> = conn
                .query_row(
                    "SELECT board_id FROM custom_field_defs WHERE id=?1",
                    [field_id],
                    |r| r.get(0),
                )
                .optional()?;
            if owner.as_deref() != Some(card.board_id.as_str()) {
                return Err(Error::Validation("champ hors board de la carte".into()));
            }
            let v = match action.get("value") {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
                None => String::new(),
            };
            conn.execute(
                "INSERT INTO custom_field_values(card_id,field_id,value) VALUES(?1,?2,?3)
                 ON CONFLICT(card_id,field_id) DO UPDATE SET value=excluded.value",
                params![card.id, field_id, v],
            )?;
            Ok(vec![])
        }
        other => Err(Error::Validation(format!(
            "action d'automatisation inconnue: {other}"
        ))),
    }
}

pub(crate) fn validate_conditions(conditions: &str) -> Result<()> {
    let v: serde_json::Value = serde_json::from_str(conditions)
        .map_err(|e| Error::Validation(format!("conditions JSON invalides: {e}")))?;
    if !v.is_object() {
        return Err(Error::Validation(
            "conditions doivent être un objet JSON".into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_actions(actions: &str) -> Result<()> {
    let parsed: Vec<serde_json::Value> = serde_json::from_str(actions)
        .map_err(|e| Error::Validation(format!("actions JSON invalides: {e}")))?;
    const REQUIRED: &[(&str, &[&str])] = &[
        ("move", &["list_id"]),
        ("copy", &["list_id"]),
        ("create_card", &["list_id", "title"]),
        ("label", &["label_id"]),
        ("unlabel", &["label_id"]),
        ("priority", &["value"]),
        ("comment", &["text"]),
        ("set_title", &["value"]),
        ("set_field", &["field_id", "value"]),
        ("check_item", &["item_title"]),
    ];
    for a in &parsed {
        let ty = a.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if !ACTIONS.contains(&ty) {
            return Err(Error::Validation(format!(
                "action inconnue: {ty} (attendus: {})",
                ACTIONS.join(", ")
            )));
        }
        if let Some((_, keys)) = REQUIRED.iter().find(|(t, _)| *t == ty) {
            for k in *keys {
                if a.get(k).is_none() {
                    return Err(Error::Validation(format!(
                        "action {ty}: champ « {k} » requis"
                    )));
                }
            }
        }
    }
    Ok(())
}
