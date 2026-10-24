//! Run and cost queries.

use rusqlite::{Row, params};
use sentinel_core::{Cost, RunId, TaskId, ToolId};
use time::OffsetDateTime;

use crate::{Result, Store, StoreError, time_from_text, time_text};

/// One agent run with its cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRecord {
    pub id: RunId,
    pub task: TaskId,
    pub tool: ToolId,
    pub model: String,
    pub started_at: OffsetDateTime,
    pub ended_at: Option<OffsetDateTime>,
    pub exit_code: Option<i32>,
    pub cost: Cost,
}

const RUN_COLUMNS: &str =
    "id, task_id, tool_id, model, started_at, ended_at, \"exit\", cost_micro_usd, cost_estimated";

type RunRow = (
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<i32>,
    i64,
    i64,
);

impl Store {
    pub fn insert_run(&self, run: &RunRecord) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO runs
                (id, task_id, tool_id, model, started_at, ended_at, \"exit\",
                 cost_micro_usd, cost_estimated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                run.id.as_str(),
                run.task.as_str(),
                run.tool.as_str(),
                run.model,
                time_text(run.started_at)?,
                run.ended_at.map(time_text).transpose()?,
                run.exit_code,
                i64::try_from(run.cost.micro_usd()).unwrap_or(i64::MAX),
                estimate_column(run.cost),
            ],
        )?;
        Ok(())
    }

    // The public API fixes these four arguments, and clippy counts `self`.
    #[allow(clippy::too_many_arguments)]
    pub fn finish_run(
        &self,
        id: &RunId,
        ended_at: OffsetDateTime,
        exit_code: i32,
        cost: Cost,
    ) -> Result<()> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE runs SET ended_at = ?1, \"exit\" = ?2, cost_micro_usd = ?3,
                cost_estimated = ?4
             WHERE id = ?5",
            params![
                time_text(ended_at)?,
                exit_code,
                i64::try_from(cost.micro_usd()).unwrap_or(i64::MAX),
                estimate_column(cost),
                id.as_str(),
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn list_runs(&self) -> Result<Vec<RunRecord>> {
        let connection = self.connection()?;
        let rows = {
            let mut statement = connection.prepare(&format!(
                "SELECT {RUN_COLUMNS} FROM runs ORDER BY started_at, id"
            ))?;
            let mapped = statement.query_map([], read_run_row)?;
            mapped.collect::<rusqlite::Result<Vec<RunRow>>>()?
        };
        rows.into_iter().map(run_from_row).collect()
    }

    /// Sum over every run. The total is estimated when any run is.
    pub fn total_cost(&self) -> Result<Cost> {
        let connection = self.connection()?;
        let (micro, estimated) = connection.query_row(
            "SELECT COALESCE(SUM(cost_micro_usd), 0), COALESCE(MAX(cost_estimated), 0) FROM runs",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;
        Ok(cost_from_columns(micro, estimated))
    }

    /// What the agent runs that started at or after `at` cost.
    pub fn cost_since(&self, at: OffsetDateTime) -> Result<Cost> {
        let connection = self.connection()?;
        let (micro, estimated) = connection.query_row(
            "SELECT COALESCE(SUM(cost_micro_usd), 0), COALESCE(MAX(cost_estimated), 0)
             FROM runs WHERE started_at >= ?1",
            params![time_text(at)?],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;
        Ok(cost_from_columns(micro, estimated))
    }

    pub fn task_cost(&self, task: &TaskId) -> Result<Cost> {
        let connection = self.connection()?;
        let (micro, estimated) = connection.query_row(
            "SELECT COALESCE(SUM(cost_micro_usd), 0), COALESCE(MAX(cost_estimated), 0)
             FROM runs WHERE task_id = ?1",
            params![task.as_str()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;
        Ok(cost_from_columns(micro, estimated))
    }
}

fn read_run_row(row: &Row<'_>) -> rusqlite::Result<RunRow> {
    columns!(row; 0, 1, 2, 3, 4, 5, 6, 7, 8)
}

fn run_from_row(row: RunRow) -> Result<RunRecord> {
    let (id, task, tool, model, started_at, ended_at, exit_code, micro, estimated) = row;
    Ok(RunRecord {
        id: RunId::new(id),
        task: TaskId::new(task),
        tool: ToolId::new(tool),
        model,
        started_at: time_from_text(&started_at)?,
        ended_at: ended_at.as_deref().map(time_from_text).transpose()?,
        exit_code,
        cost: cost_from_columns(micro, estimated),
    })
}

fn cost_from_columns(micro_usd: i64, estimated: i64) -> Cost {
    Cost::from_micro_usd(u64::try_from(micro_usd).unwrap_or(0), estimated != 0)
}

fn estimate_column(cost: Cost) -> i64 {
    i64::from(u8::from(cost.is_estimated()))
}
