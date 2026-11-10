//! Full-text search (FTS5) + structured filters.

use rusqlite::params;

use crate::error::Result;
use crate::models::Card;
use crate::undo::row_card;

#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub board_id: Option<String>,
    pub list_id: Option<String>,
    pub label_id: Option<String>,
    pub priority: Option<String>,
    /// ISO date (inclusive)
    pub due_before: Option<String>,
    /// ISO date (inclusive)
    pub due_after: Option<String>,
    pub has_due: Option<bool>,
    pub include_archived: bool,
    pub limit: i64,
}

/// Build an FTS5 MATCH expression: each whitespace token becomes a quoted
/// prefix query (`foo bar` → `"foo"* "bar"*`).
pub fn sanitize_fts(q: &str) -> String {
    q.split_whitespace()
        .map(|tok| {
            let cleaned = tok.replace('"', "");
            format!("\"{}\"*", cleaned)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn search_cards(
    conn: &rusqlite::Connection,
    q: Option<&str>,
    f: &SearchFilters,
) -> Result<Vec<Card>> {
    let fts = q.map(sanitize_fts).filter(|s| !s.is_empty());
    let mut sql = String::from("SELECT c.* FROM cards c");
    if fts.is_some() {
        sql.push_str(" JOIN cards_fts f ON f.rowid = c.rowid");
    }
    sql.push_str(" WHERE 1=1");
    let mut args: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(expr) = &fts {
        args.push(Box::new(expr.clone()));
        sql.push_str(&format!(" AND cards_fts MATCH ?{}", args.len()));
    }
    if let Some(b) = &f.board_id {
        args.push(Box::new(b.clone()));
        sql.push_str(&format!(" AND c.board_id = ?{}", args.len()));
    }
    if let Some(l) = &f.list_id {
        args.push(Box::new(l.clone()));
        sql.push_str(&format!(" AND c.list_id = ?{}", args.len()));
    }
    if let Some(l) = &f.label_id {
        args.push(Box::new(l.clone()));
        sql.push_str(&format!(
            " AND c.id IN (SELECT card_id FROM card_labels WHERE label_id = ?{})",
            args.len()
        ));
    }
    if let Some(p) = &f.priority {
        args.push(Box::new(p.clone()));
        sql.push_str(&format!(" AND c.priority = ?{}", args.len()));
    }
    if let Some(d) = &f.due_before {
        args.push(Box::new(d.clone()));
        sql.push_str(&format!(" AND c.due IS NOT NULL AND c.due <= ?{}", args.len()));
    }
    if let Some(d) = &f.due_after {
        args.push(Box::new(d.clone()));
        sql.push_str(&format!(" AND c.due IS NOT NULL AND c.due >= ?{}", args.len()));
    }
    if f.has_due == Some(true) {
        sql.push_str(" AND c.due IS NOT NULL");
    }
    if !f.include_archived {
        sql.push_str(" AND c.archived = 0");
    }
    if fts.is_some() {
        sql.push_str(" ORDER BY rank");
    } else {
        sql.push_str(" ORDER BY c.list_id, c.position");
    }
    args.push(Box::new(f.limit.clamp(1, 500)));
    sql.push_str(&format!(" LIMIT ?{}", args.len()));

    let mut stmt = conn.prepare(&sql)?;
    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        args.iter().map(|b| b.as_ref() as &dyn rusqlite::types::ToSql).collect();
    let cards = stmt
        .query_map(params_ref.as_slice(), row_card)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(cards)
}

/// Global search across all boards (home screen).
pub fn search_global(conn: &rusqlite::Connection, q: &str, limit: i64) -> Result<Vec<Card>> {
    search_cards(
        conn,
        Some(q),
        &SearchFilters {
            limit: limit.clamp(1, 100),
            ..Default::default()
        },
    )
}

/// Lightweight suggestions for the ⌘K palette (titles only, prefix match).
pub fn suggest_cards(conn: &rusqlite::Connection, q: &str, limit: i64) -> Result<Vec<Card>> {
    let like = format!("%{}%", q.replace('%', "").to_lowercase());
    let mut stmt = conn.prepare(
        "SELECT * FROM cards WHERE archived=0 AND LOWER(title) LIKE ?1
         ORDER BY updated_at DESC LIMIT ?2",
    )?;
    let cards = stmt
        .query_map(params![like, limit.clamp(1, 20)], row_card)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(cards)
}
