//! Automation engine (Butler-like, local): rules with triggers, conditions and actions.

use rusqlite::{params, OptionalExtension};

use super::{log_activity, next_card_position, Service};
use crate::error::{Error, Result};
use crate::events::Event;
use crate::ids::new_id;
use crate::models::*;
use crate::time::{now_rfc3339, parse_date, today};
use crate::undo::*;

pub const TRIGGERS: [&str; 7] = [
    "card_created",
    "card_moved",
    "card_labeled",
    "card_due_soon",
    "button",
    "scheduled_daily",
    "scheduled_weekly",
];

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
        let a = Automation {
            id: new_id("auto"),
            board_id: board_id.to_string(),
            name: c.name.trim().to_string(),
            enabled: c.enabled.unwrap_or(true),
            trigger: c.trigger,
            conditions: c.conditions.unwrap_or_else(|| "{}".into()),
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
                return Err(Error::Validation(format!("déclencheur invalide: {t}")));
            }
        }
        if let Some(a) = &u.actions {
            validate_actions(a)?;
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
        let mut stmt = db.conn.prepare(
            "SELECT * FROM automations WHERE board_id=?1 ORDER BY created_at",
        )?;
        let autos = stmt
            .query_map([board_id], row_automation)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(autos)
    }

    /// Manually run a button automation. Targets are selected by conditions.
    pub fn run_automation(&self, id: &str) -> Result<usize> {
        let auto = {
            let db = self.lock_db()?;
            db.conn
                .query_row("SELECT * FROM automations WHERE id=?1", [id], row_automation)
                .optional()?
                .ok_or_else(|| Error::NotFound(format!("automation {id}")))?
        };
        let targets = select_targets(self, &auto)?;
        let count = self.apply_to_cards(&auto, &targets);
        Ok(count)
    }

    /// Fire all matching automations for a trigger. Called from card services.
    pub(crate) fn run_automations(&self, board_id: &str, trigger: &str, ctx: &serde_json::Value) {
        if let Err(e) = self.run_automations_inner(board_id, trigger, ctx) {
            tracing::warn!("automations: {e}");
        }
    }

    fn run_automations_inner(&self, board_id: &str, trigger: &str, ctx: &serde_json::Value) -> Result<()> {
        let autos = {
            let db = self.lock_db()?;
            let mut stmt = db.conn.prepare(
                "SELECT * FROM automations WHERE board_id=?1 AND enabled=1 AND trigger=?2",
            )?;
            let v: Vec<Automation> = stmt
                .query_map(params![board_id, trigger], row_automation)?
                .collect::<rusqlite::Result<_>>()?;
            v
        };
        let card_id = ctx.get("card_id").and_then(|v| v.as_str());
        let mut touched = Vec::new();
        for auto in autos {
            let conds: serde_json::Value = serde_json::from_str(&auto.conditions).unwrap_or_default();
            if !conditions_match(&conds, ctx) {
                continue;
            }
            if let Some(card_id) = card_id {
                touched.push((auto, card_id.to_string()));
            }
        }
        if touched.is_empty() {
            return Ok(());
        }
        let mut events = Vec::new();
        {
            let db = self.lock_db()?;
            for (auto, card_id) in &touched {
                let Some(card) = query_card(&db.conn, card_id)? else {
                    continue;
                };
                let actions: Vec<serde_json::Value> =
                    serde_json::from_str(&auto.actions).unwrap_or_default();
                for action in &actions {
                    apply_action(&db.conn, &card, action, &auto.id)?;
                }
                log_activity(
                    &db.conn,
                    board_id,
                    Some(card_id),
                    "automation",
                    &serde_json::json!({ "automation": auto.name, "trigger": auto.trigger }),
                )?;
                events.push(card_id.clone());
            }
        }
        for card_id in events {
            self.emit(Event::CardUpdated {
                board_id: board_id.to_string(),
                card_id,
            });
        }
        Ok(())
    }

    /// Periodic sweep: scheduled_daily / scheduled_weekly / card_due_soon.
    /// Called by the server on a timer.
    pub fn run_scheduled_sweep(&self) -> Result<usize> {
        let mut total = 0;
        let boards = self.list_boards(false)?;
        for board in boards {
            let autos = {
                let db = self.lock_db()?;
                let mut stmt = db.conn.prepare(
                    "SELECT * FROM automations WHERE board_id=?1 AND enabled=1 AND trigger IN
                     ('scheduled_daily','scheduled_weekly','card_due_soon')",
                )?;
                let v: Vec<Automation> = stmt
                    .query_map([&board.id], row_automation)?
                    .collect::<rusqlite::Result<_>>()?;
                v
            };
            for auto in autos {
                match auto.trigger.as_str() {
                    "scheduled_daily" | "scheduled_weekly" => {
                        if auto.trigger == "scheduled_weekly"
                            && chrono::Datelike::weekday(&today()) != chrono::Weekday::Mon
                        {
                            // weekly rules run on Mondays
                            continue;
                        }
                        let targets = select_targets(self, &auto)?;
                        total += self.apply_to_cards(&auto, &targets);
                    }
                    "card_due_soon" => {
                        let conds: serde_json::Value =
                            serde_json::from_str(&auto.conditions).unwrap_or_default();
                        let days = conds.get("days").and_then(|d| d.as_i64()).unwrap_or(1);
                        let cards = self.list_cards(&board.id, false)?;
                        let now = now_rfc3339();
                        let targets: Vec<String> = cards
                            .into_iter()
                            .filter(|c| {
                                c.due
                                    .as_ref()
                                    .and_then(|d| parse_date(d).or_else(|| crate::time::parse_dt(d).map(|t| t.date())))
                                    .map(|d| {
                                        let delta = crate::time::days_between(today(), d);
                                        delta >= 0 && delta <= days && !c.due_complete && !c.is_template
                                    })
                                    .unwrap_or(false)
                            })
                            .map(|c| c.id)
                            .collect();
                        let _ = now;
                        total += self.apply_to_cards(&auto, &targets);
                    }
                    _ => {}
                }
            }
        }
        Ok(total)
    }

    fn apply_to_cards(&self, auto: &Automation, card_ids: &[String]) -> usize {
        let mut count = 0;
        {
            let Ok(db) = self.lock_db() else { return 0 };
            for cid in card_ids {
                let Ok(Some(card)) = query_card(&db.conn, cid) else {
                    continue;
                };
                let Ok(actions) = serde_json::from_str::<Vec<serde_json::Value>>(&auto.actions) else {
                    continue;
                };
                for action in &actions {
                    if let Err(e) = apply_action(&db.conn, &card, action, &auto.id) {
                        tracing::warn!("automation action: {e}");
                    }
                }
                let _ = log_activity(
                    &db.conn,
                    &auto.board_id,
                    Some(cid),
                    "automation",
                    &serde_json::json!({ "automation": auto.name }),
                );
                count += 1;
            }
        }
        for cid in card_ids {
            self.emit(Event::CardUpdated {
                board_id: auto.board_id.clone(),
                card_id: cid.clone(),
            });
        }
        count
    }
}

