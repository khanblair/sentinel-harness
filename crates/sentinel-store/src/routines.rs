//! Routine and brief rows: jobs at a set local time, and the briefs they built.

use rusqlite::{OptionalExtension, Row, params};
use sentinel_core::RoutineId;
use sentinel_core::routines::{Brief, Routine, RoutineKind};
use time::OffsetDateTime;

use crate::{Result, Store, StoreError, enum_from_text, enum_text, time_text};

const ROUTINE_COLUMNS: &str = "id, name, kind, minute_of_day, days, enabled, to_telegram, text, \
     backlog, last_fired_on";
const BRIEF_COLUMNS: &str = "id, routine_id, kind, title, body, created_at, sent_to_telegram";

type RoutineRow = (
    String,
    String,
    String,
    i64,
    i64,
    bool,
    bool,
    String,
    String,
    Option<String>,
);

type BriefRow = (i64, String, String, String, String, String, bool);

/// One brief to store. Its fields travel together, so they share one struct.
pub struct NewBrief<'a> {
    pub routine: &'a RoutineId,
    pub kind: RoutineKind,
    pub title: &'a str,
    pub body: &'a str,
    pub created_at: OffsetDateTime,
    pub sent: bool,
}

impl Store {
    /// Every routine, in the order it was first saved.
    pub fn list_routines(&self) -> Result<Vec<Routine>> {
        let connection = self.connection()?;
        let rows = {
            let mut statement = connection.prepare(&format!(
                "SELECT {ROUTINE_COLUMNS} FROM routines ORDER BY rowid"
            ))?;
            let mapped = statement.query_map([], read_routine)?;
            mapped.collect::<rusqlite::Result<Vec<RoutineRow>>>()?
        };
        rows.into_iter().map(routine_from_row).collect()
    }

    pub fn get_routine(&self, id: &RoutineId) -> Result<Option<Routine>> {
        let connection = self.connection()?;
        let row = connection
            .query_row(
                &format!("SELECT {ROUTINE_COLUMNS} FROM routines WHERE id = ?1"),
                params![id.as_str()],
                read_routine,
            )
            .optional()?;
        row.map(routine_from_row).transpose()
    }

    /// Saves a new routine. `created_at` is only written here, never changed later.
    pub fn insert_routine(&self, routine: &Routine, created_at: OffsetDateTime) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO routines (id, name, kind, minute_of_day, days, enabled, to_telegram,
                                   text, backlog, last_fired_on, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                routine.id.as_str(),
                routine.name,
                enum_text(&routine.kind)?,
                i64::from(routine.minute_of_day),
                i64::from(routine.days),
                routine.enabled,
                routine.to_telegram,
                routine.text,
                routine.backlog,
                routine.last_fired_on,
                time_text(created_at)?
            ],
        )?;
        Ok(())
    }

    /// Saves every editable field of a routine. The fire date is kept as the caller set it.
    pub fn update_routine(&self, routine: &Routine) -> Result<()> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE routines SET name = ?2, kind = ?3, minute_of_day = ?4, days = ?5,
                 enabled = ?6, to_telegram = ?7, text = ?8, backlog = ?9, last_fired_on = ?10
             WHERE id = ?1",
            params![
                routine.id.as_str(),
                routine.name,
                enum_text(&routine.kind)?,
                i64::from(routine.minute_of_day),
                i64::from(routine.days),
                routine.enabled,
                routine.to_telegram,
                routine.text,
                routine.backlog,
                routine.last_fired_on
            ],
        )?;
        require_one_row(changed, routine.id.as_str())
    }

    pub fn delete_routine(&self, id: &RoutineId) -> Result<()> {
        let connection = self.connection()?;
        let changed =
            connection.execute("DELETE FROM routines WHERE id = ?1", params![id.as_str()])?;
        require_one_row(changed, id.as_str())
    }

    /// Writes the local date a routine last fired. Set before the brief is built, so a crash
    /// cannot fire it twice.
    pub fn mark_routine_fired(&self, id: &RoutineId, local_date: &str) -> Result<()> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE routines SET last_fired_on = ?2 WHERE id = ?1",
            params![id.as_str(), local_date],
        )?;
        require_one_row(changed, id.as_str())
    }

    /// Stores a brief and returns its id.
    pub fn insert_brief(&self, brief: &NewBrief<'_>) -> Result<i64> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO briefs (routine_id, kind, title, body, created_at, sent_to_telegram)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                brief.routine.as_str(),
                enum_text(&brief.kind)?,
                brief.title,
                brief.body,
                time_text(brief.created_at)?,
                brief.sent
            ],
        )?;
        Ok(connection.last_insert_rowid())
    }

    /// The newest briefs first.
    pub fn list_briefs(&self, limit: u32) -> Result<Vec<Brief>> {
        let connection = self.connection()?;
        let rows = {
            let mut statement = connection.prepare(&format!(
                "SELECT {BRIEF_COLUMNS} FROM briefs ORDER BY id DESC LIMIT ?1"
            ))?;
            let mapped = statement.query_map(params![i64::from(limit)], read_brief)?;
            mapped.collect::<rusqlite::Result<Vec<BriefRow>>>()?
        };
        rows.into_iter().map(brief_from_row).collect()
    }

    pub fn mark_brief_sent(&self, id: i64) -> Result<()> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE briefs SET sent_to_telegram = 1 WHERE id = ?1",
            params![id],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound(format!("brief {id}")));
        }
        Ok(())
    }
}

