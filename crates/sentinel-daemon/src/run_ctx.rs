//! Everything the tasks of one run share. A run owns one `RunCtx`; every wave and attempt gets an
//! `Arc` of it.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sentinel_core::{ChatId, PermissionMode, TaskId, Tier, ToolId};
use sentinel_gate::ApprovalBook;
use sentinel_reserve::Scheduler;
use sentinel_tree::{Checkpoint, Git, WritePolicy, WriteVerdict, revert_checkpoint};
use sentinel_verify::verifier::DiffReviewer;
use sentinel_workers::Budget;
use sentinel_workers::context::ContextLimits;
use tokio::sync::watch;

use crate::config::Config;
use crate::engine::{Engine, Result, lock};
use crate::run_source::RunSource;

impl Engine {
    /// The agent a chat is set to. Its runs, its questions, and its type step all use it. `None`
    /// means auto: the routing picks.
    pub(crate) fn chat_agent(&self, chat: &ChatId) -> Result<Option<ToolId>> {
        Ok(self.chat_record(chat)?.agent.map(ToolId::new))
    }
}

/// What a run remembers between attempts of one task.
#[derive(Default)]
pub(crate) struct RunMemory {
    /// The last failure summary, fed to the next attempt of the task.
    pub failures: HashMap<TaskId, String>,
    pub checkpoints: HashMap<TaskId, Checkpoint>,
    /// The tier of the next attempt. It climbs one step after each failure.
    pub tiers: HashMap<TaskId, Tier>,
    pub agents: HashMap<TaskId, ToolId>,
    /// The turn of each task, in the order the run first picked it. A roster gives turn `k` to
    /// `roster[k % n]`, so every agent gets work.
    pub turns: HashMap<TaskId, usize>,
    /// The failed tries of each task. A roster moves each one to a stronger agent.
    pub failed_tries: HashMap<TaskId, u32>,
}

impl RunMemory {
    /// The turn of a task. Its first pick takes the next free turn, and later picks keep it.
    pub(crate) fn turn_of(&mut self, task: &TaskId) -> usize {
        let next = self.turns.len();
        *self.turns.entry(task.clone()).or_insert(next)
    }

    pub(crate) fn failures_of(&self, task: &TaskId) -> u32 {
        self.failed_tries.get(task).copied().unwrap_or(0)
    }

    pub(crate) fn record_failed_try(&mut self, task: &TaskId) {
        *self.failed_tries.entry(task.clone()).or_insert(0) += 1;
    }
}

pub(crate) struct RunCtx {
    pub engine: Engine,
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
    /// Where the run came from: the chat, the backlog or words, and the attached context.
    pub source: RunSource,
    /// True when this run picks up a run that stopped without finishing.
    pub resuming: bool,
    /// The chat's agent, read when the run starts. Every task and plan of the run goes to it alone.
    pub forced: Option<ToolId>,
    /// Checkpoints share one private git index, so only one may stage, write, and commit at a time.
    pub checkpoint_lock: Mutex<()>,
    /// Paths a revert is rewriting. The write guard leaves them alone, so it cannot undo the revert.
    pub reverting: Arc<Mutex<HashSet<String>>>,
}

/// How long the guard leaves a reverted path alone. The guard collects events for 100 ms first.
const REVERT_QUIET: Duration = Duration::from_millis(400);

impl RunCtx {
    /// Undo a checkpoint. The guard watches the same tree and would restore the files that the
    /// revert writes from the old HEAD, which brings the bad code back. So the paths are open to
    /// writes until the revert is committed and the guard has caught up.
    pub(crate) fn revert_quietly(
        &self,
        target: &Checkpoint,
        message: &str,
    ) -> sentinel_tree::Result<Option<Checkpoint>> {
        lock(&self.reverting).extend(target.paths.iter().cloned());
        let result = revert_checkpoint(&self.git, target, message);
        let held = self.reverting.clone();
        let paths = target.paths.clone();
        tokio::spawn(async move {
            tokio::time::sleep(REVERT_QUIET).await;
            let mut open = lock(&held);
            for path in &paths {
                open.remove(path);
            }
        });
        result
    }

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
    pub reverting: Arc<Mutex<HashSet<String>>>,
}

impl WritePolicy for RunPolicy {
    fn check(&self, path: &str) -> WriteVerdict {
        if lock(&self.reverting).contains(path) {
            return WriteVerdict::Allowed;
        }
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
    tokio::spawn(forward_kill(sources, sender));
    receiver
}

/// Wait on every source. When one turns `true`, send `true`. A source whose sender is gone is
/// dropped. Stop when nothing is left or nobody listens any more.
async fn forward_kill(mut sources: Vec<watch::Receiver<bool>>, sender: watch::Sender<bool>) {
    loop {
        if sources.iter().any(|s| *s.borrow()) {
            sender.send_replace(true);
            return;
        }
        if sources.is_empty() {
            return;
        }
        let Some(gone) = next_change(&mut sources, &sender).await else {
            return;
        };
        if let Some(index) = gone {
            sources.swap_remove(index);
        }
    }
}

/// Wait for one source to change. `None`: nobody listens to the merged signal any more.
/// `Some(Some(index))`: that source's sender is gone. `Some(None)`: a source changed.
async fn next_change(
    sources: &mut [watch::Receiver<bool>],
    sender: &watch::Sender<bool>,
) -> Option<Option<usize>> {
    let waits = sources.iter_mut().map(|s| Box::pin(s.changed()));
    tokio::select! {
        (changed, index, _) = futures::future::select_all(waits) => Some(changed.is_err().then_some(index)),
        () = sender.closed() => None,
    }
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

    #[test]
    fn a_task_keeps_the_turn_of_its_first_pick() {
        let mut memory = RunMemory::default();
        let first = TaskId::new("T-01");
        let second = TaskId::new("T-02");
        assert_eq!(memory.turn_of(&first), 0);
        assert_eq!(memory.turn_of(&second), 1);
        assert_eq!(memory.turn_of(&first), 0);
    }

    #[test]
    fn each_failed_try_of_a_task_is_counted_for_that_task_only() {
        let mut memory = RunMemory::default();
        let task = TaskId::new("T-01");
        memory.record_failed_try(&task);
        memory.record_failed_try(&task);
        assert_eq!(memory.failures_of(&task), 2);
        assert_eq!(memory.failures_of(&TaskId::new("T-02")), 0);
    }
}
