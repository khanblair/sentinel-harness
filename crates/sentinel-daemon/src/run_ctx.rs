//! Everything the tasks of one run share. A run owns one `RunCtx`; every wave and attempt gets an
//! `Arc` of it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sentinel_core::{PermissionMode, RunId, TaskId, Tier, ToolId};
use sentinel_gate::ApprovalBook;
use sentinel_reserve::Scheduler;
use sentinel_tree::{Checkpoint, Git, WritePolicy, WriteVerdict};
use sentinel_verify::verifier::DiffReviewer;
use sentinel_workers::Budget;
use sentinel_workers::context::ContextLimits;
use tokio::sync::watch;

use crate::config::Config;
use crate::engine::{Engine, lock};

/// What a run remembers between attempts of one task.
#[derive(Default)]
pub(crate) struct RunMemory {
    /// The last failure summary, fed to the next attempt of the task.
    pub failures: HashMap<TaskId, String>,
    pub checkpoints: HashMap<TaskId, Checkpoint>,
    /// The tier of the next attempt. It climbs one step after each failure.
    pub tiers: HashMap<TaskId, Tier>,
    pub agents: HashMap<TaskId, ToolId>,
}

pub(crate) struct RunCtx {
    pub engine: Engine,
    pub run_id: RunId,
    pub git: Git,
    pub scheduler: Arc<Mutex<Scheduler>>,
    pub budget: Arc<Budget>,
    pub stop: watch::Receiver<bool>,
    pub pause: watch::Receiver<bool>,
    pub limits: ContextLimits,
    pub config: Config,
    pub mode: Mutex<PermissionMode>,
    pub book: Mutex<ApprovalBook>,
    pub memory: Arc<Mutex<RunMemory>>,
    pub reviewer: Arc<dyn DiffReviewer>,
    pub suite_dir: PathBuf,
}

impl RunCtx {
    pub fn stopped(&self) -> bool {
        *self.stop.borrow()
    }

    pub fn task_timeout(&self) -> Duration {
        Duration::from_secs(self.config.task_timeout_seconds)
    }

    pub fn permission_mode(&self) -> PermissionMode {
        *lock(&self.mode)
    }

    /// Block while the run is paused. Returns early when the run is stopped.
    pub async fn wait_unpaused(&self) {
        let mut pause = self.pause.clone();
        let mut stop = self.stop.clone();
        while *pause.borrow() && !*stop.borrow() {
            tokio::select! {
                _ = pause.changed() => {}
                _ = stop.changed() => {}
            }
        }
    }
}

/// The write guard's rule: a path covered by any held reservation is some task's to write. Any
/// other path is blocked and restored. (A file watcher cannot tell which agent wrote, so a path
/// held by task A is also open to task B's writes. MCP file tools close that gap.)
pub(crate) struct RunPolicy {
    pub scheduler: Arc<Mutex<Scheduler>>,
}

impl WritePolicy for RunPolicy {
    fn check(&self, path: &str) -> WriteVerdict {
        let holder = lock(&self.scheduler).table().holder_of(path);
        match holder {
            Some(_) => WriteVerdict::Allowed,
            None => WriteVerdict::Blocked { holder: None },
        }
    }
}

/// One kill signal that turns `true` when any source does: the run stop, the budget kill at 95
/// percent, or a kill of this one task.
pub(crate) fn merge_kill(sources: Vec<watch::Receiver<bool>>) -> watch::Receiver<bool> {
    let (sender, receiver) = watch::channel(sources.iter().any(|s| *s.borrow()));
    tokio::spawn(async move {
        let mut sources = sources;
        loop {
            if sources.iter().any(|s| *s.borrow()) {
                sender.send_replace(true);
                return;
            }
            if sources.is_empty() {
                return;
            }
            let waits = sources.iter_mut().map(|s| Box::pin(s.changed()));
            tokio::select! {
                (changed, index, _rest) = futures::future::select_all(waits) => {
                    if changed.is_err() {
                        sources.swap_remove(index);
                    }
                }
                () = sender.closed() => return,
            }
        }
    });
    receiver
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn merged_kill_turns_true_when_any_source_does() {
        let (first_tx, first_rx) = watch::channel(false);
        let (_second_tx, second_rx) = watch::channel(false);
        let mut merged = merge_kill(vec![first_rx, second_rx]);
        assert!(!*merged.borrow());
        first_tx.send_replace(true);
        merged.wait_for(|killed| *killed).await.unwrap();
        assert!(*merged.borrow());
    }

    #[tokio::test]
    async fn merged_kill_starts_true_when_a_source_is_already_true() {
        let (_tx, rx) = watch::channel(true);
        assert!(*merge_kill(vec![rx]).borrow());
    }

    #[tokio::test]
    async fn merged_kill_survives_a_dropped_source() {
        let (dropped_tx, dropped_rx) = watch::channel(false);
        let (live_tx, live_rx) = watch::channel(false);
        let mut merged = merge_kill(vec![dropped_rx, live_rx]);
        drop(dropped_tx);
        live_tx.send_replace(true);
        merged.wait_for(|killed| *killed).await.unwrap();
    }
}
