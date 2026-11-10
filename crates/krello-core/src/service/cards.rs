//! Card CRUD, floating-position moves (incl. cross-board), duplication, deletion.

use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};

use super::{log_activity, next_card_position, now_updated, renormalize_cards, Service};
use crate::error::{Error, Result};
use crate::events::Event;
use crate::ids::new_id;
use crate::models::*;
use crate::time::{now_rfc3339, parse_date, parse_dt};
use crate::undo::*;

const EPS: f64 = 1e-9;

impl Service {
    pub fn create_card(&self, board_id: &str, c: CreateCard) -> Result<Card> {
        if c.title.trim().is_empty() {
            return Err(Error::Validation("le titre de la carte est requis".into()));
        }
        let mut card = Card {
            id: new_id("card"),
            board_id: board_id.to_string(),
            list_id: c.list_id.clone(),
            title: c.title.trim().to_string(),
            description: c.description,
            position: 0.0,
            archived: false,
            cover: c.cover,
            start: c.start,
            due: c.due,
            due_complete: c.due_complete.unwrap_or(false),
            reminder: c.reminder,
            recurrence: c.recurrence,
            priority: c.priority.unwrap_or_else(|| "none".into()),
            is_milestone: c.is_milestone.unwrap_or(false),
            duration_days: c.duration_days,
            work_hours: c.work_hours,
            pct_complete: c.pct_complete.unwrap_or(0).clamp(0, 100),
            task_type: c.task_type.unwrap_or_else(|| "fixed_duration".into()),
            units: c.units.unwrap_or(1.0),
            hours_per_day: c.hours_per_day.unwrap_or(8.0),
            constraint_type: c.constraint_type.unwrap_or_else(|| "asap".into()),
            constraint_date: c.constraint_date,
            wbs: String::new(),
            cost: c.cost.unwrap_or(0.0),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
            completed_at: None,
        };
        if card.due_complete {
            card.completed_at = Some(now_rfc3339());
        }
        validate_card(&card)?;

        {
            let db = self.lock_db()?;
            let list = query_list(&db.conn, &c.list_id)?
                .ok_or_else(|| Error::NotFound(format!("list {}", c.list_id)))?;
            if list.board_id != board_id {
                return Err(Error::Validation(
                    "la liste n'appartient pas à ce board".into(),
                ));
            }
            card.position = match c.position {
                Some(p) => p,
                None => next_card_position(&db.conn, &c.list_id)?,
            };
            insert_card_row(&db.conn, &card)?;

            for lid in c.label_ids.iter().flatten() {
                let owner: Option<String> = db
                    .conn
                    .query_row("SELECT board_id FROM labels WHERE id=?1", [lid], |r| r.get(0))
                    .optional()?;
                if owner.as_deref() != Some(board_id) {
                    return Err(Error::Validation(format!("label {lid} hors board")));
                }
                db.conn.execute(
                    "INSERT OR IGNORE INTO card_labels(card_id,label_id) VALUES(?1,?2)",
                    params![&card.id, lid],
                )?;
            }
            if let Some(fields) = &c.custom_fields {
                for (fid, value) in fields {
                    set_field_value_tx(&db.conn, board_id, &card.id, fid, &value_to_string(value))?;
                }
            }
            if let Some(items) = &c.checklist {
                if !items.is_empty() {
                    let cl = Checklist {
                        id: new_id("cl"),
                        card_id: card.id.clone(),
                        title: "Checklist".into(),
                        position: 0.0,
                        created_at: now_rfc3339(),
                    };
                    insert_checklist_row(&db.conn, &cl)?;
                    for (i, title) in items.iter().enumerate() {
                        insert_checklist_item_row(
                            &db.conn,
                            &ChecklistItem {
                                id: new_id("cli"),
                                checklist_id: cl.id.clone(),
                                title: title.clone(),
                                checked: false,
                                position: i as f64,
                                due: None,
                                created_at: now_rfc3339(),
                            },
                        )?;
                    }
                }
            }
            log_activity(
                &db.conn,
                board_id,
                Some(&card.id),
                "card_created",
                &serde_json::json!({ "title": card.title, "list_id": card.list_id }),
            )?;
        }

        self.push_undo(UndoEntry {
            label: format!("Création de la carte « {} »", card.title),
            actions: vec![UndoAction::CardCreated(card.clone())],
        });
        self.emit(Event::CardCreated {
            board_id: board_id.to_string(),
            card_id: card.id.clone(),
        });
        self.run_automations(
            board_id,
            "card_created",
            &serde_json::json!({ "card_id": card.id, "list_id": card.list_id }),
        );
        Ok(card)
    }

