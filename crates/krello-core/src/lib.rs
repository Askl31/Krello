//! Krello core: domain model, SQLite storage, CPM scheduling, search, automations, AI abstraction.

pub mod ai;
pub mod db;
pub mod deeplink;
pub mod error;
pub mod events;
pub mod exim;
pub mod ids;
pub mod models;
pub mod scheduler;
pub mod search;
pub mod service;
pub mod shortcut;
pub mod time;
pub mod undo;

pub use error::{Error, Result};

pub const SCHEMA_VERSION: i64 = 13;
