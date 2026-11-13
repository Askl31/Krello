//! Export (JSON / Markdown / CSV / ICS) and import (Krello JSON, Trello JSON).

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{Error, Result};
use crate::models::*;
use crate::time::parse_date;
use crate::undo::{
    insert_list_row, query_board, row_card, row_checklist, row_automation, row_calendar,
    row_dependency, row_field_def, row_label, row_list, snapshot_cards_bulk,
};

pub const EXPORT_FORMATS: [&str; 4] = ["json", "md", "csv", "ics"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Markdown,
    Csv,
    Ics,
}

impl ExportFormat {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "md" | "markdown" => Ok(Self::Markdown),
            "csv" => Ok(Self::Csv),
            "ics" => Ok(Self::Ics),
            other => Err(Error::Validation(format!(
                "format d'export inconnu: {other} (attendus: json, md, csv, ics)"
            ))),
        }
    }

    pub fn mime(&self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Markdown => "text/markdown",
            Self::Csv => "text/csv",
            Self::Ics => "text/calendar",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Markdown => "md",
            Self::Csv => "csv",
            Self::Ics => "ics",
        }
    }
}

/// Export one board in the requested format. Returns (filename, mime, body).
pub fn export_board(
    conn: &Connection,
    board_id: &str,
    format: ExportFormat,
) -> Result<(String, &'static str, String)> {
    let board =
        query_board(conn, board_id)?.ok_or_else(|| Error::NotFound(format!("board {board_id}")))?;
    let body = match format {
        ExportFormat::Json => export_json(conn, board_id)?,
        ExportFormat::Markdown => export_markdown(conn, &board)?,
        ExportFormat::Csv => export_csv(conn, &board)?,
        ExportFormat::Ics => export_ics(conn, &board)?,
    };
    let safe_title: String = board
        .title
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    Ok((format!("{safe_title}.{}", format.extension()), format.mime(), body))
}

