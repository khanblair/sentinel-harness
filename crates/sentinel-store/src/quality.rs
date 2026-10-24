//! Quality notes: smell and slop warnings on agent code. They never change a
//! verdict, so the store only adds and lists them.

use rusqlite::{Row, params};
use sentinel_core::{QualityNote, TaskId};
use time::OffsetDateTime;

use crate::{Result, Store, enum_from_text, enum_text, time_text};

const QUALITY_NOTE_COLUMNS: &str = "task_id, kind, location, detail_code";

type NoteRow = (String, String, String, String);

impl Store {
    pub fn insert_quality_note(&self, note: &QualityNote, at: OffsetDateTime) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO quality_notes (task_id, kind, location, detail_code, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                note.task.as_str(),
                enum_text(&note.kind)?,
                note.location,
                note.detail_code,
                time_text(at)?
            ],
        )?;
        Ok(())
    }

    /// Notes of one task, or every note when `task` is `None`. Oldest first.
    pub fn list_quality_notes(&self, task: Option<&TaskId>) -> Result<Vec<QualityNote>> {
        let connection = self.connection()?;
        let sql = note_filter(task);
        let rows = {
            let mut statement = connection.prepare(&sql)?;
            let mapped = match task {
                Some(task) => statement.query_map(params![task.as_str()], read_note_row)?,
                None => statement.query_map([], read_note_row)?,
            };
            mapped.collect::<rusqlite::Result<Vec<NoteRow>>>()?
        };
        rows.into_iter().map(note_from_row).collect()
    }
}

fn note_filter(task: Option<&TaskId>) -> String {
    match task {
        Some(_) => format!(
            "SELECT {QUALITY_NOTE_COLUMNS} FROM quality_notes WHERE task_id = ?1 ORDER BY rowid"
        ),
        None => format!("SELECT {QUALITY_NOTE_COLUMNS} FROM quality_notes ORDER BY rowid"),
    }
}

fn read_note_row(row: &Row<'_>) -> rusqlite::Result<NoteRow> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
}

fn note_from_row(row: NoteRow) -> Result<QualityNote> {
    let (task, kind, location, detail_code) = row;
    Ok(QualityNote {
        task: TaskId::new(task),
        kind: enum_from_text(&kind)?,
        location,
        detail_code,
    })
}