/// Cards selected by conditions (used by button/scheduled triggers).
fn select_targets(service: &Service, auto: &Automation) -> Result<Vec<String>> {
    let conds: serde_json::Value = serde_json::from_str(&auto.conditions).unwrap_or_default();
    let list_id = conds.get("list_id").and_then(|v| v.as_str());
    let label_id = conds.get("label_id").and_then(|v| v.as_str());
    let db = service.lock_db()?;
    let mut stmt = db.conn.prepare(
        "SELECT id FROM cards WHERE board_id=?1 AND archived=0 AND is_template=0
         AND (?2 IS NULL OR list_id=?2)
         AND (?3 IS NULL OR id IN (SELECT card_id FROM card_labels WHERE label_id=?3))",
    )?;
    let ids: Vec<String> = stmt
        .query_map(
            params![auto.board_id, list_id, label_id],
            |r| r.get::<_, String>(0),
        )?
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

fn get_s<'a>(v: &'a serde_json::Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(|x| x.as_str())
}

fn conditions_match(conds: &serde_json::Value, ctx: &serde_json::Value) -> bool {
    for key in ["list_id", "from_list_id", "to_list_id", "label_id"] {
        if let Some(want) = get_s(conds, key) {
            match get_s(ctx, key) {
                Some(got) if got == want => {}
                _ => return false,
            }
        }
    }
    if let Some(sub) = get_s(conds, "title_contains") {
        let title = get_s(ctx, "title").unwrap_or("");
        if !title.to_lowercase().contains(&sub.to_lowercase()) {
            return false;
        }
    }
    true
}

/// Apply one automation action to a card, directly (no undo entries).
fn apply_action(
    conn: &rusqlite::Connection,
    card: &Card,
    action: &serde_json::Value,
    automation_id: &str,
) -> Result<()> {
    let ty = action.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let now = now_rfc3339();
    match ty {
        "move" => {
            let list_id = action
                .get("list_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Validation("move: list_id requis".into()))?;
            let owner: Option<String> = conn
                .query_row("SELECT board_id FROM lists WHERE id=?1", [list_id], |r| r.get(0))
                .optional()?;
            if owner.as_deref() == Some(card.board_id.as_str()) {
                let pos = next_card_position(conn, list_id)?;
                conn.execute(
                    "UPDATE cards SET list_id=?2, position=?3, updated_at=?4 WHERE id=?1",
                    params![card.id, list_id, pos, now],
                )?;
            }
        }
        "label" => {
            let label_id = action
                .get("label_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Validation("label: label_id requis".into()))?;
            conn.execute(
                "INSERT OR IGNORE INTO card_labels(card_id,label_id) VALUES(?1,?2)",
                params![card.id, label_id],
            )?;
        }
        "due" => {
            let offset = action.get("offset_days").and_then(|v| v.as_i64()).unwrap_or(0);
            let time = action.get("time").and_then(|v| v.as_str());
            let d = crate::time::add_days(today(), offset);
            let due = match time {
                Some(t) => crate::time::date_with_time(d, Some(t))
                    .format("%Y-%m-%dT%H:%M:%S")
                    .to_string(),
                None => d.to_string(),
            };
            conn.execute(
                "UPDATE cards SET due=?2, updated_at=?3 WHERE id=?1",
                params![card.id, due, now],
            )?;
        }
        "priority" => {
            let value = action
                .get("value")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Validation("priority: value requis".into()))?;
            if PRIORITIES.contains(&value) {
                conn.execute(
                    "UPDATE cards SET priority=?2, updated_at=?3 WHERE id=?1",
                    params![card.id, value, now],
                )?;
            }
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
        }
        "archive" => {
            conn.execute(
                "UPDATE cards SET archived=1, updated_at=?2 WHERE id=?1",
                params![card.id, now],
            )?;
        }
        other => {
            return Err(Error::Validation(format!(
                "action d'automatisation inconnue: {other}"
            )))
        }
    }
    let _ = automation_id;
    Ok(())
}

fn validate_actions(actions: &str) -> Result<()> {
    let parsed: Vec<serde_json::Value> = serde_json::from_str(actions)
        .map_err(|e| Error::Validation(format!("actions JSON invalides: {e}")))?;
    for a in &parsed {
        let ty = a.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if !["move", "label", "due", "priority", "check", "archive"].contains(&ty) {
            return Err(Error::Validation(format!("action inconnue: {ty}")));
        }
    }
    Ok(())
}
