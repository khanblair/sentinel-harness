//! Clear what belongs to one run, so the next run starts from an empty board. Events, runs, cost
//! history, devices, and settings stay.

use crate::{Result, Store};

/// Tables that hold one run's tasks and what happened to them.
const RUN_TABLES: [&str; 9] = [
    "quality_notes",
    "verifications",
    "checkpoints",
    "barriers",
    "approvals",
    "decisions",
    "reservations",
    "task_deps",
    "tasks",
];

impl Store {
    pub fn reset_for_new_run(&self) -> Result<()> {
        let connection = self.connection()?;
        for table in RUN_TABLES {
            connection.execute(&format!("DELETE FROM {table}"), [])?;
        }
        Ok(())
    }
}