    pub fn get_card(&self, id: &str) -> Result<Card> {
        let db = self.lock_db()?;
        query_card(&db.conn, id)?.ok_or_else(|| Error::NotFound(format!("card {id}")))
    }

    pub fn list_cards(&self, board_id: &str, include_archived: bool) -> Result<Vec<Card>> {
        let db = self.lock_db()?;
        let sql = if include_archived {
            "SELECT * FROM cards WHERE board_id=?1 ORDER BY list_id, position"
        } else {
            "SELECT * FROM cards WHERE board_id=?1 AND archived=0 ORDER BY list_id, position"
        };
        let mut stmt = db.conn.prepare(sql)?;
        let cards = stmt
            .query_map([board_id], row_card)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(cards)
    }

    pub fn update_card(&self, id: &str, u: UpdateCard) -> Result<Card> {
        let db = self.lock_db()?;
        let before =
            query_card(&db.conn, id)?.ok_or_else(|| Error::NotFound(format!("card {id}")))?;
        let mut after = before.clone();
        if let Some(v) = &u.title {
            if v.trim().is_empty() {
                return Err(Error::Validation("le titre ne peut pas être vide".into()));
            }
            after.title = v.trim().to_string();
        }
        if let Some(v) = u.description {
            after.description = v;
        }
        if u.cover.is_some() {
            after.cover = u.cover;
        }
        if u.start.is_some() {
            after.start = u.start;
        }
        if u.due.is_some() {
            after.due = u.due;
        }
        if let Some(v) = u.reminder {
            after.reminder = Some(v);
        }
        if u.recurrence.is_some() {
            after.recurrence = u.recurrence;
        }
        if let Some(v) = u.priority {
            after.priority = v;
        }
        if let Some(v) = u.is_milestone {
            after.is_milestone = v;
        }
        if u.duration_days.is_some() {
            after.duration_days = u.duration_days;
        }
        if u.work_hours.is_some() {
            after.work_hours = u.work_hours;
        }
        if let Some(v) = u.pct_complete {
            after.pct_complete = v.clamp(0, 100);
        }
        if let Some(v) = u.task_type {
            after.task_type = v;
        }
        if let Some(v) = u.units {
            after.units = v;
        }
        if let Some(v) = u.hours_per_day {
            after.hours_per_day = v;
        }
        if let Some(v) = u.constraint_type {
            after.constraint_type = v;
        }
        if u.constraint_date.is_some() {
            after.constraint_date = u.constraint_date;
        }
        if let Some(v) = u.cost {
            after.cost = v;
        }
        if let Some(v) = u.archived {
            after.archived = v;
        }
        if let Some(v) = u.due_complete {
            if v != after.due_complete {
                after.due_complete = v;
                after.completed_at = if v { Some(now_rfc3339()) } else { None };
            }
        }
        validate_card(&after)?;
        after.updated_at = now_updated();
        update_card_row(&db.conn, &after)?;

        let schedule_changed = before.start != after.start
            || before.due != after.due
            || before.constraint_type != after.constraint_type
            || before.constraint_date != after.constraint_date
            || before.task_type != after.task_type
            || before.duration_days != after.duration_days
            || before.work_hours != after.work_hours
            || before.units != after.units
            || before.hours_per_day != after.hours_per_day
            || before.is_milestone != after.is_milestone;
        let board_id = after.board_id.clone();
        let entry = UndoEntry {
            label: format!("Modification de la carte « {} »", after.title),
            actions: vec![UndoAction::CardUpdated {
                before,
                after: after.clone(),
            }],
        };
        drop(db);
        if schedule_changed {
            self.recompute_and_notify(&board_id)?;
        }
        self.push_undo(entry);
        self.emit(Event::CardUpdated {
            board_id: board_id.clone(),
            card_id: id.to_string(),
        });
        Ok(after)
    }