fn require_one_row(changed: usize, id: &str) -> Result<()> {
    if changed == 0 {
        return Err(StoreError::NotFound(format!("routine {id}")));
    }
    Ok(())
}

fn read_routine(row: &Row<'_>) -> rusqlite::Result<RoutineRow> {
    columns!(row; 0, 1, 2, 3, 4, 5, 6, 7, 8, 9)
}

fn routine_from_row(row: RoutineRow) -> Result<Routine> {
    let (id, name, kind, minute, days, enabled, to_telegram, text, backlog, last_fired_on) = row;
    Ok(Routine {
        id: RoutineId::new(id),
        name,
        kind: enum_from_text(&kind)?,
        minute_of_day: from_column(minute, 3)?,
        days: from_column(days, 4)?,
        enabled,
        to_telegram,
        text,
        backlog,
        last_fired_on,
    })
}

/// A stored integer that does not fit its type is a corrupt row, so it is an error, not a clamp.
fn from_column<T: TryFrom<i64>>(value: i64, column: usize) -> Result<T> {
    T::try_from(value)
        .map_err(|_| StoreError::Sqlite(rusqlite::Error::IntegralValueOutOfRange(column, value)))
}

fn read_brief(row: &Row<'_>) -> rusqlite::Result<BriefRow> {
    columns!(row; 0, 1, 2, 3, 4, 5, 6)
}

fn brief_from_row(row: BriefRow) -> Result<Brief> {
    let (id, routine, kind, title, body, created_at, sent) = row;
    Ok(Brief {
        id,
        routine: RoutineId::new(routine),
        kind: enum_from_text(&kind)?,
        title,
        body,
        created_at,
        sent_to_telegram: sent,
    })
}

#[cfg(test)]
mod tests {
    use sentinel_core::RoutineId;
    use sentinel_core::routines::{Routine, RoutineKind, WEEKDAYS};
    use time::OffsetDateTime;

    use crate::Store;
    use crate::routines::NewBrief;

    fn morning() -> Routine {
        Routine {
            id: RoutineId::new("morning"),
            name: "Morning brief".to_owned(),
            kind: RoutineKind::Morning,
            minute_of_day: 8 * 60,
            days: WEEKDAYS,
            enabled: false,
            to_telegram: true,
            text: String::new(),
            backlog: String::new(),
            last_fired_on: None,
        }
    }

    #[test]
    fn a_saved_routine_reads_back_with_its_clock_and_days() {
        let store = Store::open_in_memory().unwrap();
        store
            .insert_routine(&morning(), OffsetDateTime::UNIX_EPOCH)
            .unwrap();
        let read = store.get_routine(&RoutineId::new("morning")).unwrap();
        assert_eq!(read, Some(morning()));
        assert_eq!(store.list_routines().unwrap(), vec![morning()]);
    }

    #[test]
    fn an_update_and_a_fire_are_kept_and_a_delete_removes_the_row() {
        let store = Store::open_in_memory().unwrap();
        store
            .insert_routine(&morning(), OffsetDateTime::UNIX_EPOCH)
            .unwrap();
        let id = RoutineId::new("morning");
        let mut edited = morning();
        edited.enabled = true;
        store.update_routine(&edited).unwrap();
        store.mark_routine_fired(&id, "2026-10-25").unwrap();
        let read = store.get_routine(&id).unwrap().unwrap();
        assert!(read.enabled);
        assert_eq!(read.last_fired_on.as_deref(), Some("2026-10-25"));
        store.delete_routine(&id).unwrap();
        assert_eq!(store.get_routine(&id).unwrap(), None);
    }

    #[test]
    fn a_missing_routine_is_not_found_on_update_and_delete() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.delete_routine(&RoutineId::new("nope")).is_err());
        assert!(store.update_routine(&morning()).is_err());
    }

    #[test]
    fn briefs_list_newest_first_with_their_sent_flag() {
        let store = Store::open_in_memory().unwrap();
        let id = RoutineId::new("morning");
        let first = NewBrief {
            routine: &id,
            kind: RoutineKind::Morning,
            title: "Morning brief",
            body: "Good morning.",
            created_at: OffsetDateTime::UNIX_EPOCH,
            sent: false,
        };
        let first_id = store.insert_brief(&first).unwrap();
        store
            .insert_brief(&NewBrief {
                title: "Sundown wrap-up",
                kind: RoutineKind::Sundown,
                sent: true,
                ..first
            })
            .unwrap();
        let briefs = store.list_briefs(14).unwrap();
        assert_eq!(briefs.len(), 2);
        assert_eq!(briefs[0].title, "Sundown wrap-up");
        assert!(briefs[0].sent_to_telegram);
        store.mark_brief_sent(first_id).unwrap();
        let after = store.list_briefs(14).unwrap();
        let marked = after.iter().find(|brief| brief.id == first_id).unwrap();
        assert!(marked.sent_to_telegram);
        assert_eq!(marked.body, "Good morning.");
    }
}
