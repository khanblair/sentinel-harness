//! The scheduler: picks ready tasks, builds waves, and moves tasks through the
//! reservation lifecycle. The store is the state of record and the table is the
//! live view, so every method keeps the two in step.

use std::collections::{BTreeMap, BTreeSet};

use sentinel_core::{Dependency, Reservation, ReservationState, Task, TaskId, TaskStatus};
use sentinel_store::Store;
use time::OffsetDateTime;

use crate::table::ReservationTable;
use crate::waves::build_wave;
use crate::{Grant, ReserveError, Result};

/// The tasks chosen for one round, in ready order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wave {
    pub number: u32,
    pub tasks: Vec<TaskId>,
}

pub struct Scheduler {
    store: Store,
    table: ReservationTable,
    retry_limit: u32,
}

impl Scheduler {
    /// Load the live table from the store. A fresh run keeps three attempts.
    pub fn new(store: Store) -> Result<Scheduler> {
        let table = ReservationTable::load(store.clone())?;
        Ok(Scheduler {
            store,
            table,
            retry_limit: 3,
        })
    }

    #[must_use]
    pub fn with_retry_limit(mut self, limit: u32) -> Scheduler {
        self.retry_limit = limit;
        self
    }

    pub fn table(&self) -> &ReservationTable {
        &self.table
    }

    /// Tasks in Ready or Waiting whose dependencies have checkpointed. Order is
    /// the insertion order from the store.
    pub fn ready_tasks(&self) -> Result<Vec<Task>> {
        let tasks = self.store.list_tasks()?;
        let status = status_by_id(&tasks);
        let deps = self.store.list_dependencies()?;
        let ready = tasks
            .into_iter()
            .filter(is_ready)
            .filter(|task| deps_for(&deps, &task.id).all(|dep| dependency_met(&status, dep)))
            .collect();
        Ok(ready)
    }

    /// Tasks that can never run because a dependency ended in a dead status.
    pub fn unrunnable_tasks(&self) -> Result<Vec<TaskId>> {
        let tasks = self.store.list_tasks()?;
        let status = status_by_id(&tasks);
        let deps = self.store.list_dependencies()?;
        let stuck = tasks
            .iter()
            .filter(|task| is_ready(task))
            .filter(|task| deps_for(&deps, &task.id).any(|dep| dependency_ends(&status, dep)))
            .map(|task| task.id.clone())
            .collect();
        Ok(stuck)
    }

    /// Build a wave, reserve the files of each task, and move it to Reserved.
    /// A task that loses a race becomes Waiting and stays out of the wave.
    pub fn next_wave(
        &mut self,
        number: u32,
        max_tasks: usize,
        now: OffsetDateTime,
    ) -> Result<Wave> {
        let ready = self.ready_tasks()?;
        let chosen = build_wave(&ready, &self.table.all_held(), max_tasks);
        let mut reserved = Vec::new();
        for id in chosen {
            let Some(task) = ready.iter().find(|task| task.id == id) else {
                continue;
            };
            match self.table.try_reserve(&id, &task.scope, now)? {
                Grant::Granted(_) => {
                    self.store.set_task_status(&id, TaskStatus::Reserved)?;
                    reserved.push(id);
                }
                Grant::Waiting { .. } => self.wait_for_holder(task)?,
            }
        }
        Ok(Wave {
            number,
            tasks: reserved,
        })
    }

    /// Reserved to Running. One start is one attempt.
    pub fn start_task(&mut self, task: &TaskId) -> Result<()> {
        let current = self.require_task(task)?;
        if current.status != TaskStatus::Reserved {
            return Err(invalid_status(task, current.status));
        }
        self.store.set_task_status(task, TaskStatus::Running)?;
        self.store.increment_attempts(task)?;
        Ok(())
    }

    /// Running to Checkpointed, and the task's files go back to the pool.
    pub fn on_checkpoint(&mut self, task: &TaskId, now: OffsetDateTime) -> Result<()> {
        let current = self.require_task(task)?;
        if current.status != TaskStatus::Running {
            return Err(invalid_status(task, current.status));
        }
        self.store.set_task_status(task, TaskStatus::Checkpointed)?;
        self.table.release_task(task, now)?;
        Ok(())
    }

