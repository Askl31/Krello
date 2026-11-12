//! App-level settings (stored in the `meta` table) + automatic database backups.
//!
//! Backup strategy: SQLite `VACUUM INTO` produces a compact, consistent copy of
//! the whole database without blocking other connections. Files land in the
//! user-chosen directory as `krello-backup-<YYYY-MM-DD-HHMMSS>.db`; the oldest
//! ones are pruned beyond the configured `keep` count. Automatic backups run on
//! the server timer (every minute checks the configured interval).

use std::path::PathBuf;

use rusqlite::params;

use super::Service;
use crate::error::{Error, Result};
use crate::time::now_rfc3339;

pub const BACKUP_DIR_KEY: &str = "backup_dir";
pub const BACKUP_INTERVAL_KEY: &str = "backup_interval_hours";
pub const BACKUP_KEEP_KEY: &str = "backup_keep";
pub const LAST_BACKUP_AT_KEY: &str = "last_backup_at";
pub const LAST_BACKUP_FILE_KEY: &str = "last_backup_file";

pub const UI_THEME_KEY: &str = "ui_theme";
pub const UI_STYLE_KEY: &str = "ui_style";
pub const UI_SIDEBAR_COLLAPSED_KEY: &str = "ui_sidebar_collapsed";

pub const DEFAULT_INTERVAL_HOURS: i64 = 24;
pub const DEFAULT_KEEP: i64 = 10;
const MAX_INTERVAL_HOURS: i64 = 24 * 30;
const MAX_KEEP: i64 = 100;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct UiSettings {
    pub theme: String,
    pub style: String,
    #[serde(default)]
    pub sidebar_collapsed: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BackupSettings {
    /// `None` or empty = automatic backups disabled.
    pub backup_dir: Option<String>,
    pub interval_hours: i64,
    pub keep: i64,
    pub last_backup_at: Option<String>,
    pub last_backup_file: Option<String>,
}

impl Service {
    /// Current backup settings, with defaults filled in.
    pub fn backup_settings(&self) -> Result<BackupSettings> {
        let db = self.lock_db()?;
        let dir = db.meta_get(BACKUP_DIR_KEY)?;
        let interval: Option<String> = db.meta_get(BACKUP_INTERVAL_KEY)?;
        let keep: Option<String> = db.meta_get(BACKUP_KEEP_KEY)?;
        Ok(BackupSettings {
            backup_dir: dir.filter(|d| !d.trim().is_empty()),
            interval_hours: interval
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_INTERVAL_HOURS)
                .clamp(1, MAX_INTERVAL_HOURS),
            keep: keep
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_KEEP)
                .clamp(1, MAX_KEEP),
            last_backup_at: db.meta_get(LAST_BACKUP_AT_KEY)?,
            last_backup_file: db.meta_get(LAST_BACKUP_FILE_KEY)?,
        })
    }

    /// Update backup settings. An empty/whitespace `dir` disables automatic
    /// backups; otherwise the directory is created and validated (writable
    /// check happens on the first backup). Returns the normalized settings.
    pub fn set_backup_settings(
        &self,
        dir: Option<String>,
        interval_hours: Option<i64>,
        keep: Option<i64>,
    ) -> Result<BackupSettings> {
        {
            let db = self.lock_db()?;
            match dir.as_deref().map(str::trim) {
                Some("") | None => {
                    // Absent field: no change. Explicitly empty: disable.
                    if dir.is_some() {
                        db.conn
                            .execute("DELETE FROM meta WHERE key=?1", [BACKUP_DIR_KEY])?;
                    }
                }
                Some(d) => {
                    std::fs::create_dir_all(d).map_err(|e| {
                        Error::Validation(format!(
                            "répertoire de sauvegarde inutilisable ({d}): {e}"
                        ))
                    })?;
                    db.meta_set(BACKUP_DIR_KEY, d)?;
                }
            }
            if let Some(h) = interval_hours {
                db.meta_set(
                    BACKUP_INTERVAL_KEY,
                    &h.clamp(1, MAX_INTERVAL_HOURS).to_string(),
                )?;
            }
            if let Some(k) = keep {
                db.meta_set(BACKUP_KEEP_KEY, &k.clamp(1, MAX_KEEP).to_string())?;
            }
        }
        self.backup_settings()
    }

    /// App-level UI preferences (theme, visual style, sidebar collapsed state).
    pub fn ui_settings(&self) -> Result<UiSettings> {
        let db = self.lock_db()?;
        let theme = db.meta_get(UI_THEME_KEY)?.unwrap_or_else(|| "auto".into());
        let style = db.meta_get(UI_STYLE_KEY)?.unwrap_or_else(|| "standard".into());
        let sidebar_collapsed = db
            .meta_get(UI_SIDEBAR_COLLAPSED_KEY)?
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        Ok(UiSettings {
            theme,
            style,
            sidebar_collapsed,
        })
    }

    /// Update app-level UI preferences in SQLite `meta` table.
    pub fn set_ui_settings(
        &self,
        theme: Option<String>,
        style: Option<String>,
        sidebar_collapsed: Option<bool>,
    ) -> Result<UiSettings> {
        {
            let db = self.lock_db()?;
            if let Some(t) = theme {
                let valid = match t.as_str() {
                    "light" | "dark" | "auto" => t,
                    _ => "auto".into(),
                };
                db.meta_set(UI_THEME_KEY, &valid)?;
            }
            if let Some(s) = style {
                let valid = match s.as_str() {
                    "translucent" | "standard" => s,
                    _ => "standard".into(),
                };
                db.meta_set(UI_STYLE_KEY, &valid)?;
            }
            if let Some(sc) = sidebar_collapsed {
                db.meta_set(UI_SIDEBAR_COLLAPSED_KEY, if sc { "true" } else { "false" })?;
            }
        }
        self.ui_settings()
    }

    /// Run a backup right now into the configured directory. Returns its path.
    pub fn run_backup_now(&self) -> Result<PathBuf> {
        let dir = self
            .backup_settings()?
            .backup_dir
            .ok_or_else(|| Error::Validation("aucun répertoire de sauvegarde configuré".into()))?;
        self.backup_into(&dir)
    }

    /// Called by the server timer: runs a backup when the configured interval
    /// has elapsed since the last one (or when none exists yet). Returns the
    /// backup path when a backup was made.
    pub fn maybe_auto_backup(&self) -> Result<Option<PathBuf>> {
        let settings = self.backup_settings()?;
        let Some(dir) = settings.backup_dir else {
            return Ok(None);
        };
        let due = {
            let db = self.lock_db()?;
            match db.meta_get(LAST_BACKUP_AT_KEY)? {
                Some(last) => match crate::time::parse_dt(&last) {
                    Some(t) => {
                        let elapsed = chrono::Local::now().naive_local() - t;
                        elapsed.num_hours() >= settings.interval_hours
                    }
                    None => true,
                },
                None => true,
            }
        };
        if !due {
            return Ok(None);
        }
        Ok(Some(self.backup_into(&dir)?))
    }

    /// Write a compact snapshot of the database into `dir` and prune old files.
    fn backup_into(&self, dir: &str) -> Result<PathBuf> {
        std::fs::create_dir_all(dir).map_err(|e| {
            Error::Validation(format!("répertoire de sauvegarde inutilisable ({dir}): {e}"))
        })?;
        let stamp = chrono::Local::now().format("%Y-%m-%d-%H%M%S");
        let mut target = PathBuf::from(dir).join(format!("krello-backup-{stamp}.db"));
        // Two backups within the same second: disambiguate (-1, -2…) — VACUUM
        // INTO refuses to overwrite an existing file.
        for n in 1..=9 {
            if !target.exists() {
                break;
            }
            target = PathBuf::from(dir).join(format!("krello-backup-{stamp}-{n}.db"));
        }
        {
            let db = self.lock_db()?;
            db.conn
                .execute("VACUUM INTO ?1", params![target.to_string_lossy().as_ref()])
                .map_err(|e| {
                    Error::Validation(format!("sauvegarde impossible ({target:?}): {e}"))
                })?;
            db.meta_set(LAST_BACKUP_AT_KEY, &now_rfc3339())?;
            db.meta_set(LAST_BACKUP_FILE_KEY, target.to_string_lossy().as_ref())?;
        }
        tracing::info!("backup: {}", target.display());
        self.prune_old_backups(dir)?;
        Ok(target)
    }

    /// Delete the oldest `krello-backup-*.db` files beyond the configured keep.
    fn prune_old_backups(&self, dir: &str) -> Result<usize> {
        let keep = self.backup_settings()?.keep as usize;
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("krello-backup-") && n.ends_with(".db"))
                    .unwrap_or(false)
            })
            .collect();
        // Fixed-width ISO-ish stamp in the name: lexical order = chronological.
        files.sort();
        let mut removed = 0;
        while files.len() > keep {
            let oldest = files.remove(0);
            if std::fs::remove_file(&oldest).is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }
}
