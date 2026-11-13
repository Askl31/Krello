//! SQLite storage: connection management, pragmas, migrations.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension};

use crate::error::{Error, Result};
use crate::time::now_rfc3339;

pub struct Db {
    pub conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.configure()?;
        db.migrate()?;
        Ok(db)
    }

    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.configure()?;
        db.migrate()?;
        Ok(db)
    }

    fn configure(&self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             PRAGMA synchronous=NORMAL;
             PRAGMA busy_timeout=5000;
             PRAGMA temp_store=MEMORY;
             PRAGMA cache_size=-8000;",
        )?;
        Ok(())
    }

    /// Apply pending migrations. Idempotent.
    pub fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )?;
        let version: i64 = self
            .conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM meta WHERE key='schema_version'",
                [],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0);

        if version < 1 {
            self.conn.execute_batch(include_str!("../migrations/001_init.sql"))?;
            self.conn.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES('schema_version', '1')",
                [],
            )?;
        }
        if version < 2 {
            self.conn.execute_batch(include_str!("../migrations/002_templates.sql"))?;
            self.conn.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES('schema_version', '2')",
                [],
            )?;
        }
        if version < 3 {
            self.conn.execute_batch(include_str!("../migrations/003_automations_v3.sql"))?;
            self.conn.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES('schema_version', '3')",
                [],
            )?;
        }
        if version < 4 {
            self.conn.execute_batch(include_str!("../migrations/004_reminders.sql"))?;
            self.conn.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES('schema_version', '4')",
                [],
            )?;
        }
        if version < 5 {
            self.conn.execute_batch(include_str!("../migrations/005_time_tracking.sql"))?;
            self.conn.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES('schema_version', '5')",
                [],
            )?;
        }
        if version < 6 {
            self.conn.execute_batch(include_str!("../migrations/006_perf_indexes.sql"))?;
            self.conn.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES('schema_version', '6')",
                [],
            )?;
        }
        if version < 7 {
            self.conn.execute_batch(include_str!("../migrations/007_fts_trigger_when.sql"))?;
            self.conn.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES('schema_version', '7')",
                [],
            )?;
        }
        if version < 8 {
            self.conn.execute_batch(include_str!("../migrations/008_pinned.sql"))?;
            self.conn.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES('schema_version', '8')",
                [],
            )?;
        }
        if version < 9 {
            self.conn.execute_batch(include_str!("../migrations/009_reminders.sql"))?;
            self.conn.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES('schema_version', '9')",
                [],
            )?;
        }
        if version < 10 {
            self.conn.execute_batch(include_str!("../migrations/010_comments_fts.sql"))?;
            self.conn.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES('schema_version', '10')",
                [],
            )?;
        }
        if version < 11 {
            self.conn.execute_batch(include_str!("../migrations/011_board_templates.sql"))?;
            self.conn.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES('schema_version', '11')",
                [],
            )?;
        }
        if version < 12 {
            self.conn.execute_batch(include_str!("../migrations/012_subcards.sql"))?;
            self.conn.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES('schema_version', '12')",
                [],
            )?;
        }
        if version < 13 {
            self.conn.execute_batch(include_str!("../migrations/013_board_position.sql"))?;
            self.conn.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES('schema_version', '13')",
                [],
            )?;
        }

        // Ensure the default calendar exists.
        self.conn.execute(
            "INSERT OR IGNORE INTO calendars(id, name, weekends, holidays, hours_per_day, created_at)
             VALUES('default', 'Standard', '[6,0]', '[]', 8.0, ?1)",
            [now_rfc3339()],
        )?;
        Ok(())
    }

    pub fn meta_get(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn.prepare_cached("SELECT value FROM meta WHERE key = ?1")?;
        let mut rows = stmt.query([key])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    pub fn meta_set(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }
}

impl Db {
    /// Convenience wrapper used across services.
    pub fn query_one<T, F>(&self, sql: &str, params: impl rusqlite::Params, map: F) -> Result<T>
    where
        F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        self.conn
            .query_row(sql, params, map)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound("row".into()),
                other => Error::Sql(other),
            })
    }
}