    /// Release the files, fail, then retry or block. Returns the final status.
    pub fn on_failure(&mut self, task: &TaskId, now: OffsetDateTime) -> Result<TaskStatus> {
        let current = self.require_task(task)?;
        if !matches!(current.status, TaskStatus::Reserved | TaskStatus::Running) {
            return Err(invalid_status(task, current.status));
        }
        self.table.release_task(task, now)?;
        self.store.set_task_status(task, TaskStatus::Failed)?;
        let next = if current.attempts < self.retry_limit {
            TaskStatus::Ready
        } else {
            TaskStatus::Blocked
        };
        self.store.set_task_status(task, next)?;
        Ok(next)
    }

    /// Scope extension. The status does not change either way.
    pub fn request_files(
        &mut self,
        task: &TaskId,
        path: &str,
        now: OffsetDateTime,
    ) -> Result<Grant> {
        self.require_task(task)?;
        self.table.request_more(task, path, now)
    }

    /// Crash resume: take back every hold, drop every wait, and requeue the
    /// tasks that were mid-flight. Returns those task ids in insertion order.
    pub fn recover_after_crash(&mut self, now: OffsetDateTime) -> Result<Vec<TaskId>> {
        let tasks = self.store.list_tasks()?;
        let requeue = tasks
            .iter()
            .filter(|task| {
                matches!(
                    task.status,
                    TaskStatus::Reserved | TaskStatus::Running | TaskStatus::Waiting
                )
            })
            .map(|task| task.id.clone())
            .collect::<Vec<TaskId>>();
        let holders = self
            .table
            .all_held()
            .into_iter()
            .map(|(id, _)| id)
            .collect::<Vec<TaskId>>();
        for holder in holders {
            self.table.revoke_task(&holder, now)?;
        }
        let waiting = self.store.list_reservations(&[ReservationState::Waiting])?;
        for waiter in waiting_ids(&waiting) {
            self.table.abandon_waiting(&waiter, now)?;
        }
        for id in &requeue {
            self.store.set_task_status(id, TaskStatus::Ready)?;
        }
        Ok(requeue)
    }

    fn require_task(&self, task: &TaskId) -> Result<Task> {
        self.store
            .get_task(task)?
            .ok_or_else(|| ReserveError::NotFound(task.to_string()))
    }

    /// A task that already waits keeps its status and its Waiting rows.
    fn wait_for_holder(&self, task: &Task) -> Result<()> {
        if task.status == TaskStatus::Ready {
            self.store.set_task_status(&task.id, TaskStatus::Waiting)?;
        }
        Ok(())
    }
}

fn is_ready(task: &Task) -> bool {
    matches!(task.status, TaskStatus::Ready | TaskStatus::Waiting)
}

fn status_by_id(tasks: &[Task]) -> BTreeMap<TaskId, TaskStatus> {
    tasks
        .iter()
        .map(|task| (task.id.clone(), task.status))
        .collect()
}

fn deps_for<'a>(deps: &'a [Dependency], task: &'a TaskId) -> impl Iterator<Item = &'a Dependency> {
    deps.iter().filter(move |dep| &dep.task == task)
}

/// Both dependency kinds wait for the dependency's checkpoint.
fn dependency_met(status: &BTreeMap<TaskId, TaskStatus>, dep: &Dependency) -> bool {
    matches!(
        status.get(&dep.depends_on),
        Some(TaskStatus::Checkpointed | TaskStatus::Verifying | TaskStatus::Verified)
    )
}

/// A dependency in one of these states will never checkpoint.
fn dependency_ends(status: &BTreeMap<TaskId, TaskStatus>, dep: &Dependency) -> bool {
    matches!(
        status.get(&dep.depends_on),
        Some(
            TaskStatus::Blocked
                | TaskStatus::Skipped
                | TaskStatus::Contradiction
                | TaskStatus::Partial
        )
    )
}

fn waiting_ids(rows: &[Reservation]) -> Vec<TaskId> {
    let ids: BTreeSet<TaskId> = rows.iter().map(|row| row.task_id.clone()).collect();
    ids.into_iter().collect()
}

fn invalid_status(task: &TaskId, status: TaskStatus) -> ReserveError {
    ReserveError::Invalid(format!("task {task} is {status:?}"))
}
