//! Date & time helpers. All dates are stored as ISO-8601 TEXT in SQLite.
//! - dates: `YYYY-MM-DD`
//! - datetimes: RFC3339 (`2026-11-10T14:30:00+01:00`) or naive `YYYY-MM-DDTHH:MM:SS`.

use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike};

pub fn now_rfc3339() -> String {
    Local::now().to_rfc3339()
}

pub fn now_date() -> String {
    Local::now().date_naive().to_string()
}

/// Parse a date (`YYYY-MM-DD`) or any datetime string into a NaiveDate.
pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(d);
    }
    parse_dt(s).map(|dt| dt.date())
}

/// Parse a datetime in several accepted formats into a NaiveDateTime.
pub fn parse_dt(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim();
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(dt);
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(dt);
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M") {
        return Some(dt);
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d.and_hms_opt(0, 0, 0);
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.naive_local());
    }
    None
}

/// True if the string looks like a pure date (no time component).
pub fn is_date_only(s: &str) -> bool {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").is_ok()
}

pub fn fmt_date(d: NaiveDate) -> String {
    d.to_string()
}

pub fn fmt_dt(d: NaiveDateTime) -> String {
    d.format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Combine a date with an optional time-of-day string (`HH:MM` or `HH:MM:SS`).
pub fn date_with_time(date: NaiveDate, time: Option<&str>) -> NaiveDateTime {
    let t = time
        .and_then(|t| NaiveTime::parse_from_str(t, "%H:%M").ok())
        .or_else(|| time.and_then(|t| NaiveTime::parse_from_str(t, "%H:%M:%S").ok()))
        .unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    date.and_time(t)
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn weekday(d: NaiveDate) -> u32 {
    // chrono: Monday=0 .. Sunday=6 ; we use 0=Sunday..6=Saturday (JS-like)
    d.weekday().num_days_from_sunday()
}

pub fn add_days(d: NaiveDate, days: i64) -> NaiveDate {
    d.checked_add_signed(chrono::Duration::days(days)).unwrap_or(d)
}

pub fn days_between(a: NaiveDate, b: NaiveDate) -> i64 {
    (b - a).num_days()
}

pub fn iso_year_week(d: NaiveDate) -> (i32, u32) {
    let iso = d.iso_week();
    (iso.year(), iso.week())
}

pub fn hour_of(dt: NaiveDateTime) -> u32 {
    dt.hour()
}
