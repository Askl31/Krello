//! Calendars: working days, weekends, holidays, hours per day.

use rusqlite::{params, OptionalExtension};

use super::Service;
use crate::error::{Error, Result};
use crate::events::Event;
use crate::models::*;
use crate::time::now_rfc3339;
use crate::undo::*;

pub fn parse_weekends(json: &str) -> Result<Vec<u32>> {
    let v: Vec<u32> =
        serde_json::from_str(json).map_err(|e| Error::Validation(format!("weekends invalides: {e}")))?;
    if v.iter().any(|d| *d > 6) {
        return Err(Error::Validation("weekends: indices 0..6 attendus".into()));
    }
    Ok(v)
}

pub fn parse_holidays(json: &str) -> Result<Vec<String>> {
    let v: Vec<String> =
        serde_json::from_str(json).map_err(|e| Error::Validation(format!("holidays invalides: {e}")))?;
    for d in &v {
        if crate::time::parse_date(d).is_none() {
            return Err(Error::Validation(format!("jour férié invalide: {d}")));
        }
    }
    Ok(v)
}

impl Service {
    pub fn create_calendar(&self, c: CreateCalendar) -> Result<Calendar> {
        let weekends = parse_weekends(c.weekends.as_deref().unwrap_or("[6,0]"))?;
        let holidays = parse_holidays(c.holidays.as_deref().unwrap_or("[]"))?;
        let cal = Calendar {
            id: crate::ids::new_id("cal"),
            name: c.name.trim().to_string(),
            weekends: serde_json::to_string(&weekends)?,
            holidays: serde_json::to_string(&holidays)?,
            hours_per_day: c.hours_per_day.unwrap_or(8.0).clamp(1.0, 24.0),
            created_at: now_rfc3339(),
        };
        if cal.name.is_empty() {
            return Err(Error::Validation("nom de calendrier requis".into()));
        }
        {
            let db = self.lock_db()?;
            db.conn.execute(
                "INSERT INTO calendars(id,name,weekends,holidays,hours_per_day,created_at)
                 VALUES(?1,?2,?3,?4,?5,?6)",
                params![cal.id, cal.name, cal.weekends, cal.holidays, cal.hours_per_day, cal.created_at],
            )?;
        }
        self.emit(Event::CalendarsUpdated {
            calendar_id: cal.id.clone(),
        });
        Ok(cal)
    }

    pub fn update_calendar(&self, id: &str, u: UpdateCalendar) -> Result<Calendar> {
        let db = self.lock_db()?;
        let before = db
            .conn
            .query_row("SELECT * FROM calendars WHERE id=?1", [id], row_calendar)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("calendar {id}")))?;
        let after = Calendar {
            name: u.name.unwrap_or_else(|| before.name.clone()),
            weekends: match &u.weekends {
                Some(w) => serde_json::to_string(&parse_weekends(w)?)?,
                None => before.weekends.clone(),
            },
            holidays: match &u.holidays {
                Some(h) => serde_json::to_string(&parse_holidays(h)?)?,
                None => before.holidays.clone(),
            },
            hours_per_day: u.hours_per_day.unwrap_or(before.hours_per_day).clamp(1.0, 24.0),
            ..before.clone()
        };
        db.conn.execute(
            "UPDATE calendars SET name=?2, weekends=?3, holidays=?4, hours_per_day=?5 WHERE id=?1",
            params![after.id, after.name, after.weekends, after.holidays, after.hours_per_day],
        )?;
        drop(db);
        self.emit(Event::CalendarsUpdated {
            calendar_id: after.id.clone(),
        });
        Ok(after)
    }

    pub fn delete_calendar(&self, id: &str) -> Result<()> {
        if id == "default" {
            return Err(Error::Validation("le calendrier par défaut ne peut pas être supprimé".into()));
        }
        let db = self.lock_db()?;
        let n = db.conn.execute("DELETE FROM calendars WHERE id=?1", [id])?;
        if n == 0 {
            return Err(Error::NotFound(format!("calendar {id}")));
        }
        drop(db);
        self.emit(Event::CalendarsUpdated {
            calendar_id: id.to_string(),
        });
        Ok(())
    }

    pub fn list_calendars(&self) -> Result<Vec<Calendar>> {
        let db = self.lock_db()?;
        let mut stmt = db.conn.prepare("SELECT * FROM calendars ORDER BY created_at")?;
        let cals = stmt
            .query_map([], row_calendar)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(cals)
    }

    pub fn get_calendar(&self, id: &str) -> Result<Calendar> {
        let db = self.lock_db()?;
        db.conn
            .query_row("SELECT * FROM calendars WHERE id=?1", [id], row_calendar)
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("calendar {id}")))
    }
}
