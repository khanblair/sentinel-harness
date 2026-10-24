//! The read side of the reservation table: queries over the held and waiting rows.

use std::collections::{BTreeMap, BTreeSet};

use sentinel_core::{Scope, TaskId};

use crate::claims::{covers, entries_overlap};
use crate::table::ReservationTable;

impl ReservationTable {
    /// The task whose granted entry covers this concrete path.
    pub fn holder_of(&self, path: &str) -> Option<TaskId> {
        self.held
            .iter()
            .find(|row| covers(&row.path, path))
            .map(|row| row.task_id.clone())
    }

    /// True when some granted entry of `task` covers `path`.
    pub fn may_write(&self, task: &TaskId, path: &str) -> bool {
        self.held
            .iter()
            .any(|row| &row.task_id == task && covers(&row.path, path))
    }

    /// The paths `task` holds, as one scope.
    pub fn held_scope(&self, task: &TaskId) -> Scope {
        let mut scope = Scope::new();
        for row in self.held.iter().filter(|row| &row.task_id == task) {
            let _ = scope.insert(&row.path);
        }
        scope
    }

    /// Every holder and its scope, sorted by task id.
    pub fn all_held(&self) -> Vec<(TaskId, Scope)> {
        let mut holders: BTreeMap<TaskId, Scope> = BTreeMap::new();
        for row in &self.held {
            let _ = holders
                .entry(row.task_id.clone())
                .or_default()
                .insert(&row.path);
        }
        holders.into_iter().collect()
    }

    /// The holders a waiting task is waiting for, sorted and without repeats.
    pub fn blockers_of(&self, task: &TaskId) -> Vec<TaskId> {
        let mut blockers = BTreeSet::new();
        for waiter in self.waiting.iter().filter(|row| &row.task_id == task) {
            for holder in &self.held {
                if &holder.task_id != task && entries_overlap(&waiter.path, &holder.path) {
                    blockers.insert(holder.task_id.clone());
                }
            }
        }
        blockers.into_iter().collect()
    }
}
