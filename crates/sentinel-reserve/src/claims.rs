//! Claim helpers: one task on one path, and single-entry scope math.

use sentinel_core::{Reservation, Scope, TaskId};

use crate::{ReserveError, Result};

/// Task and path travel together, so claim helpers take one value for both.
pub(crate) struct Claim<'a> {
    pub(crate) task: &'a TaskId,
    pub(crate) path: &'a str,
}

impl<'a> Claim<'a> {
    /// A claim over one path of one task.
    pub(crate) fn new(task: &'a TaskId, path: &'a str) -> Self {
        Self { task, path }
    }

    /// The held row that matches this exact claim, if any.
    pub(crate) fn matching_row<'r>(&self, rows: &'r [Reservation]) -> Option<&'r Reservation> {
        rows.iter()
            .find(|row| &row.task_id == self.task && row.path == self.path)
    }

    /// True when the rows already contain this exact claim.
    pub(crate) fn claimed_in(&self, rows: &[Reservation]) -> bool {
        self.matching_row(rows).is_some()
    }

    /// The tasks whose held rows overlap this claim.
    pub(crate) fn conflicting_holders(&self, held: &[Reservation]) -> Result<Vec<TaskId>> {
        let probe = entry_scope(self.path)?;
        let mut holders = Vec::new();
        for row in held {
            if &row.task_id != self.task && entry_scope(&row.path)?.overlaps(&probe) {
                holders.push(row.task_id.clone());
            }
        }
        Ok(holders)
    }
}

/// A one-entry scope for a table row or a concrete path.
pub(crate) fn entry_scope(path: &str) -> Result<Scope> {
    Scope::from_paths([path]).map_err(|error| ReserveError::Invalid(error.to_string()))
}

/// True when a concrete path is covered by a row entry.
pub(crate) fn covers(entry: &str, path: &str) -> bool {
    entry_scope(entry).is_ok_and(|scope| scope.covers(path))
}

/// True when two row entries can touch the same file.
pub(crate) fn entries_overlap(left: &str, right: &str) -> bool {
    match (entry_scope(left), entry_scope(right)) {
        (Ok(left), Ok(right)) => left.overlaps(&right),
        (Err(_), _) | (_, Err(_)) => false,
    }
}
