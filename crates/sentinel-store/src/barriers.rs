//! Wave barrier records. A barrier starts before a wave of tasks and finishes
//! when the whole test suite has run on the wave's commits.

use rusqlite::{Row, params};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{Result, Store, StoreError, time_from_text, time_text};

/// A stored barrier. `passed` and `ended_at` say nothing until the suite ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarrierRecord {
    pub id: i64,
    pub wave: u32,
    pub started_at: OffsetDateTime,
    pub ended_at: Option<OffsetDateTime>,
    pub passed: Option<bool>,
    pub output_ref: Option<String>,
}

const BARRIER_COLUMNS: &str = "id, wave, started_at, ended_at, passed, output_ref";

impl Store {
    /// Opens a barrier for one wave and returns its row id.
    pub fn start_barrier(&self, wave: u32, at: OffsetDateTime) -> Result<i64> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO barriers (wave, started_at) VALUES (?1, ?2)",
            params![wave, time_text(at)?],
        )?;
        Ok(connection.last_insert_rowid())
    }

    /// Closes a barrier. The suite verdict decides `passed`.
    #[allow(clippy::too_many_arguments)]
    pub fn finish_barrier(
        &self,
        id: i64,
        at: OffsetDateTime,
        passed: bool,
        output_ref: &str,
    ) -> Result<()> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE barriers SET ended_at = ?1, passed = ?2, output_ref = ?3 WHERE id = ?4",
            params![time_text(at)?, passed, output_ref, id],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound(format!("barrier {id}")));
        }
        Ok(())
    }

    /// Every barrier, oldest first.
    pub fn list_barriers(&self) -> Result<Vec<BarrierRecord>> {
        let connection = self.connection()?;
        let rows = {
            let mut statement = connection.prepare(&format!(
                "SELECT {BARRIER_COLUMNS} FROM barriers ORDER BY id"
            ))?;
            let mapped = statement.query_map([], read_barrier_row)?;
            mapped.collect::<rusqlite::Result<Vec<BarrierRow>>>()?
        };
        rows.into_iter().map(barrier_from_row).collect()
    }
}

fn read_barrier_row(row: &Row<'_>) -> rusqlite::Result<BarrierRow> {
    columns!(row; 0, 1, 2, 3, 4, 5)
}

fn barrier_from_row(row: BarrierRow) -> Result<BarrierRecord> {
    let (id, wave, started_at, ended_at, passed, output_ref) = row;
    Ok(BarrierRecord {
        id,
        wave,
        started_at: time_from_text(&started_at)?,
        ended_at: ended_at.as_deref().map(time_from_text).transpose()?,
        passed,
        output_ref,
    })
}

type BarrierRow = (
    i64,
    u32,
    String,
    Option<String>,
    Option<bool>,
    Option<String>,
);