/// Krello JSON dump (round-trippable via import_krello).
pub fn export_json(conn: &Connection, board_id: &str) -> Result<String> {
    let board = query_board(conn, board_id)?
        .ok_or_else(|| Error::NotFound(format!("board {board_id}")))?;
    // Bulk: un jeu fixe de requêtes (le format du dump est inchangé).
    // On ne charge que les ids : snapshot_cards_bulk re-lit les lignes en groupé.
    let card_ids: Vec<String> = {
        let mut stmt = conn.prepare_cached("SELECT id FROM cards WHERE board_id=?1 ORDER BY position")?;
        let v = stmt
            .query_map([board_id], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    let card_payloads: Vec<serde_json::Value> = snapshot_cards_bulk(conn, &card_ids)?
        .into_iter()
        .map(|snap| {
            serde_json::json!({
                "card": snap.card,
                "label_ids": snap.label_ids,
                "checklists": snap.checklists,
                "comments": snap.comments,
                "field_values": snap.field_values,
            })
        })
        .collect();
    let lists: Vec<List> = {
        let mut stmt = conn.prepare_cached("SELECT * FROM lists WHERE board_id=?1 ORDER BY position")?;
        let v = stmt
            .query_map([board_id], row_list)?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    let labels: Vec<Label> = {
        let mut stmt = conn.prepare_cached("SELECT * FROM labels WHERE board_id=?1")?;
        let v = stmt
            .query_map([board_id], row_label)?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    let deps: Vec<Dependency> = {
        let mut stmt = conn.prepare_cached("SELECT * FROM dependencies WHERE board_id=?1")?;
        let v = stmt
            .query_map([board_id], row_dependency)?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    let field_defs: Vec<CustomFieldDef> = {
        let mut stmt = conn.prepare_cached("SELECT * FROM custom_field_defs WHERE board_id=?1")?;
        let v = stmt
            .query_map([board_id], row_field_def)?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    let automations: Vec<Automation> = {
        let mut stmt = conn.prepare_cached("SELECT * FROM automations WHERE board_id=?1")?;
        let v = stmt
            .query_map([board_id], row_automation)?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    let calendar: Option<Calendar> = match &board.calendar_id {
        Some(id) => conn
            .query_row("SELECT * FROM calendars WHERE id=?1", [id], row_calendar)
            .optional()?,
        None => None,
    };
    let doc = serde_json::json!({
        "format": "krello",
        "version": 1,
        "board": board,
        "lists": lists,
        "labels": labels,
        "cards": card_payloads,
        "dependencies": deps,
        "custom_field_defs": field_defs,
        "automations": automations,
        "calendar": calendar,
    });
    Ok(serde_json::to_string_pretty(&doc)?)
}

fn export_markdown(conn: &Connection, board: &Board) -> Result<String> {
    use std::collections::HashMap;
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", board.title));
    if !board.description.is_empty() {
        out.push_str(&board.description);
        out.push_str("\n\n");
    }

    // Bulk-load in a fixed number of queries: no per-row prepare/queries.
    let lists: Vec<List> = {
        let mut stmt = conn.prepare_cached("SELECT * FROM lists WHERE board_id=?1 AND archived=0 ORDER BY position")?;
        let v = stmt
            .query_map([&board.id], row_list)?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    let mut cards_by_list: HashMap<String, Vec<Card>> = HashMap::new();
    {
        let mut stmt = conn.prepare_cached("SELECT * FROM cards WHERE board_id=?1 AND archived=0 AND is_template=0 ORDER BY list_id, position")?;
        for r in stmt.query_map([&board.id], row_card)? {
            let c: Card = r?;
            cards_by_list.entry(c.list_id.clone()).or_default().push(c);
        }
    }
    let mut label_names: HashMap<String, String> = HashMap::new();
    {
        let mut stmt =
            conn.prepare_cached("SELECT id, COALESCE(NULLIF(name, ''), color) FROM labels WHERE board_id=?1")?;
        for r in stmt.query_map([&board.id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })? {
            let (id, name) = r?;
            label_names.insert(id, name);
        }
    }
    let mut label_ids_by_card: HashMap<String, Vec<String>> = HashMap::new();
    {
        let mut stmt = conn.prepare_cached("SELECT cl.card_id, cl.label_id FROM card_labels cl
         JOIN cards cd ON cd.id = cl.card_id
         WHERE cd.board_id=?1 AND cd.archived=0")?;
        for r in stmt.query_map([&board.id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })? {
            let (cid, lid) = r?;
            label_ids_by_card.entry(cid).or_default().push(lid);
        }
    }
    let mut checklists_by_card: HashMap<String, Vec<Checklist>> = HashMap::new();
    {
        let mut stmt = conn.prepare_cached("SELECT ch.* FROM checklists ch
         JOIN cards cd ON cd.id = ch.card_id
         WHERE cd.board_id=?1 AND cd.archived=0
         ORDER BY ch.card_id, ch.position")?;
        for r in stmt.query_map([&board.id], row_checklist)? {
            let cl: Checklist = r?;
            checklists_by_card.entry(cl.card_id.clone()).or_default().push(cl);
        }
    }
    let cl_ids: Vec<String> = checklists_by_card
        .values()
        .flatten()
        .map(|c| c.id.clone())
        .collect();
    let items_by_cl = crate::service::checklists::items_for_checklists(conn, &cl_ids)?;

    for l in &lists {
        out.push_str(&format!("\n## {}\n\n", l.title));
        let Some(cards) = cards_by_list.get(&l.id) else {
            continue;
        };
        for c in cards {
            let mut meta = Vec::new();
            if let Some(lids) = label_ids_by_card.get(&c.id) {
                let names: Vec<&str> = lids
                    .iter()
                    .filter_map(|id| label_names.get(id).map(|s| s.as_str()))
                    .filter(|n| !n.is_empty())
                    .collect();
                if !names.is_empty() {
                    meta.push(format!("[{}]", names.join(", ")));
                }
            }
            if let Some(d) = &c.due {
                meta.push(format!("échéance: {d}"));
            }
            if c.priority != "none" {
                meta.push(format!("priorité: {}", c.priority));
            }
            if c.is_milestone {
                meta.push("jalon".into());
            }
            if c.pct_complete > 0 {
                meta.push(format!("{}%", c.pct_complete));
            }
            let meta_str = if meta.is_empty() {
                String::new()
            } else {
                format!(" ({})", meta.join(" · "))
            };
            out.push_str(&format!(
                "- [{}] {}{}\n",
                if c.due_complete { "x" } else { " " },
                c.title,
                meta_str
            ));
            if !c.description.is_empty() {
                for line in c.description.lines() {
                    out.push_str(&format!("    {line}\n"));
                }
            }
            if let Some(cls) = checklists_by_card.get(&c.id) {
                for cl in cls {
                    out.push_str(&format!("    - {}/\n", cl.title));
                    if let Some(items) = items_by_cl.get(&cl.id) {
                        for it in items {
                            out.push_str(&format!(
                                "        - [{}] {}\n",
                                if it.checked { "x" } else { " " },
                                it.title
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

fn csv_escape(s: &str) -> String {
    if s.contains('"') || s.contains(',') || s.contains('\n') || s.contains(';') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// One CSV row: (card_id, title, list, priority, start, due, pct, duration, work, wbs, milestone, desc).
type CsvRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    i64,
    Option<f64>,
    Option<f64>,
    String,
    bool,
    String,
);

fn export_csv(conn: &Connection, board: &Board) -> Result<String> {
    let mut out = String::from(
        "titre;liste;labels;priorite;debut;echeance;pct;duration_j;work_h;wbs;jalon;description\n",
    );
    // card id first: labels resolve by card_id, never by title (not unique).
    let rows: Vec<CsvRow> = {
        let mut stmt = conn.prepare_cached("SELECT c.id, c.title, l.title, c.priority, c.start, c.due, c.pct_complete,
                c.duration_days, c.work_hours, c.wbs, c.is_milestone, c.description
         FROM cards c JOIN lists l ON l.id = c.list_id
         WHERE c.board_id=?1 AND c.archived=0 AND c.is_template=0
         ORDER BY l.position, c.position")?;
        let v = stmt
            .query_map([&board.id], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    // Labels de toutes les cartes en UNE requête groupée (avant : 1 requête par carte).
    let mut labels_by_card: std::collections::HashMap<String, String> = Default::default();
    {
        let mut stmt = conn.prepare_cached(
            "SELECT cl.card_id, COALESCE(group_concat(COALESCE(NULLIF(l.name,''), l.color), ', '), '')
             FROM card_labels cl
             JOIN labels l ON l.id = cl.label_id
             JOIN cards c ON c.id = cl.card_id
             WHERE c.board_id=?1 AND c.archived=0 AND c.is_template=0
             GROUP BY cl.card_id",
        )?;
        let rows = stmt.query_map([&board.id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for r in rows {
            let (cid, names) = r?;
            labels_by_card.insert(cid, names);
        }
    }
    for (card_id, title, list, priority, start, due, pct, dur, work, wbs, milestone, desc) in rows {
        let labels = labels_by_card.get(&card_id).cloned().unwrap_or_default();
        out.push_str(&format!(
            "{};{};{};{};{};{};{};{};{};{};{};{}\n",
            csv_escape(&title),
            csv_escape(&list),
            csv_escape(&labels),
            priority,
            start.unwrap_or_default(),
            due.unwrap_or_default(),
            pct,
            dur.map(|d| d.to_string()).unwrap_or_default(),
            work.map(|w| w.to_string()).unwrap_or_default(),
            wbs,
            milestone,
            csv_escape(&desc),
        ));
    }
    Ok(out)
}

fn ics_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace(',', "\\,")
        .replace(';', "\\;")
}

fn export_ics(conn: &Connection, board: &Board) -> Result<String> {
    let mut out = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Krello//FR\r\nCALSCALE:GREGORIAN\r\n");
    let cards: Vec<Card> = {
        let mut stmt =
            conn.prepare_cached("SELECT * FROM cards WHERE board_id=?1 AND archived=0 AND due IS NOT NULL")?;
        let v = stmt
            .query_map([&board.id], row_card)?
            .collect::<rusqlite::Result<_>>()?;
        v
    };
    for c in cards {
        let due = c.due.as_ref().unwrap();
        let has_time = crate::time::parse_dt(due).is_some() && !crate::time::is_date_only(due);
        // RFC 5545: DTEND est obligatoire pour les clients stricts. Une date
        // sans heure = journée entière (DTEND exclusif = lendemain) ; une
        // échéance horodatée est un instant (DTEND = DTSTART).
        let (dtstart, dtend) = if has_time {
            let dt = crate::time::parse_dt(due).unwrap();
            (
                format!("DTSTART:{}\r\n", dt.format("%Y%m%dT%H%M%S")),
                format!("DTEND:{}\r\n", dt.format("%Y%m%dT%H%M%S")),
            )
        } else {
            let d = parse_date(due).unwrap_or_else(crate::time::today);
            (
                format!("DTSTART;VALUE=DATE:{}\r\n", d.format("%Y%m%d")),
                format!("DTEND;VALUE=DATE:{}\r\n", crate::time::add_days(d, 1).format("%Y%m%d")),
            )
        };
        out.push_str("BEGIN:VEVENT\r\n");
        out.push_str(&format!("UID:{}@krello.local\r\n", c.id));
        out.push_str(&dtstart);
        out.push_str(&dtend);
        out.push_str(&format!("SUMMARY:{}\r\n", ics_escape(&c.title)));
        if !c.description.is_empty() {
            out.push_str(&format!("DESCRIPTION:{}\r\n", ics_escape(&c.description)));
        }
        out.push_str("END:VEVENT\r\n");
    }
    out.push_str("END:VCALENDAR\r\n");
    Ok(out)
}

// ---------- imports ----------

/// Import a Krello JSON dump. Creates a NEW board (fresh ids).
pub fn import_krello(service: &crate::service::Service, json: &str) -> Result<Board> {
    let doc: serde_json::Value =
        serde_json::from_str(json).map_err(|e| Error::Validation(format!("JSON invalide: {e}")))?;
    if doc.get("format").and_then(|v| v.as_str()) != Some("krello") {
        return Err(Error::Validation("ce fichier n'est pas un export Krello".into()));
    }
    let board: Board = serde_json::from_value(doc.get("board").cloned().unwrap_or_default())
        .map_err(|e| Error::Validation(format!("board invalide: {e}")))?;

    let new_board = create_board_shell(service, &board.title, Some(board.background.clone()), board.calendar_id.clone(), board.start_date.clone(), board.deadline.clone())?;

    let mut list_map: std::collections::HashMap<String, String> = Default::default();
    let mut card_map: std::collections::HashMap<String, String> = Default::default();
    let mut label_map: std::collections::HashMap<String, String> = Default::default();

    {
        let mut guard = service.lock_db()?;
        let tx = guard.conn.transaction()?;
        let db = &*tx;
        if let Some(lists) = doc.get("lists").and_then(|v| v.as_array()) {
            for l in lists {
                let old: List = serde_json::from_value(l.clone())
                    .map_err(|e| Error::Validation(format!("liste invalide: {e}")))?;
                let nid = crate::ids::new_id("list");
                list_map.insert(old.id.clone(), nid.clone());
                insert_list_row(
                    db,
                    &List { id: nid, board_id: new_board.id.clone(), ..old },
                )?;
            }
        }
        if let Some(labels) = doc.get("labels").and_then(|v| v.as_array()) {
            for l in labels {
                let old: Label = serde_json::from_value(l.clone())
                    .map_err(|e| Error::Validation(format!("label invalide: {e}")))?;
                let nid = crate::ids::new_id("label");
                label_map.insert(old.id.clone(), nid.clone());
                crate::undo::insert_label_row(
                    db,
                    &Label { id: nid, board_id: new_board.id.clone(), ..old },
                )?;
            }
        }
        if let Some(defs) = doc.get("custom_field_defs").and_then(|v| v.as_array()) {
            for d in defs {
                let old: CustomFieldDef = serde_json::from_value(d.clone())
                    .map_err(|e| Error::Validation(format!("champ invalide: {e}")))?;
                crate::service::fields::insert_field_def_row(
                    db,
                    &CustomFieldDef { board_id: new_board.id.clone(), ..old },
                )?;
            }
        }
        if let Some(cards) = doc.get("cards").and_then(|v| v.as_array()) {
            for cp in cards {
                let card: Card = serde_json::from_value(cp.get("card").cloned().unwrap_or_default())
                    .map_err(|e| Error::Validation(format!("carte invalide: {e}")))?;
                let nid = crate::ids::new_id("card");
                card_map.insert(card.id.clone(), nid.clone());
                let list_id = list_map.get(&card.list_id).cloned().unwrap_or_default();
                if list_id.is_empty() {
                    continue;
                }
                crate::undo::insert_card_row(
                    db,
                    &Card {
                        id: nid.clone(),
                        board_id: new_board.id.clone(),
                        list_id,
                        ..card
                    },
                )?;
                for lid in cp.get("label_ids").and_then(|v| v.as_array()).into_iter().flatten() {
                    if let Some(old_id) = lid.as_str() {
                        if let Some(new_id) = label_map.get(old_id) {
                            db.execute("INSERT OR IGNORE INTO card_labels(card_id,label_id) VALUES(?1,?2)",
                            params![nid, new_id],)?;
                        }
                    }
                }
                if let Some(cls) = cp.get("checklists").and_then(|v| v.as_array()) {
                    for clv in cls {
                        // snapshot tuples serialize as [checklist, [items]]
                        let cl: Checklist = serde_json::from_value(
                            clv.get(0).cloned().unwrap_or_default(),
                        )
                        .map_err(|e| Error::Validation(format!("checklist invalide: {e}")))?;
                        let ncl = crate::ids::new_id("cl");
                        crate::undo::insert_checklist_row(
                            db,
                            &Checklist { id: ncl.clone(), card_id: nid.clone(), ..cl.clone() },
                        )?;
                        if let Some(items) = clv.get(1).and_then(|v| v.as_array()) {
                            for it in items {
                                let item: ChecklistItem = serde_json::from_value(it.clone())
                                    .map_err(|e| Error::Validation(format!("élément invalide: {e}")))?;
                                crate::undo::insert_checklist_item_row(
                                    db,
                                    &ChecklistItem { id: crate::ids::new_id("cli"), checklist_id: ncl.clone(), ..item },
                                )?;
                            }
                        }
                    }
                }
                if let Some(fvs) = cp.get("field_values").and_then(|v| v.as_array()) {
                    for fv in fvs {
                        let v: CustomFieldValue = serde_json::from_value(fv.clone())
                            .map_err(|e| Error::Validation(format!("valeur invalide: {e}")))?;
                        db.execute("INSERT OR IGNORE INTO custom_field_values(card_id,field_id,value) VALUES(?1,?2,?3)",
                        params![nid, v.field_id, v.value],)?;
                    }
                }
                if let Some(coms) = cp.get("comments").and_then(|v| v.as_array()) {
                    for cv in coms {
                        let c: Comment = serde_json::from_value(cv.clone())
                            .map_err(|e| Error::Validation(format!("commentaire invalide: {e}")))?;
                        crate::undo::insert_comment_row(
                            db,
                            &Comment { id: crate::ids::new_id("com"), card_id: nid.clone(), ..c },
                        )?;
                    }
                }
            }
        }
        // Remap des liens parent/enfant (les ids de cartes ont été régénérés).
        {
            let mut stmt = db.prepare_cached(
                "SELECT id, parent_id FROM cards WHERE board_id=?1 AND parent_id IS NOT NULL",
            )?;
            let pairs: Vec<(String, String)> = stmt
                .query_map([new_board.id.clone()], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            drop(stmt);
            for (cid, old_parent) in pairs {
                match card_map.get(&old_parent) {
                    Some(new_parent) => {
                        db.execute(
                            "UPDATE cards SET parent_id=?2 WHERE id=?1",
                            params![cid, new_parent],
                        )?;
                    }
                    None => {
                        db.execute("UPDATE cards SET parent_id=NULL WHERE id=?1", params![cid])?;
                    }
                }
            }
        }
        if let Some(deps) = doc.get("dependencies").and_then(|v| v.as_array()) {
            for dv in deps {
                let d: Dependency = serde_json::from_value(dv.clone())
                    .map_err(|e| Error::Validation(format!("dépendance invalide: {e}")))?;
                let p = card_map.get(&d.predecessor_id).cloned();
                let s = card_map.get(&d.successor_id).cloned();
                if let (Some(p), Some(s)) = (p, s) {
                    crate::undo::insert_dependency_row(
                        db,
                        &Dependency {
                            id: crate::ids::new_id("dep"),
                            board_id: new_board.id.clone(),
                            predecessor_id: p,
                            successor_id: s,
                            ..d
                        },
                    )?;
                }
            }
        }
        if let Some(autos) = doc.get("automations").and_then(|v| v.as_array()) {
            for av in autos {
                let mut a: Automation = match serde_json::from_value(av.clone()) {
                    Ok(a) => a,
                    Err(_) => continue,
                };
                if !crate::service::automations::TRIGGERS.contains(&a.trigger.as_str()) {
                    continue; // règle inconnue : ignorée plutôt que d'échouer l'import
                }
                a.id = crate::ids::new_id("auto");
                a.board_id = new_board.id.clone();
                a.created_at = crate::time::now_rfc3339();
                let mut conds: serde_json::Value =
                    serde_json::from_str(&a.conditions).unwrap_or_else(|_| serde_json::json!({}));
                remap_ref(&mut conds, &list_map, &label_map);
                a.conditions = conds.to_string();
                let mut acts: serde_json::Value =
                    serde_json::from_str(&a.actions).unwrap_or_else(|_| serde_json::json!([]));
                remap_ref(&mut acts, &list_map, &label_map);
                a.actions = acts.to_string();
                if crate::service::automations::validate_actions(&a.actions).is_ok()
                    && crate::service::automations::validate_conditions(&a.conditions).is_ok()
                {
                    crate::service::automations::insert_automation_row(db, &a)?;
                }
            }
        }
        tx.commit()?;
        drop(guard);
    }
    crate::scheduler::recompute_service(service, &new_board.id)?;
    service.emit(crate::events::Event::BoardCreated {
        board_id: new_board.id.clone(),
    });
    Ok(new_board)
}

/// Remappe récursivement les ids board-scopés (listes, labels) référencés dans
/// les conditions/actions d'une règle importée. Les clés inconnues sont retirées.
fn remap_ref(
    v: &mut serde_json::Value,
    lists: &std::collections::HashMap<String, String>,
    labels: &std::collections::HashMap<String, String>,
) {
    match v {
        serde_json::Value::Object(obj) => {
            for key in ["list_id", "from_list_id", "to_list_id", "label_id", "without_label_id"] {
                if let Some(val) = obj.get(key).cloned() {
                    if let Some(s) = val.as_str() {
                        let map = if key.contains("label") { labels } else { lists };
                        match map.get(s) {
                            Some(n) => {
                                obj.insert(key.to_string(), serde_json::Value::String(n.clone()));
                            }
                            None => {
                                obj.remove(key);
                            }
                        }
                    }
                }
            }
            if let Some(arr) = obj.get_mut("list_ids").and_then(|v| v.as_array_mut()) {
                let mut out = Vec::new();
                for item in arr.iter() {
                    if let Some(s) = item.as_str() {
                        if let Some(n) = lists.get(s) {
                            out.push(serde_json::Value::String(n.clone()));
                        }
                    }
                }
                *obj.get_mut("list_ids").unwrap() = serde_json::Value::Array(out);
            }
            for val in obj.values_mut() {
                remap_ref(val, lists, labels);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                remap_ref(item, lists, labels);
            }
        }
        _ => {}
    }
}

/// Import a Trello board export (boards/*.json from Trello).
pub fn import_trello(service: &crate::service::Service, json: &str) -> Result<Board> {
    let doc: serde_json::Value =
        serde_json::from_str(json).map_err(|e| Error::Validation(format!("JSON Trello invalide: {e}")))?;
    let title = doc
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::Validation("champ 'name' manquant".into()))?
        .to_string();

    let new_board = create_board_shell(service, &title, None, None, None, None)?;

    let mut list_map: std::collections::HashMap<String, String> = Default::default();
    let mut label_map: std::collections::HashMap<String, String> = Default::default();
    let mut card_map: std::collections::HashMap<String, String> = Default::default();

    {
        let mut guard = service.lock_db()?;
        let tx = guard.conn.transaction()?;
        let db = &*tx;
        // labels
        if let Some(labels) = doc.get("labels").and_then(|v| v.as_array()) {
            for l in labels {
                let id = l.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                let name = l.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let color = l.get("color").and_then(|v| v.as_str()).unwrap_or("gray").to_string();
                if id.is_empty() {
                    continue;
                }
                let nid = crate::ids::new_id("label");
                label_map.insert(id, nid.clone());
                crate::undo::insert_label_row(
                    db,
                    &Label {
                        id: nid,
                        board_id: new_board.id.clone(),
                        name,
                        color,
                        created_at: crate::time::now_rfc3339(),
                    },
                )?;
            }
        }
        // custom field defs (map option type → select)
        let mut cf_map: std::collections::HashMap<String, String> = Default::default();
        if let Some(defs) = doc.get("customFields").and_then(|v| v.as_array()) {
            for d in defs {
                let id = d.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                let name = d.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if id.is_empty() || name.is_empty() {
                    continue;
                }
                let trello_type = d.get("type").and_then(|v| v.as_str()).unwrap_or("text");
                let kind = match trello_type {
                    "number" => "number",
                    "date" => "date",
                    "checkbox" => "checkbox",
                    "option" => "select",
                    _ => "text",
                };
                let options = if kind == "select" {
                    let opts: Vec<String> = d
                        .get("options")
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter()
                                .map(|o| {
                                    o.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string()
                                })
                                .filter(|s| !s.is_empty())
                                .collect()
                        })
                        .unwrap_or_default();
                    serde_json::to_string(&opts).unwrap_or_else(|_| "[]".into())
                } else {
                    "[]".into()
                };
                let nid = crate::ids::new_id("cf");
                cf_map.insert(id, nid.clone());
                crate::service::fields::insert_field_def_row(
                    db,
                    &CustomFieldDef {
                        id: nid,
                        board_id: new_board.id.clone(),
                        name,
                        kind: kind.into(),
                        options,
                        position: 0.0,
                        created_at: crate::time::now_rfc3339(),
                    },
                )?;
            }
        }
        // lists
        if let Some(lists) = doc.get("lists").and_then(|v| v.as_array()) {
            let mut pos = 0.0;
            for l in lists {
                let closed = l.get("closed").and_then(|v| v.as_bool()).unwrap_or(false);
                if closed {
                    continue;
                }
                let id = l.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                let name = l.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if id.is_empty() || name.is_empty() {
                    continue;
                }
                let nid = crate::ids::new_id("list");
                list_map.insert(id, nid.clone());
                crate::undo::insert_list_row(
                    db,
                    &List {
                        id: nid,
                        board_id: new_board.id.clone(),
                        title: name,
                        position: pos,
                        wip_limit: None,
                        archived: false,
                        created_at: crate::time::now_rfc3339(),
                        updated_at: crate::time::now_rfc3339(),
                    },
                )?;
                pos += 1.0;
            }
        }
        // cards
        if let Some(cards) = doc.get("cards").and_then(|v| v.as_array()) {
            for c in cards {
                let closed = c.get("closed").and_then(|v| v.as_bool()).unwrap_or(false);
                let id = c.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                let trello_list = c.get("idList").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                let Some(list_id) = list_map.get(&trello_list).cloned() else {
                    continue;
                };
                let title = c.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if title.is_empty() {
                    continue;
                }
                let desc = c.get("desc").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let due = c.get("due").and_then(|v| v.as_str()).map(|s| s.to_string());
                let start = c.get("start").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
                let due_complete = c.get("dueComplete").and_then(|v| v.as_bool()).unwrap_or(false);
                let pos = c.get("pos").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let nid = crate::ids::new_id("card");
                card_map.insert(id.clone(), nid.clone());
                let now = crate::time::now_rfc3339();
                let card = Card {
                    id: nid.clone(),
                    board_id: new_board.id.clone(),
                    list_id: list_id.clone(),
                    title,
                    description: desc,
                    position: pos,
                    archived: closed,
                    is_template: c.get("isTemplate").and_then(|v| v.as_bool()).unwrap_or(false),
                    template_description: None,
                    cover: None,
                    start,
                    due,
                    due_complete,
                    reminders: Vec::new(),
                    reminders_sent: serde_json::Value::Null,
                    parent_id: None,
                    time_spent_minutes: 0,
                    tracking_started_at: None,
                    recurrence: None,
                    priority: "none".into(),
                    is_milestone: false,
                    duration_days: None,
                    work_hours: None,
                    pct_complete: if due_complete { 100 } else { 0 },
                    task_type: "fixed_duration".into(),
                    units: 1.0,
                    hours_per_day: 8.0,
                    constraint_type: "asap".into(),
                    constraint_date: None,
                    wbs: String::new(),
                    cost: 0.0,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                    completed_at: if due_complete { Some(now) } else { None },
                };
                crate::undo::insert_card_row(db, &card)?;
                if let Some(lids) = c.get("idLabels").and_then(|v| v.as_array()) {
                    for lid in lids {
                        if let Some(old) = lid.as_str() {
                            if let Some(new_id) = label_map.get(old) {
                                db.execute("INSERT OR IGNORE INTO card_labels(card_id,label_id) VALUES(?1,?2)",
                                params![&nid, new_id],)?;
                            }
                        }
                    }
                }
                if let Some(cfis) = c.get("customFieldItems").and_then(|v| v.as_array()) {
                    for item in cfis {
                        let fid = item.get("idCustomField").and_then(|v| v.as_str()).unwrap_or_default();
                        let value = item
                            .get("value")
                            .cloned()
                            .map(|v| match &v {
                                serde_json::Value::String(s) => s.clone(),
                                other => other.to_string(),
                            })
                            .unwrap_or_default();
                        if let Some(new_fid) = cf_map.get(fid) {
                            db.execute("INSERT OR IGNORE INTO custom_field_values(card_id,field_id,value) VALUES(?1,?2,?3)",
                            params![&nid, new_fid, value],)?;
                        }
                    }
                }
            }
        }
        // checklists (top-level, idCard)
        if let Some(cls) = doc.get("checklists").and_then(|v| v.as_array()) {
            let mut pos = 0.0;
            for cl in cls {
                let card_trello = cl.get("idCard").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                let Some(card_id) = card_map.get(&card_trello).cloned() else {
                    continue;
                };
                let name = cl.get("name").and_then(|v| v.as_str()).unwrap_or("Checklist").to_string();
                let ncl = crate::ids::new_id("cl");
                crate::undo::insert_checklist_row(
                    db,
                    &Checklist {
                        id: ncl.clone(),
                        card_id: card_id.clone(),
                        title: name,
                        position: pos,
                        created_at: crate::time::now_rfc3339(),
                    },
                )?;
                pos += 1.0;
                if let Some(items) = cl.get("checkItems").and_then(|v| v.as_array()) {
                    let mut ipos = 0.0;
                    for it in items {
                        let it_title = it.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if it_title.is_empty() {
                            continue;
                        }
                        let state = it.get("state").and_then(|v| v.as_str()).unwrap_or("incomplete");
                        crate::undo::insert_checklist_item_row(
                            db,
                            &ChecklistItem {
                                id: crate::ids::new_id("cli"),
                                checklist_id: ncl.clone(),
                                title: it_title,
                                checked: state == "complete",
                                position: ipos,
                                due: None,
                                created_at: crate::time::now_rfc3339(),
                            },
                        )?;
                        ipos += 1.0;
                    }
                }
            }
        }
        // comments from actions
        if let Some(actions) = doc.get("actions").and_then(|v| v.as_array()) {
            for a in actions {
                if a.get("type").and_then(|v| v.as_str()) != Some("commentCard") {
                    continue;
                }
                let card_trello = a
                    .get("data")
                    .and_then(|d| d.get("card"))
                    .and_then(|c| c.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let Some(card_id) = card_map.get(&card_trello).cloned() else {
                    continue;
                };
                let text = a.get("data").and_then(|d| d.get("text")).and_then(|v| v.as_str()).unwrap_or("");
                if text.is_empty() {
                    continue;
                }
                crate::undo::insert_comment_row(
                    db,
                    &Comment {
                        id: crate::ids::new_id("com"),
                        card_id,
                        body: text.to_string(),
                        created_at: a
                            .get("date")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    },
                )?;
            }
        }
        tx.commit()?;
        drop(guard);
    }
    crate::scheduler::recompute_service(service, &new_board.id)?;
    service.emit(crate::events::Event::BoardCreated {
        board_id: new_board.id.clone(),
    });
    Ok(new_board)
}

fn create_board_shell(
    service: &crate::service::Service,
    title: &str,
    background: Option<String>,
    calendar_id: Option<String>,
    start_date: Option<String>,
    deadline: Option<String>,
) -> Result<Board> {
    let now = crate::time::now_rfc3339();
    let board = Board {
        id: crate::ids::new_id("board"),
        title: title.trim().to_string(),
        description: String::new(),
        background: background.unwrap_or_else(|| crate::service::boards::DEFAULT_BACKGROUND.into()),
        calendar_id: Some(calendar_id.unwrap_or_else(|| "default".into())),
        start_date,
        deadline,
        settings: "{}".into(),
        archived: false,
        pinned: false,
        pinned_at: None,
        is_board_template: false,
        template_description: None,
        created_at: now.clone(),
        updated_at: now,
    };
    {
        let db = service.lock_db()?;
        crate::undo::insert_board_row(&db.conn, &board)?;
    }
    Ok(board)
}
