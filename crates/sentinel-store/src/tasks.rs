//! Task and dependency queries.

use rusqlite::{OptionalExtension, Row, params};
use sentinel_core::{Dependency, Task, TaskId, TaskStatus, Tier};

use crate::{Result, Store, StoreError, enum_from_text, enum_text};

const TASK_COLUMNS: &str = "id, title, body, status, tier, attempts, scope_json, flags";

type TaskRow = (String, String, String, String, String, u32, String, String);

impl Store {
    pub fn upsert_task(&self, task: &Task) -> Result<()> {
        let connection = self.connection()?;
        let scope_json = serde_json::to_string(&task.scope)?;
        let flags_json = serde_json::to_string(&task.flags)?;
        connection.execute(
            "INSERT INTO tasks (id, title, body, status, tier, attempts, scope_json, flags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                body = excluded.body,
                status = excluded.status,
                tier = excluded.tier,
                attempts = excluded.attempts,
                scope_json = excluded.scope_json,
                flags = excluded.flags",
            params![
                task.id.as_str(),
                task.title,
                task.body,
                enum_text(&task.status)?,
                enum_text(&task.tier)?,
                task.attempts,
                scope_json,
                flags_json
            ],
        )?;
        Ok(())
    }

    pub fn get_task(&self, id: &TaskId) -> Result<Option<Task>> {
        let connection = self.connection()?;
        let row = connection
            .query_row(
                &format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?1"),
                params![id.as_str()],
                read_task_row,
            )
            .optional()?;
        row.map(task_from_row).transpose()
    }

    pub fn list_tasks(&self) -> Result<Vec<Task>> {
        let connection = self.connection()?;
        let rows = {
            let mut statement = connection.prepare(&format!(
                "SELECT {TASK_COLUMNS} FROM tasks ORDER BY ordinal"
            ))?;
            let mapped = statement.query_map([], read_task_row)?;
            mapped.collect::<rusqlite::Result<Vec<TaskRow>>>()?
        };
        rows.into_iter().map(task_from_row).collect()
    }

    pub fn set_task_status(&self, id: &TaskId, next: TaskStatus) -> Result<()> {
        let connection = self.connection()?;
        let stored = connection
            .query_row(
                "SELECT status FROM tasks WHERE id = ?1",
                params![id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
        let current: TaskStatus = enum_from_text(&stored)?;
        if current == next {
            // Saying the same thing twice is not a change, for example after a crash.
            return Ok(());
        }
        let accepted = current.move_to(next)?;
        connection.execute(
            "UPDATE tasks SET status = ?1 WHERE id = ?2",
            params![enum_text(&accepted)?, id.as_str()],
        )?;
        Ok(())
    }

    pub fn set_task_tier(&self, id: &TaskId, tier: Tier) -> Result<()> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE tasks SET tier = ?1 WHERE id = ?2",
            params![enum_text(&tier)?, id.as_str()],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    /// Take back one try that failed for a reason that is not the task's, such as an agent with no
    /// credit. The count never goes below zero.
    pub fn give_back_attempt(&self, id: &TaskId) -> Result<()> {
        self.connection()?.execute(
            "UPDATE tasks SET attempts = MAX(attempts - 1, 0) WHERE id = ?1",
            params![id.as_str()],
        )?;
        Ok(())
    }

    pub fn increment_attempts(&self, id: &TaskId) -> Result<u32> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE tasks SET attempts = attempts + 1 WHERE id = ?1",
            params![id.as_str()],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        let attempts = connection.query_row(
            "SELECT attempts FROM tasks WHERE id = ?1",
            params![id.as_str()],
            |row| row.get(0),
        )?;
        Ok(attempts)
    }

    pub fn add_dependency(&self, dep: &Dependency) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT OR REPLACE INTO task_deps (task_id, depends_on, kind)
             VALUES (?1, ?2, ?3)",
            params![
                dep.task.as_str(),
                dep.depends_on.as_str(),
                enum_text(&dep.kind)?
            ],
        )?;
        Ok(())
    }

    pub fn list_dependencies(&self) -> Result<Vec<Dependency>> {
        let connection = self.connection()?;
        let rows = {
            let mut statement = connection
                .prepare("SELECT task_id, depends_on, kind FROM task_deps ORDER BY rowid")?;
            let mapped = statement.query_map([], read_dependency_row)?;
            mapped.collect::<rusqlite::Result<Vec<DependencyRow>>>()?
        };
        rows.into_iter().map(dependency_from_row).collect()
    }
}

type DependencyRow = (String, String, String);

fn read_task_row(row: &Row<'_>) -> rusqlite::Result<TaskRow> {
    columns!(row; 0, 1, 2, 3, 4, 5, 6, 7)
}

fn task_from_row(row: TaskRow) -> Result<Task> {
    let (id, title, body, status, tier, attempts, scope_json, flags_json) = row;
    Ok(Task {
        id: TaskId::new(id),
        title,
        body,
        status: enum_from_text(&status)?,
        tier: enum_from_text(&tier)?,
        attempts,
        scope: serde_json::from_str(&scope_json)?,
        flags: serde_json::from_str(&flags_json)?,
    })
}

fn read_dependency_row(row: &Row<'_>) -> rusqlite::Result<DependencyRow> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
}

fn dependency_from_row(row: DependencyRow) -> Result<Dependency> {
    let (task, depends_on, kind) = row;
    Ok(Dependency {
        task: TaskId::new(task),
        depends_on: TaskId::new(depends_on),
        kind: enum_from_text(&kind)?,
    })
}
