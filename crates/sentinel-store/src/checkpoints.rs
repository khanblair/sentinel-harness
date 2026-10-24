//! Checkpoint records: one commit per task per point in time. The `id` column
//! from migration 1 counts up as `CK-<n>` so rows stay sortable.

use rusqlite::{OptionalExtension, Row, params};
use sentinel_core::TaskId;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{Result, Store, time_from_text, time_text};

/// A stored checkpoint: what a task committed, and the files the commit touched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRecord {
    pub task: TaskId,
    pub commit: String,
    pub paths: Vec<String>,
    pub at: OffsetDateTime,
}

const CHECKPOINT_COLUMNS: &str = "task_id, commit_sha, paths_json, created_at";

impl Store {
    pub fn insert_checkpoint(&self, record: &CheckpointRecord) -> Result<()> {
        let connection = self.connection()?;
        let id = next_checkpoint_id(&connection)?;
        let paths_json = serde_json::to_string(&record.paths)?;
        connection.execute(
            "INSERT INTO checkpoints (id, task_id, commit_sha, paths_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                record.task.as_str(),
                record.commit,
                paths_json,
                time_text(record.at)?
            ],
        )?;
        Ok(())
    }

    /// Every checkpoint on this host, oldest first.
    pub fn list_checkpoints(&self) -> Result<Vec<CheckpointRecord>> {
        let connection = self.connection()?;
        let rows = {
            let mut statement = connection.prepare(&format!(
                "SELECT {CHECKPOINT_COLUMNS} FROM checkpoints ORDER BY rowid"
            ))?;
            let mapped = statement.query_map([], read_checkpoint_row)?;
            mapped.collect::<rusqlite::Result<Vec<CheckpointRow>>>()?
        };
        checkpoints_from_rows(rows)
    }

    /// The newest checkpoint of one task, or `None` before its first commit.
    pub fn latest_checkpoint(&self, task: &TaskId) -> Result<Option<CheckpointRecord>> {
        let connection = self.connection()?;
        let row = connection
            .query_row(
                &format!(
                    "SELECT {CHECKPOINT_COLUMNS} FROM checkpoints \
                     WHERE task_id = ?1 ORDER BY rowid DESC LIMIT 1"
                ),
                params![task.as_str()],
                read_checkpoint_row,
            )
            .optional()?;
        row.map(checkpoint_from_row).transpose()
    }
}

fn next_checkpoint_id(connection: &rusqlite::Connection) -> Result<String> {
    let next: i64 = connection.query_row(
        "SELECT COALESCE(MAX(rowid), 0) + 1 FROM checkpoints",
        [],
        |row| row.get(0),
    )?;
    Ok(format!("CK-{next}"))
}

fn checkpoints_from_rows(rows: Vec<CheckpointRow>) -> Result<Vec<CheckpointRecord>> {
    rows.into_iter().map(checkpoint_from_row).collect()
}

fn read_checkpoint_row(row: &Row<'_>) -> rusqlite::Result<CheckpointRow> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
}

fn checkpoint_from_row(row: CheckpointRow) -> Result<CheckpointRecord> {
    let (task, commit, paths_json, at) = row;
    Ok(CheckpointRecord {
        task: TaskId::new(task),
        commit,
        paths: serde_json::from_str(&paths_json)?,
        at: time_from_text(&at)?,
    })
}

type CheckpointRow = (String, String, String, String);
