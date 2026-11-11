//! Undo/redo: snapshot-based declarative undo entries applied as inverse operations.

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{Error, Result};
use crate::models::*;
use crate::time::now_rfc3339;

pub const UNDO_CAP: usize = 100;

#[derive(Debug, Default)]
pub struct UndoState {
    pub undo: Vec<UndoEntry>,
    pub redo: Vec<UndoEntry>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UndoEntry {
    pub label: String,
    pub actions: Vec<UndoAction>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum UndoAction {
    BoardCreated(Board),
    BoardUpdated { before: Board, after: Board },
    BoardDeleted {
        board: Board,
        lists: Vec<List>,
        cards: Vec<CardSnapshot>,
    },
    ListCreated(List),
    ListUpdated { before: List, after: List },
    ListDeleted { list: List, cards: Vec<CardSnapshot> },
    CardCreated(Card),
    CardUpdated { before: Card, after: Card },
    CardDeleted(CardSnapshot),
    LabelsChanged {
        card_id: String,
        before: Vec<String>,
        after: Vec<String>,
    },
    FieldSet {
        card_id: String,
        field_id: String,
        before: Option<String>,
        after: Option<String>,
    },
    FieldDefCreated(CustomFieldDef),
    FieldDefDeleted(CustomFieldDef),
    DependencyCreated(Dependency),
    DependencyDeleted(Dependency),
    DependencyUpdated {
        before: Dependency,
        after: Dependency,
    },
    ChecklistCreated(Checklist),
    ChecklistDeleted { checklist: Checklist, items: Vec<ChecklistItem> },
    ChecklistItemCreated(ChecklistItem),
    ChecklistItemUpdated { before: ChecklistItem, after: ChecklistItem },
    ChecklistItemDeleted(ChecklistItem),
    CommentCreated(Comment),
    CommentDeleted(Comment),
    AutomationCreated(Automation),
    AutomationDeleted(Automation),
    AutomationUpdated { before: Automation, after: Automation },
    LabelCreated(Label),
    LabelUpdated { before: Label, after: Label },
    LabelDeleted(Label),
    FieldDefUpdated { before: CustomFieldDef, after: CustomFieldDef },
}

/// Everything needed to restore a deleted card (row + children).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CardSnapshot {
    pub card: Card,
    pub label_ids: Vec<String>,
    pub checklists: Vec<(Checklist, Vec<ChecklistItem>)>,
    pub comments: Vec<Comment>,
    pub field_values: Vec<CustomFieldValue>,
    pub deps_out: Vec<Dependency>,
    pub deps_in: Vec<Dependency>,
    pub attachments: Vec<Attachment>,
}

pub fn snapshot_card(conn: &Connection, card_id: &str) -> Result<CardSnapshot> {
    let card = query_card(conn, card_id)?
        .ok_or_else(|| Error::NotFound(format!("card {card_id}")))?;
    let label_ids = label_ids_for_card(conn, card_id)?;
    let mut checklists = Vec::new();
    {
        let mut stmt =
            conn.prepare("SELECT * FROM checklists WHERE card_id=?1 ORDER BY position")?;
        let cls: Vec<Checklist> = stmt
            .query_map([card_id], row_checklist)?
            .collect::<rusqlite::Result<_>>()?;
        for cl in cls {
            let mut stmt =
                conn.prepare("SELECT * FROM checklist_items WHERE checklist_id=?1 ORDER BY position")?;
            let items: Vec<ChecklistItem> = stmt
                .query_map([&cl.id], row_checklist_item)?
                .collect::<rusqlite::Result<_>>()?;
            checklists.push((cl, items));
        }
    }
    let comments: Vec<Comment> = {
        let mut stmt = conn.prepare("SELECT * FROM comments WHERE card_id=?1 ORDER BY created_at")?;
        let v = stmt
            .query_map([card_id], row_comment)?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    let field_values: Vec<CustomFieldValue> = {
        let mut stmt =
            conn.prepare("SELECT card_id, field_id, value FROM custom_field_values WHERE card_id=?1")?;
        let v = stmt
            .query_map([card_id], row_field_value)?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    let deps_out: Vec<Dependency> = {
        let mut stmt = conn.prepare("SELECT * FROM dependencies WHERE predecessor_id=?1")?;
        let v = stmt
            .query_map([card_id], row_dependency)?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    let deps_in: Vec<Dependency> = {
        let mut stmt = conn.prepare("SELECT * FROM dependencies WHERE successor_id=?1")?;
        let v = stmt
            .query_map([card_id], row_dependency)?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    let attachments: Vec<Attachment> = {
        let mut stmt = conn.prepare("SELECT * FROM attachments WHERE card_id=?1")?;
        let v = stmt
            .query_map([card_id], row_attachment)?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    Ok(CardSnapshot {
        card,
        label_ids,
        checklists,
        comments,
        field_values,
        deps_out,
        deps_in,
        attachments,
    })
}

pub fn restore_card(conn: &Connection, snap: &CardSnapshot) -> Result<()> {
    insert_card_row(conn, &snap.card)?;
    for lid in &snap.label_ids {
        conn.execute(
            "INSERT OR IGNORE INTO card_labels(card_id, label_id) VALUES(?1, ?2)",
            params![snap.card.id, lid],
        )?;
    }
    for (cl, items) in &snap.checklists {
        insert_checklist_row(conn, cl)?;
        for it in items {
            insert_checklist_item_row(conn, it)?;
        }
    }
    for c in &snap.comments {
        insert_comment_row(conn, c)?;
    }
    for fv in &snap.field_values {
        conn.execute(
            "INSERT OR IGNORE INTO custom_field_values(card_id, field_id, value) VALUES(?1,?2,?3)",
            params![fv.card_id, fv.field_id, fv.value],
        )?;
    }
    for d in snap.deps_out.iter().chain(snap.deps_in.iter()) {
        insert_dependency_row(conn, d)?;
    }
    for a in &snap.attachments {
        insert_attachment_row(conn, a)?;
    }
    Ok(())
}

pub fn delete_card_cascade(conn: &Connection, card_id: &str) -> Result<()> {
    conn.execute("DELETE FROM cards WHERE id=?1", [card_id])?;
    Ok(())
}

// ---------- row mappers ----------

pub fn row_board(r: &rusqlite::Row<'_>) -> rusqlite::Result<Board> {
    Ok(Board {
        id: r.get("id")?,
        title: r.get("title")?,
        description: r.get("description")?,
        background: r.get("background")?,
        calendar_id: r.get("calendar_id")?,
        start_date: r.get("start_date")?,
        deadline: r.get("deadline")?,
        settings: r.get("settings")?,
        archived: r.get::<_, i64>("archived")? != 0,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}

pub fn row_list(r: &rusqlite::Row<'_>) -> rusqlite::Result<List> {
    Ok(List {
        id: r.get("id")?,
        board_id: r.get("board_id")?,
        title: r.get("title")?,
        position: r.get("position")?,
        wip_limit: r.get("wip_limit")?,
        archived: r.get::<_, i64>("archived")? != 0,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}

pub fn row_card(r: &rusqlite::Row<'_>) -> rusqlite::Result<Card> {
    Ok(Card {
        id: r.get("id")?,
        board_id: r.get("board_id")?,
        list_id: r.get("list_id")?,
        title: r.get("title")?,
        description: r.get("description")?,
        position: r.get("position")?,
        archived: r.get::<_, i64>("archived")? != 0,
        is_template: r.get::<_, i64>("is_template")? != 0,
        template_description: r.get("template_description")?,
        cover: r.get("cover")?,
        start: r.get("start")?,
        due: r.get("due")?,
        due_complete: r.get::<_, i64>("due_complete")? != 0,
        reminder: r.get("reminder")?,
        recurrence: r.get("recurrence")?,
        priority: r.get("priority")?,
        is_milestone: r.get::<_, i64>("is_milestone")? != 0,
        duration_days: r.get("duration_days")?,
        work_hours: r.get("work_hours")?,
        pct_complete: r.get("pct_complete")?,
        task_type: r.get("task_type")?,
        units: r.get("units")?,
        hours_per_day: r.get("hours_per_day")?,
        constraint_type: r.get("constraint_type")?,
        constraint_date: r.get("constraint_date")?,
        wbs: r.get("wbs")?,
        cost: r.get("cost")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
        completed_at: r.get("completed_at")?,
    })
}

pub fn row_label(r: &rusqlite::Row<'_>) -> rusqlite::Result<Label> {
    Ok(Label {
        id: r.get("id")?,
        board_id: r.get("board_id")?,
        name: r.get("name")?,
        color: r.get("color")?,
        created_at: r.get("created_at")?,
    })
}

pub fn row_checklist(r: &rusqlite::Row<'_>) -> rusqlite::Result<Checklist> {
    Ok(Checklist {
        id: r.get("id")?,
        card_id: r.get("card_id")?,
        title: r.get("title")?,
        position: r.get("position")?,
        created_at: r.get("created_at")?,
    })
}

pub fn row_checklist_item(r: &rusqlite::Row<'_>) -> rusqlite::Result<ChecklistItem> {
    Ok(ChecklistItem {
        id: r.get("id")?,
        checklist_id: r.get("checklist_id")?,
        title: r.get("title")?,
        checked: r.get::<_, i64>("checked")? != 0,
        position: r.get("position")?,
        due: r.get("due")?,
        created_at: r.get("created_at")?,
    })
}

pub fn row_field_def(r: &rusqlite::Row<'_>) -> rusqlite::Result<CustomFieldDef> {
    Ok(CustomFieldDef {
        id: r.get("id")?,
        board_id: r.get("board_id")?,
        name: r.get("name")?,
        kind: r.get("type")?,
        options: r.get("options")?,
        position: r.get("position")?,
        created_at: r.get("created_at")?,
    })
}

pub fn row_field_value(r: &rusqlite::Row<'_>) -> rusqlite::Result<CustomFieldValue> {
    Ok(CustomFieldValue {
        card_id: r.get(0)?,
        field_id: r.get(1)?,
        value: r.get(2)?,
    })
}

pub fn row_attachment(r: &rusqlite::Row<'_>) -> rusqlite::Result<Attachment> {
    Ok(Attachment {
        id: r.get("id")?,
        card_id: r.get("card_id")?,
        filename: r.get("filename")?,
        stored_path: r.get("stored_path")?,
        mime: r.get("mime")?,
        size: r.get("size")?,
        is_cover: r.get::<_, i64>("is_cover")? != 0,
        created_at: r.get("created_at")?,
    })
}

pub fn row_comment(r: &rusqlite::Row<'_>) -> rusqlite::Result<Comment> {
    Ok(Comment {
        id: r.get("id")?,
        card_id: r.get("card_id")?,
        body: r.get("body")?,
        created_at: r.get("created_at")?,
    })
}

pub fn row_activity(r: &rusqlite::Row<'_>) -> rusqlite::Result<Activity> {
    Ok(Activity {
        id: r.get("id")?,
        board_id: r.get("board_id")?,
        card_id: r.get("card_id")?,
        kind: r.get("kind")?,
        payload: r.get("payload")?,
        created_at: r.get("created_at")?,
    })
}

pub fn row_dependency(r: &rusqlite::Row<'_>) -> rusqlite::Result<Dependency> {
    Ok(Dependency {
        id: r.get("id")?,
        board_id: r.get("board_id")?,
        predecessor_id: r.get("predecessor_id")?,
        successor_id: r.get("successor_id")?,
        kind: r.get("type")?,
        lag_days: r.get("lag_days")?,
        created_at: r.get("created_at")?,
    })
}

pub fn row_schedule(r: &rusqlite::Row<'_>) -> rusqlite::Result<ScheduleEntry> {
    Ok(ScheduleEntry {
        card_id: r.get("card_id")?,
        es: r.get("es")?,
        ef: r.get("ef")?,
        ls: r.get("ls")?,
        lf: r.get("lf")?,
        total_float_days: r.get("total_float_days")?,
        is_critical: r.get::<_, i64>("is_critical")? != 0,
    })
}

pub fn row_baseline(r: &rusqlite::Row<'_>) -> rusqlite::Result<Baseline> {
    Ok(Baseline {
        id: r.get("id")?,
        board_id: r.get("board_id")?,
        name: r.get("name")?,
        created_at: r.get("created_at")?,
    })
}

pub fn row_calendar(r: &rusqlite::Row<'_>) -> rusqlite::Result<Calendar> {
    Ok(Calendar {
        id: r.get("id")?,
        name: r.get("name")?,
        weekends: r.get("weekends")?,
        holidays: r.get("holidays")?,
        hours_per_day: r.get("hours_per_day")?,
        created_at: r.get("created_at")?,
    })
}

pub fn row_automation(r: &rusqlite::Row<'_>) -> rusqlite::Result<Automation> {
    Ok(Automation {
        id: r.get("id")?,
        board_id: r.get("board_id")?,
        name: r.get("name")?,
        enabled: r.get::<_, i64>("enabled")? != 0,
        trigger: r.get("trigger")?,
        conditions: r.get("conditions")?,
        actions: r.get("actions")?,
        created_at: r.get("created_at")?,
    })
}

// ---------- raw row writers ----------

pub fn insert_board_row(conn: &Connection, b: &Board) -> Result<()> {
    conn.execute(
        "INSERT INTO boards(id,title,description,background,calendar_id,start_date,deadline,settings,archived,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![
            b.id, b.title, b.description, b.background, b.calendar_id, b.start_date, b.deadline,
            b.settings, b.archived as i64, b.created_at, b.updated_at
        ],
    )?;
    Ok(())
}

pub fn update_board_row(conn: &Connection, b: &Board) -> Result<()> {
    conn.execute(
        "UPDATE boards SET title=?2, description=?3, background=?4, calendar_id=?5, start_date=?6,
         deadline=?7, settings=?8, archived=?9, updated_at=?10 WHERE id=?1",
        params![
            b.id, b.title, b.description, b.background, b.calendar_id, b.start_date, b.deadline,
            b.settings, b.archived as i64, b.updated_at
        ],
    )?;
    Ok(())
}

pub fn insert_list_row(conn: &Connection, l: &List) -> Result<()> {
    conn.execute(
        "INSERT INTO lists(id,board_id,title,position,wip_limit,archived,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            l.id, l.board_id, l.title, l.position, l.wip_limit, l.archived as i64,
            l.created_at, l.updated_at
        ],
    )?;
    Ok(())
}

pub fn update_list_row(conn: &Connection, l: &List) -> Result<()> {
    conn.execute(
        "UPDATE lists SET title=?2, position=?3, wip_limit=?4, archived=?5, updated_at=?6 WHERE id=?1",
        params![l.id, l.title, l.position, l.wip_limit, l.archived as i64, l.updated_at],
    )?;
    Ok(())
}

pub fn insert_card_row(conn: &Connection, c: &Card) -> Result<()> {
    conn.execute(
        "INSERT INTO cards(id,board_id,list_id,title,description,position,archived,cover,start,due,
         due_complete,reminder,recurrence,priority,is_milestone,duration_days,work_hours,pct_complete,
         task_type,units,hours_per_day,constraint_type,constraint_date,wbs,cost,created_at,updated_at,completed_at,
         is_template,template_description)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30)",
        params![
            c.id, c.board_id, c.list_id, c.title, c.description, c.position, c.archived as i64,
            c.cover, c.start, c.due, c.due_complete as i64, c.reminder, c.recurrence, c.priority,
            c.is_milestone as i64, c.duration_days, c.work_hours, c.pct_complete, c.task_type,
            c.units, c.hours_per_day, c.constraint_type, c.constraint_date, c.wbs, c.cost,
            c.created_at, c.updated_at, c.completed_at, c.is_template as i64, c.template_description
        ],
    )?;
    Ok(())
}

pub fn update_card_row(conn: &Connection, c: &Card) -> Result<()> {
    conn.execute(
        "UPDATE cards SET board_id=?2, list_id=?3, title=?4, description=?5, position=?6, archived=?7,
         cover=?8, start=?9, due=?10, due_complete=?11, reminder=?12, recurrence=?13, priority=?14,
         is_milestone=?15, duration_days=?16, work_hours=?17, pct_complete=?18, task_type=?19,
         units=?20, hours_per_day=?21, constraint_type=?22, constraint_date=?23, wbs=?24, cost=?25,
         updated_at=?26, completed_at=?27, is_template=?28, template_description=?29 WHERE id=?1",
        params![
            c.id, c.board_id, c.list_id, c.title, c.description, c.position, c.archived as i64,
            c.cover, c.start, c.due, c.due_complete as i64, c.reminder, c.recurrence, c.priority,
            c.is_milestone as i64, c.duration_days, c.work_hours, c.pct_complete, c.task_type,
            c.units, c.hours_per_day, c.constraint_type, c.constraint_date, c.wbs, c.cost,
            c.updated_at, c.completed_at, c.is_template as i64, c.template_description
        ],
    )?;
    Ok(())
}

pub fn insert_label_row(conn: &Connection, l: &Label) -> Result<()> {
    conn.execute(
        "INSERT INTO labels(id,board_id,name,color,created_at) VALUES(?1,?2,?3,?4,?5)",
        params![l.id, l.board_id, l.name, l.color, l.created_at],
    )?;
    Ok(())
}

pub fn insert_checklist_row(conn: &Connection, c: &Checklist) -> Result<()> {
    conn.execute(
        "INSERT INTO checklists(id,card_id,title,position,created_at) VALUES(?1,?2,?3,?4,?5)",
        params![c.id, c.card_id, c.title, c.position, c.created_at],
    )?;
    Ok(())
}

pub fn insert_checklist_item_row(conn: &Connection, i: &ChecklistItem) -> Result<()> {
    conn.execute(
        "INSERT INTO checklist_items(id,checklist_id,title,checked,position,due,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![i.id, i.checklist_id, i.title, i.checked as i64, i.position, i.due, i.created_at],
    )?;
    Ok(())
}

pub fn insert_comment_row(conn: &Connection, c: &Comment) -> Result<()> {
    conn.execute(
        "INSERT INTO comments(id,card_id,body,created_at) VALUES(?1,?2,?3,?4)",
        params![c.id, c.card_id, c.body, c.created_at],
    )?;
    Ok(())
}

pub fn insert_dependency_row(conn: &Connection, d: &Dependency) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO dependencies(id,board_id,predecessor_id,successor_id,type,lag_days,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![d.id, d.board_id, d.predecessor_id, d.successor_id, d.kind, d.lag_days, d.created_at],
    )?;
    Ok(())
}

pub fn insert_attachment_row(conn: &Connection, a: &Attachment) -> Result<()> {
    conn.execute(
        "INSERT INTO attachments(id,card_id,filename,stored_path,mime,size,is_cover,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![a.id, a.card_id, a.filename, a.stored_path, a.mime, a.size, a.is_cover as i64, a.created_at],
    )?;
    Ok(())
}

// ---------- queries ----------

pub fn query_card(conn: &Connection, id: &str) -> Result<Option<Card>> {
    let mut stmt = conn.prepare("SELECT * FROM cards WHERE id=?1")?;
    Ok(stmt.query_row([id], row_card).optional()?)
}

pub fn query_board(conn: &Connection, id: &str) -> Result<Option<Board>> {
    let mut stmt = conn.prepare("SELECT * FROM boards WHERE id=?1")?;
    Ok(stmt.query_row([id], row_board).optional()?)
}

pub fn query_list(conn: &Connection, id: &str) -> Result<Option<List>> {
    let mut stmt = conn.prepare("SELECT * FROM lists WHERE id=?1")?;
    Ok(stmt.query_row([id], row_list).optional()?)
}

pub fn label_ids_for_card(conn: &Connection, card_id: &str) -> Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT label_id FROM card_labels WHERE card_id=?1 ORDER BY label_id")?;
    let ids = stmt
        .query_map([card_id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ids)
}

/// Timestamp used in undo restores must stay stable; new rows on redo get fresh stamps.
pub fn fresh_updated_at(c: &mut Card) {
    c.updated_at = now_rfc3339();
}

impl UndoEntry {
    /// Boards whose schedule may be affected by this entry (for recompute after undo).
    pub fn schedule_board_ids(&self, conn: &Connection) -> Vec<String> {
        let mut out = Vec::new();
        for a in &self.actions {
            match a {
                UndoAction::BoardUpdated { before, .. } => out.push(before.id.clone()),
                UndoAction::ListDeleted { list, .. } => out.push(list.board_id.clone()),
                UndoAction::ListCreated(l) | UndoAction::ListUpdated { before: l, .. } => {
                    out.push(l.board_id.clone())
                }
                UndoAction::CardCreated(c) | UndoAction::CardUpdated { before: c, .. } => {
                    out.push(c.board_id.clone())
                }
                UndoAction::CardDeleted(snap) => out.push(snap.card.board_id.clone()),
                _ => {}
            }
        }
        let _ = conn;
        out.sort();
        out.dedup();
        out
    }

    pub fn apply_undo(&self, conn: &Connection) -> Result<()> {
        for action in &self.actions {
            apply_action_undo(conn, action)?;
        }
        Ok(())
    }

    pub fn apply_redo(&self, conn: &Connection) -> Result<()> {
        for action in &self.actions {
            apply_action_redo(conn, action)?;
        }
        Ok(())
    }
}

fn apply_action_undo(conn: &Connection, a: &UndoAction) -> Result<()> {
    use crate::service::automations::{insert_automation_row, update_automation_row};
    use crate::service::fields::{insert_field_def_row, update_field_def_row};
    use crate::service::labels::update_label_row;
    match a {
        UndoAction::BoardCreated(b) => {
            conn.execute("DELETE FROM boards WHERE id=?1", [&b.id])?;
        }
        UndoAction::BoardUpdated { before, .. } => update_board_row(conn, before)?,
        UndoAction::BoardDeleted { board, lists, cards } => {
            insert_board_row(conn, board)?;
            for l in lists {
                insert_list_row(conn, l)?;
            }
            for snap in cards {
                restore_card(conn, snap)?;
            }
        }
        UndoAction::ListCreated(l) => {
            conn.execute("DELETE FROM lists WHERE id=?1", [&l.id])?;
        }
        UndoAction::ListUpdated { before, .. } => update_list_row(conn, before)?,
        UndoAction::ListDeleted { list, cards } => {
            insert_list_row(conn, list)?;
            for snap in cards {
                restore_card(conn, snap)?;
            }
        }
        UndoAction::CardCreated(c) => delete_card_cascade(conn, &c.id)?,
        UndoAction::CardUpdated { before, .. } => update_card_row(conn, before)?,
        UndoAction::CardDeleted(snap) => restore_card(conn, snap)?,
        UndoAction::LabelsChanged { card_id, before, .. } => {
            conn.execute("DELETE FROM card_labels WHERE card_id=?1", [card_id])?;
            for lid in before {
                conn.execute(
                    "INSERT OR IGNORE INTO card_labels(card_id,label_id) VALUES(?1,?2)",
                    params![card_id, lid],
                )?;
            }
        }
        UndoAction::FieldSet { card_id, field_id, before, .. } => match before {
            Some(v) => {
                conn.execute(
                    "INSERT INTO custom_field_values(card_id,field_id,value) VALUES(?1,?2,?3)
                     ON CONFLICT(card_id, field_id) DO UPDATE SET value=excluded.value",
                    params![card_id, field_id, v],
                )?;
            }
            None => {
                conn.execute(
                    "DELETE FROM custom_field_values WHERE card_id=?1 AND field_id=?2",
                    params![card_id, field_id],
                )?;
            }
        },
        UndoAction::FieldDefCreated(d) => {
            conn.execute("DELETE FROM custom_field_defs WHERE id=?1", [&d.id])?;
        }
        UndoAction::FieldDefDeleted(d) => insert_field_def_row(conn, d)?,
        UndoAction::FieldDefUpdated { before, .. } => update_field_def_row(conn, before)?,
        UndoAction::DependencyCreated(d) => {
            conn.execute("DELETE FROM dependencies WHERE id=?1", [&d.id])?;
        }
        UndoAction::DependencyDeleted(d) => insert_dependency_row(conn, d)?,
        UndoAction::DependencyUpdated { before, .. } => update_dependency_row_undo(conn, before)?,
        UndoAction::ChecklistCreated(cl) => {
            conn.execute("DELETE FROM checklists WHERE id=?1", [&cl.id])?;
        }
        UndoAction::ChecklistDeleted { checklist, items } => {
            insert_checklist_row(conn, checklist)?;
            for it in items {
                insert_checklist_item_row(conn, it)?;
            }
        }
        UndoAction::ChecklistItemCreated(i) => {
            conn.execute("DELETE FROM checklist_items WHERE id=?1", [&i.id])?;
        }
        UndoAction::ChecklistItemUpdated { before, .. } => {
            crate::service::checklists::update_checklist_item_row(conn, before)?
        }
        UndoAction::ChecklistItemDeleted(i) => insert_checklist_item_row(conn, i)?,
        UndoAction::CommentCreated(c) => {
            conn.execute("DELETE FROM comments WHERE id=?1", [&c.id])?;
        }
        UndoAction::CommentDeleted(c) => insert_comment_row(conn, c)?,
        UndoAction::AutomationCreated(a) => {
            conn.execute("DELETE FROM automations WHERE id=?1", [&a.id])?;
        }
        UndoAction::AutomationDeleted(a) => insert_automation_row(conn, a)?,
        UndoAction::AutomationUpdated { before, .. } => update_automation_row(conn, before)?,
        UndoAction::LabelCreated(l) => {
            conn.execute("DELETE FROM labels WHERE id=?1", [&l.id])?;
        }
        UndoAction::LabelUpdated { before, .. } => update_label_row(conn, before)?,
        UndoAction::LabelDeleted(l) => insert_label_row(conn, l)?,
    }
    Ok(())
}

fn apply_action_redo(conn: &Connection, a: &UndoAction) -> Result<()> {
    use crate::service::automations::{insert_automation_row, update_automation_row};
    use crate::service::fields::{insert_field_def_row, update_field_def_row};
    use crate::service::labels::update_label_row;
    match a {
        UndoAction::BoardCreated(b) => insert_board_row(conn, b)?,
        UndoAction::BoardUpdated { after, .. } => update_board_row(conn, after)?,
        UndoAction::BoardDeleted { board, .. } => {
            conn.execute("DELETE FROM boards WHERE id=?1", [&board.id])?;
        }
        UndoAction::ListCreated(l) => insert_list_row(conn, l)?,
        UndoAction::ListUpdated { after, .. } => update_list_row(conn, after)?,
        UndoAction::ListDeleted { list, .. } => {
            conn.execute("DELETE FROM lists WHERE id=?1", [&list.id])?;
        }
        UndoAction::CardCreated(c) => insert_card_row(conn, c)?,
        UndoAction::CardUpdated { after, .. } => update_card_row(conn, after)?,
        UndoAction::CardDeleted(snap) => delete_card_cascade(conn, &snap.card.id)?,
        UndoAction::LabelsChanged { card_id, after, .. } => {
            conn.execute("DELETE FROM card_labels WHERE card_id=?1", [card_id])?;
            for lid in after {
                conn.execute(
                    "INSERT OR IGNORE INTO card_labels(card_id,label_id) VALUES(?1,?2)",
                    params![card_id, lid],
                )?;
            }
        }
        UndoAction::FieldSet { card_id, field_id, after, .. } => match after {
            Some(v) => {
                conn.execute(
                    "INSERT INTO custom_field_values(card_id,field_id,value) VALUES(?1,?2,?3)
                     ON CONFLICT(card_id, field_id) DO UPDATE SET value=excluded.value",
                    params![card_id, field_id, v],
                )?;
            }
            None => {
                conn.execute(
                    "DELETE FROM custom_field_values WHERE card_id=?1 AND field_id=?2",
                    params![card_id, field_id],
                )?;
            }
        },
        UndoAction::FieldDefCreated(d) => insert_field_def_row(conn, d)?,
        UndoAction::FieldDefDeleted(d) => {
            conn.execute("DELETE FROM custom_field_defs WHERE id=?1", [&d.id])?;
        }
        UndoAction::FieldDefUpdated { after, .. } => update_field_def_row(conn, after)?,
        UndoAction::DependencyCreated(d) => insert_dependency_row(conn, d)?,
        UndoAction::DependencyDeleted(d) => {
            conn.execute("DELETE FROM dependencies WHERE id=?1", [&d.id])?;
        }
        UndoAction::DependencyUpdated { after, .. } => update_dependency_row_undo(conn, after)?,
        UndoAction::ChecklistCreated(cl) => insert_checklist_row(conn, cl)?,
        UndoAction::ChecklistDeleted { checklist, .. } => {
            conn.execute("DELETE FROM checklists WHERE id=?1", [&checklist.id])?;
        }
        UndoAction::ChecklistItemCreated(i) => insert_checklist_item_row(conn, i)?,
        UndoAction::ChecklistItemUpdated { after, .. } => {
            crate::service::checklists::update_checklist_item_row(conn, after)?
        }
        UndoAction::ChecklistItemDeleted(i) => {
            conn.execute("DELETE FROM checklist_items WHERE id=?1", [&i.id])?;
        }
        UndoAction::CommentCreated(c) => insert_comment_row(conn, c)?,
        UndoAction::CommentDeleted(c) => {
            conn.execute("DELETE FROM comments WHERE id=?1", [&c.id])?;
        }
        UndoAction::AutomationCreated(a) => insert_automation_row(conn, a)?,
        UndoAction::AutomationDeleted(a) => {
            conn.execute("DELETE FROM automations WHERE id=?1", [&a.id])?;
        }
        UndoAction::AutomationUpdated { after, .. } => update_automation_row(conn, after)?,
        UndoAction::LabelCreated(l) => insert_label_row(conn, l)?,
        UndoAction::LabelUpdated { after, .. } => update_label_row(conn, after)?,
        UndoAction::LabelDeleted(l) => {
            conn.execute("DELETE FROM labels WHERE id=?1", [&l.id])?;
        }
    }
    Ok(())
}

fn update_dependency_row_undo(conn: &Connection, d: &Dependency) -> Result<()> {
    conn.execute(
        "UPDATE dependencies SET type=?2, lag_days=?3 WHERE id=?1",
        params![d.id, d.kind, d.lag_days],
    )?;
    Ok(())
}
