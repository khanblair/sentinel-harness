//! What the engine keeps while a run is live: control channels, the budget, and the questions the
//! run loop is waiting on. Everything here is replaced whole, never edited in pieces, so a panic
//! cannot leave it half written.

use std::collections::HashMap;
use std::sync::Arc;

use sentinel_core::{ApprovalId, Cost, DecisionId, HarnessState, RunInfo, TaskId, ToolId};
use sentinel_workers::Budget;
use tokio::sync::{oneshot, watch};

use crate::engine::{Engine, lock};

#[derive(Default)]
pub(crate) struct LiveRun {
    pub state: HarnessState,
    pub run: Option<RunInfo>,
    pub budget: Option<Arc<Budget>>,
    /// `true` while paused: no new task or wave starts.
    pub pause: Option<watch::Sender<bool>>,
    /// `true` once the run must stop: every worker is killed.
    pub stop: Option<watch::Sender<bool>>,
    pub task_kills: HashMap<TaskId, watch::Sender<bool>>,
    /// Which task each agent runs now.
    pub agent_task: HashMap<ToolId, TaskId>,
    pub decisions: HashMap<DecisionId, oneshot::Sender<String>>,
    pub approvals: HashMap<ApprovalId, oneshot::Sender<bool>>,
}

pub(crate) struct LiveSummary {
    pub state: HarnessState,
    pub run: Option<RunInfo>,
    /// Spend of the live run, when there is one.
    pub spent: Option<Cost>,
}

impl Engine {
    pub(crate) fn live_summary(&self) -> LiveSummary {
        let live = lock(&self.inner.live);
        LiveSummary {
            state: live.state,
            run: live.run.clone(),
            spent: live.budget.as_ref().map(|b| b.spent()),
        }
    }

    pub(crate) fn set_state(&self, state: HarnessState) {
        lock(&self.inner.live).state = state;
        self.emit_quiet(sentinel_core::Event::HarnessState { state });
    }

    /// Total spend: the live budget when a run is live, else everything the store knows.
    pub(crate) fn total_cost(&self, live_spent: &Option<Cost>) -> crate::engine::Result<Cost> {
        match live_spent {
            Some(spent) => Ok(*spent),
            None => Ok(self.inner.store.total_cost()?),
        }
    }

    pub(crate) fn cap(&self, config: &crate::config::Config) -> Cost {
        let live = lock(&self.inner.live);
        match &live.budget {
            Some(budget) => budget.cap(),
            None => config.cap().unwrap_or(Cost::ZERO),
        }
    }
}
