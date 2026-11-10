use krello_core::db::Db;
use rusqlite::params;

fn test_db() -> Db {
    Db::open_memory().unwrap()
}

#[test]
fn migration_is_idempotent_and_creates_schema() {
    let db = test_db();
    db.migrate().unwrap();
    db.migrate().unwrap();
    let n: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN
             ('boards','lists','cards','labels','card_labels','checklists','checklist_items',
              'custom_field_defs','custom_field_values','attachments','comments','activity',
              'dependencies','schedule_cache','baselines','baseline_snapshots','calendars',
              'automations','board_views','cards_fts')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 20, "expected all 20 schema objects");

    let version: i64 = db
        .conn
        .query_row("SELECT CAST(value AS INTEGER) FROM meta WHERE key='schema_version'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(version, krello_core::SCHEMA_VERSION);
}

#[test]
fn default_calendar_exists() {
    let db = test_db();
    let name: String = db
        .conn
        .query_row("SELECT name FROM calendars WHERE id='default'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(name, "Standard");
}

#[test]
fn raw_board_list_card_roundtrip() {
    let db = test_db();
    db.conn
        .execute(
            "INSERT INTO boards(id,title,description,background,created_at,updated_at)
             VALUES('b1','Projet','', '{}', datetime('now'), datetime('now'))",
            [],
        )
        .unwrap();
    db.conn
        .execute(
            "INSERT INTO lists(id,board_id,title,position,created_at,updated_at)
             VALUES('l1','b1','À faire',0,datetime('now'),datetime('now'))",
            [],
        )
        .unwrap();
    db.conn
        .execute(
            "INSERT INTO cards(id,board_id,list_id,title,description,position,created_at,updated_at)
             VALUES('c1','b1','l1','Ma carte','une description',0,datetime('now'),datetime('now'))",
            [],
        )
        .unwrap();

    let (title, list_id): (String, String) = db
        .conn
        .query_row("SELECT title, list_id FROM cards WHERE id='c1'", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(title, "Ma carte");
    assert_eq!(list_id, "l1");
}

#[test]
fn foreign_keys_cascade_on_board_delete() {
    let db = test_db();
    db.conn
        .execute(
            "INSERT INTO boards(id,title,created_at,updated_at) VALUES('b1','X',datetime('now'),datetime('now'))",
            [],
        )
        .unwrap();
    db.conn
        .execute(
            "INSERT INTO lists(id,board_id,title,position,created_at,updated_at)
             VALUES('l1','b1','T',0,datetime('now'),datetime('now'))",
            [],
        )
        .unwrap();
    db.conn
        .execute(
            "INSERT INTO cards(id,board_id,list_id,title,position,created_at,updated_at)
             VALUES('c1','b1','l1','C',0,datetime('now'),datetime('now'))",
            [],
        )
        .unwrap();
    db.conn.execute("DELETE FROM boards WHERE id='b1'", []).unwrap();
    let cards: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM cards", [], |r| r.get(0))
        .unwrap();
    let lists: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM lists", [], |r| r.get(0))
        .unwrap();
    assert_eq!((cards, lists), (0, 0), "cards and lists must cascade");
}

#[test]
fn fts_insert_and_search() {
    let db = test_db();
    db.conn
        .execute(
            "INSERT INTO boards(id,title,created_at,updated_at) VALUES('b1','X',datetime('now'),datetime('now'))",
            [],
        )
        .unwrap();
    db.conn
        .execute(
            "INSERT INTO lists(id,board_id,title,position,created_at,updated_at)
             VALUES('l1','b1','T',0,datetime('now'),datetime('now'))",
            [],
        )
        .unwrap();
    db.conn
        .execute(
            "INSERT INTO cards(id,board_id,list_id,title,description,position,created_at,updated_at)
             VALUES('c1','b1','l1','Facture courant','envoyer le devis au client',0,datetime('now'),datetime('now'))",
            params![],
        )
        .unwrap();
    let hits: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM cards_fts WHERE cards_fts MATCH 'devis'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(hits, 1);
}

#[test]
fn meta_get_set_roundtrip() {
    let db = test_db();
    assert_eq!(db.meta_get("missing").unwrap(), None);
    db.meta_set("undo_depth", "100").unwrap();
    db.meta_set("undo_depth", "200").unwrap();
    assert_eq!(db.meta_get("undo_depth").unwrap().unwrap(), "200");
}