    /// Move a card within its list, to another list, or to another board.
    pub fn move_card(&self, id: &str, m: MoveCard) -> Result<Card> {
        let before = {
            let db = self.lock_db()?;
            query_card(&db.conn, id)?.ok_or_else(|| Error::NotFound(format!("card {id}")))?
        };
        let target_list_id = m.list_id.clone().unwrap_or_else(|| before.list_id.clone());
        let mut after = before.clone();

        {
            let db = self.lock_db()?;
            let target = query_list(&db.conn, &target_list_id)?
                .ok_or_else(|| Error::NotFound(format!("list {target_list_id}")))?;
            let cross_board = target.board_id != before.board_id;
            if cross_board {
                if target.archived {
                    return Err(Error::Validation("liste archivée".into()));
                }
                // Cross-board move drops board-scoped labels and custom fields.
                db.conn
                    .execute("DELETE FROM card_labels WHERE card_id=?1", [id])?;
                db.conn
                    .execute("DELETE FROM custom_field_values WHERE card_id=?1", [id])?;
                db.conn
                    .execute("DELETE FROM schedule_cache WHERE card_id=?1", [id])?;
                after.board_id = target.board_id.clone();
            }

            let new_position = if let Some(p) = m.position {
                p
            } else if let Some(before_id) = &m.before {
                let anchor: Option<(f64, String)> = db
                    .conn
                    .query_row(
                        "SELECT position, list_id FROM cards WHERE id=?1",
                        [before_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                let Some((anchor_pos, anchor_list)) = anchor else {
                    return Err(Error::NotFound(format!("card {before_id}")));
                };
                if anchor_list != target_list_id {
                    return Err(Error::Validation(
                        "la carte de référence n'est pas dans la liste cible".into(),
                    ));
                }
                let prev: Option<f64> = db
                    .conn
                    .query_row(
                        "SELECT MAX(position) FROM cards WHERE list_id=?1 AND id != ?2 AND position < ?3",
                        params![target_list_id, id, anchor_pos],
                        |r| r.get(0),
                    )
                    .optional()?
                    .flatten();
                match prev {
                    Some(p) => (p + anchor_pos) / 2.0,
                    None => anchor_pos - 1.0,
                }
            } else if let Some(after_id) = &m.after {
                let anchor: Option<(f64, String)> = db
                    .conn
                    .query_row(
                        "SELECT position, list_id FROM cards WHERE id=?1",
                        [after_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                let Some((anchor_pos, anchor_list)) = anchor else {
                    return Err(Error::NotFound(format!("card {after_id}")));
                };
                if anchor_list != target_list_id {
                    return Err(Error::Validation(
                        "la carte de référence n'est pas dans la liste cible".into(),
                    ));
                }
                let next: Option<f64> = db
                    .conn
                    .query_row(
                        "SELECT MIN(position) FROM cards WHERE list_id=?1 AND id != ?2 AND position > ?3",
                        params![target_list_id, id, anchor_pos],
                        |r| r.get(0),
                    )
                    .optional()?
                    .flatten();
                match next {
                    Some(n) => (n + anchor_pos) / 2.0,
                    None => anchor_pos + 1.0,
                }
            } else {
                let max: Option<f64> = db
                    .conn
                    .query_row(
                        "SELECT MAX(position) FROM cards WHERE list_id=?1 AND id != ?2",
                        params![target_list_id, id],
                        |r| r.get(0),
                    )
                    .optional()?
                    .flatten();
                max.unwrap_or(-1.0) + 1.0
            };

            // Collision → renormalize the list and recompute a collision-free position.
            let collision: i64 = db.conn.query_row(
                "SELECT COUNT(*) FROM cards WHERE list_id=?1 AND id != ?2 AND ABS(position - ?3) < ?4",
                params![target_list_id, id, new_position, EPS],
                |r| r.get(0),
            )?;
            if collision > 0 {
                renormalize_cards(&db.conn, &target_list_id)?;
            }
            after.list_id = target_list_id.clone();
            after.position = new_position;
            after.updated_at = now_updated();
            update_card_row(&db.conn, &after)?;
        }

        let from = before.list_id.clone();
        let to = after.list_id.clone();
        let board_id = after.board_id.clone();
        let entry = UndoEntry {
            label: format!("Déplacement de la carte « {} »", after.title),
            actions: vec![UndoAction::CardUpdated {
                before,
                after: after.clone(),
            }],
        };
        self.recompute_and_notify(&board_id)?;
        self.push_undo(entry);
        self.emit(Event::CardMoved {
            board_id: board_id.clone(),
            card_id: id.to_string(),
        });
        self.run_automations(
            &board_id,
            "card_moved",
            &serde_json::json!({ "card_id": id, "from_list_id": from, "to_list_id": to }),
        );
        Ok(after)
    }

    /// Deep-copy a card in the same list, right below the source.
    pub fn duplicate_card(&self, id: &str) -> Result<Card> {
        let db = self.lock_db()?;
        let src =
            query_card(&db.conn, id)?.ok_or_else(|| Error::NotFound(format!("card {id}")))?;
        let max_below: Option<f64> = db
            .conn
            .query_row(
                "SELECT MIN(position) FROM cards WHERE list_id=?1 AND position > ?2",
                params![src.list_id, src.position],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        let new_position = match max_below {
            Some(n) => (n + src.position) / 2.0,
            None => src.position + 1.0,
        };
        let mut card = src.clone();
        card.id = new_id("card");
        card.title = format!("{} (copie)", src.title);
        card.position = new_position;
        card.completed_at = None;
        card.created_at = now_rfc3339();
        card.updated_at = now_rfc3339();
        insert_card_row(&db.conn, &card)?;

        let lids = label_ids_for_card(&db.conn, id)?;
        for lid in lids {
            db.conn.execute(
                "INSERT OR IGNORE INTO card_labels(card_id,label_id) VALUES(?1,?2)",
                params![&card.id, lid],
            )?;
        }
        let mut stmt = db
            .conn
            .prepare("SELECT * FROM checklists WHERE card_id=?1 ORDER BY position")?;
        let cls: Vec<Checklist> = stmt
            .query_map([id], row_checklist)?
            .collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        for cl in cls {
            let ncl = Checklist {
                id: new_id("cl"),
                card_id: card.id.clone(),
                ..cl.clone()
            };
            insert_checklist_row(&db.conn, &ncl)?;
            let mut stmt = db.conn.prepare(
                "SELECT * FROM checklist_items WHERE checklist_id=?1 ORDER BY position",
            )?;
            let items: Vec<ChecklistItem> = stmt
                .query_map([&cl.id], row_checklist_item)?
                .collect::<rusqlite::Result<_>>()?;
            drop(stmt);
            for it in items {
                insert_checklist_item_row(
                    &db.conn,
                    &ChecklistItem {
                        id: new_id("cli"),
                        checklist_id: ncl.id.clone(),
                        ..it
                    },
                )?;
            }
        }
        let mut stmt = db.conn.prepare(
            "SELECT card_id, field_id, value FROM custom_field_values WHERE card_id=?1",
        )?;
        let fvs: Vec<CustomFieldValue> = stmt
            .query_map([id], row_field_value)?
            .collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        for fv in fvs {
            db.conn.execute(
                "INSERT OR IGNORE INTO custom_field_values(card_id,field_id,value) VALUES(?1,?2,?3)",
                params![&card.id, fv.field_id, fv.value],
            )?;
        }
        log_activity(
            &db.conn,
            &card.board_id,
            Some(&card.id),
            "card_duplicated",
            &serde_json::json!({ "source": id }),
        )?;
        drop(db);

        self.push_undo(UndoEntry {
            label: format!("Duplication de la carte « {} »", src.title),
            actions: vec![UndoAction::CardCreated(card.clone())],
        });
        self.emit(Event::CardCreated {
            board_id: card.board_id.clone(),
            card_id: card.id.clone(),
        });
        Ok(card)
    }

    pub fn delete_card(&self, id: &str) -> Result<()> {
        let db = self.lock_db()?;
        let snap = snapshot_card(&db.conn, id)?;
        let entry = UndoEntry {
            label: format!("Suppression de la carte « {} »", snap.card.title),
            actions: vec![UndoAction::CardDeleted(snap.clone())],
        };
        db.conn.execute("DELETE FROM schedule_cache WHERE card_id=?1", [id])?;
        db.conn.execute("DELETE FROM cards WHERE id=?1", [id])?;
        log_activity(
            &db.conn,
            &snap.card.board_id,
            None,
            "card_deleted",
            &serde_json::json!({ "title": snap.card.title }),
        )?;
        drop(db);
        self.recompute_and_notify(&snap.card.board_id)?;
        self.push_undo(entry);
        self.emit(Event::CardDeleted {
            board_id: snap.card.board_id.clone(),
            card_id: id.to_string(),
        });
        Ok(())
    }

    /// Board-scoped view data for one card: labels, checklists, comments,
    /// custom field values, dependencies, attachments.
    pub fn card_detail(&self, id: &str) -> Result<serde_json::Value> {
        let card = self.get_card(id)?;
        let db = self.lock_db()?;
        let labels: Vec<Label> = {
            let mut stmt = db.conn.prepare(
                "SELECT l.* FROM labels l JOIN card_labels cl ON cl.label_id = l.id WHERE cl.card_id=?1",
            )?;
            let v = stmt
                .query_map([id], row_label)?
                .collect::<rusqlite::Result<_>>()?;
            v
        };
        let checklists: Vec<serde_json::Value> = {
            let mut stmt = db
                .conn
                .prepare("SELECT * FROM checklists WHERE card_id=?1 ORDER BY position")?;
            let cls: Vec<Checklist> = stmt
                .query_map([id], row_checklist)?
                .collect::<rusqlite::Result<_>>()?;
            drop(stmt);
            let mut out = Vec::with_capacity(cls.len());
            for cl in cls {
                let items: Vec<ChecklistItem> = {
                    let mut s = db.conn.prepare(
                        "SELECT * FROM checklist_items WHERE checklist_id=?1 ORDER BY position",
                    )?;
                    let v = s
                        .query_map([&cl.id], row_checklist_item)?
                        .collect::<rusqlite::Result<_>>()?;
                    v
                };
                out.push(serde_json::to_value(ChecklistWithItems { checklist: cl, items })
                    .unwrap_or_default());
            }
            out
        };
        let comments: Vec<Comment> = {
            let mut stmt =
                db.conn.prepare("SELECT * FROM comments WHERE card_id=?1 ORDER BY created_at")?;
            let v = stmt
                .query_map([id], row_comment)?
                .collect::<rusqlite::Result<_>>()?;
            v
        };
        let attachments: Vec<Attachment> = {
            let mut stmt =
                db.conn.prepare("SELECT * FROM attachments WHERE card_id=?1 ORDER BY created_at")?;
            let v = stmt
                .query_map([id], row_attachment)?
                .collect::<rusqlite::Result<_>>()?;
            v
        };
        let custom_fields: serde_json::Map<String, serde_json::Value> = {
            let mut stmt = db.conn.prepare(
                "SELECT field_id, value FROM custom_field_values WHERE card_id=?1",
            )?;
            let pairs: Vec<(String, String)> = {
                let v = stmt
                    .query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .collect::<rusqlite::Result<_>>()?;
                v
            };
            drop(stmt);
            pairs
                .into_iter()
                .map(|(k, v)| (k, serde_json::Value::from(v)))
                .collect()
        };
        let deps_out: Vec<Dependency> = {
            let mut stmt =
                db.conn.prepare("SELECT * FROM dependencies WHERE predecessor_id=?1")?;
            let v = stmt
                .query_map([id], row_dependency)?
                .collect::<rusqlite::Result<_>>()?;
            v
        };
        let deps_in: Vec<Dependency> = {
            let mut stmt =
                db.conn.prepare("SELECT * FROM dependencies WHERE successor_id=?1")?;
            let v = stmt
                .query_map([id], row_dependency)?
                .collect::<rusqlite::Result<_>>()?;
            v
        };
        let schedule: Option<ScheduleEntry> = {
            let mut stmt = db
                .conn
                .prepare("SELECT * FROM schedule_cache WHERE card_id=?1")?;
            stmt.query_row([id], row_schedule).optional()?
        };
        Ok(serde_json::json!({
            "card": card,
            "labels": labels,
            "checklists": checklists,
            "comments": comments,
            "attachments": attachments,
            "custom_fields": custom_fields,
            "dependencies_out": deps_out,
            "dependencies_in": deps_in,
            "schedule": schedule,
        }))
    }
}

#[derive(serde::Serialize)]
struct ChecklistWithItems {
    #[serde(flatten)]
    checklist: Checklist,
    items: Vec<ChecklistItem>,
}

fn value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn validate_card(c: &Card) -> Result<()> {
    if !PRIORITIES.contains(&c.priority.as_str()) {
        return Err(Error::Validation(format!("priorité invalide: {}", c.priority)));
    }
    if !TASK_TYPES.contains(&c.task_type.as_str()) {
        return Err(Error::Validation(format!("type de tâche invalide: {}", c.task_type)));
    }
    if !CONSTRAINT_TYPES.contains(&c.constraint_type.as_str()) {
        return Err(Error::Validation(format!(
            "contrainte invalide: {}",
            c.constraint_type
        )));
    }
    if !(0..=100).contains(&c.pct_complete) {
        return Err(Error::Validation("% accompli doit être entre 0 et 100".into()));
    }
    if let Some(s) = &c.start {
        if parse_date(s).is_none() && parse_dt(s).is_none() {
            return Err(Error::Validation(format!("date de début invalide: {s}")));
        }
    }
    if let Some(d) = &c.due {
        if parse_date(d).is_none() && parse_dt(d).is_none() {
            return Err(Error::Validation(format!("date d'échéance invalide: {d}")));
        }
    }
    if let Some(d) = &c.constraint_date {
        if parse_date(d).is_none() {
            return Err(Error::Validation(format!(
                "date de contrainte invalide: {d}"
            )));
        }
    }
    if let Some(days) = c.duration_days {
        if days < 0.0 {
            return Err(Error::Validation("la durée ne peut pas être négative".into()));
        }
    }
    if c.units < 0.0 || c.hours_per_day <= 0.0 {
        return Err(Error::Validation("unités/heures par jour invalides".into()));
    }
    Ok(())
}

/// Board-scoped helper used at creation time (same tx as the insert).
pub(crate) fn set_field_value_tx(
    conn: &rusqlite::Connection,
    board_id: &str,
    card_id: &str,
    field_id: &str,
    value: &str,
) -> Result<()> {
    let def: Option<(String, String, String)> = conn
        .query_row(
            "SELECT board_id, type, options FROM custom_field_defs WHERE id=?1",
            [field_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((owner, kind, options)) = def else {
        return Err(Error::NotFound(format!("champ {field_id}")));
    };
    if owner != board_id {
        return Err(Error::Validation("champ hors board".into()));
    }
    validate_field_value(&kind, &options, value)?;
    if value.is_empty() {
        conn.execute(
            "DELETE FROM custom_field_values WHERE card_id=?1 AND field_id=?2",
            params![card_id, field_id],
        )?;
    } else {
        conn.execute(
            "INSERT INTO custom_field_values(card_id,field_id,value) VALUES(?1,?2,?3)
             ON CONFLICT(card_id, field_id) DO UPDATE SET value=excluded.value",
            params![card_id, field_id, value],
        )?;
    }
    Ok(())
}

pub(crate) fn validate_field_value(kind: &str, options: &str, value: &str) -> Result<()> {
    match kind {
        "number" => {
            if !value.is_empty() && value.parse::<f64>().is_err() {
                return Err(Error::Validation(format!("nombre invalide: {value}")));
            }
        }
        "date" => {
            if !value.is_empty() && parse_date(value).is_none() && parse_dt(value).is_none() {
                return Err(Error::Validation(format!("date invalide: {value}")));
            }
        }
        "checkbox" => {
            if !value.is_empty() && value != "true" && value != "false" {
                return Err(Error::Validation("case à cocher: true|false attendu".into()));
            }
        }
        "select" => {
            if !value.is_empty() {
                let opts: Vec<String> = serde_json::from_str(options)
                    .map_err(|e| Error::Validation(format!("options invalides: {e}")))?;
                if !opts.contains(&value.to_string()) {
                    return Err(Error::Validation(format!(
                        "« {value} » n'est pas une option valide"
                    )));
                }
            }
        }
        "text" => {}
        other => return Err(Error::Validation(format!("type de champ inconnu: {other}"))),
    }
    Ok(())
}

/// Positions of all cards per list — useful for optimistic client moves.
pub fn positions_by_list(cards: &[Card]) -> HashMap<String, Vec<&Card>> {
    let mut map: HashMap<String, Vec<&Card>> = HashMap::new();
    for c in cards {
        map.entry(c.list_id.clone()).or_default().push(c);
    }
    for v in map.values_mut() {
        v.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap_or(std::cmp::Ordering::Equal));
    }
    map
}
