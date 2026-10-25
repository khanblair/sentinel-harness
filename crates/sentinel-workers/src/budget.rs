//! The hard budget guard. Spend can never pass the cap. The cap only goes down.
//!
//! At 70 percent no new task starts. At 95 percent every worker is told to stop through one
//! shared signal. Each task also has a ceiling, so one runaway CLI cannot eat the whole cap.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use sentinel_core::{Cost, TaskId, Tier};
use thiserror::Error;
use tokio::sync::watch;

const NO_NEW_TASKS_PERCENT: u8 = 70;
const KILL_PERCENT: u8 = 95;
/// How many times a tier's typical task cost a task may spend before it is killed.
const CEILING_FACTOR: u64 = 2;
const SECONDS_PER_HOUR: u64 = 3_600;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BudgetError {
    #[error("the cap can only be lowered: {requested} is above {current}")]
    CapRaise { requested: Cost, current: Cost },
}

/// Something a caller should react to after recording spend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetEvent {
    /// 70 percent of the cap is spent. No new task should start.
    Threshold70,
    /// 95 percent is spent. The kill signal is now set.
    Threshold95,
    CeilingHit {
        task: TaskId,
        cost: Cost,
        ceiling: Cost,
    },
}

struct State {
    cap: Cost,
    spent: Cost,
    per_task: HashMap<TaskId, Cost>,
    ceilings: HashMap<TaskId, Cost>,
    fired_70: bool,
    fired_95: bool,
    /// Events not yet read by `drain_events`, so the engine can turn them into stream events.
    pending: Vec<BudgetEvent>,
}

pub struct Budget {
    state: Mutex<State>,
    kill: watch::Sender<bool>,
}

impl Budget {
    /// A new guard and the receiver every worker listens on.
    pub fn new(cap: Cost) -> (Arc<Budget>, watch::Receiver<bool>) {
        let (kill, receiver) = watch::channel(false);
        let state = State {
            cap,
            spent: Cost::ZERO,
            per_task: HashMap::new(),
            ceilings: HashMap::new(),
            fired_70: false,
            fired_95: false,
            pending: Vec::new(),
        };
        (
            Arc::new(Budget {
                state: Mutex::new(state),
                kill,
            }),
            receiver,
        )
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic while holding the lock cannot leave the counters half-updated: every update
        // is a single assignment. Keep going with the inner value.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn cap(&self) -> Cost {
        self.lock().cap
    }

    pub fn spent(&self) -> Cost {
        self.lock().spent
    }

    pub fn task_spent(&self, task: &TaskId) -> Cost {
        self.lock()
            .per_task
            .get(task)
            .copied()
            .unwrap_or(Cost::ZERO)
    }

    pub fn receiver(&self) -> watch::Receiver<bool> {
        self.kill.subscribe()
    }

    /// Lower the cap. Raising it is refused, from every route.
    pub fn lower_cap(&self, new_cap: Cost) -> Result<(), BudgetError> {
        let mut state = self.lock();
        if new_cap > state.cap {
            return Err(BudgetError::CapRaise {
                requested: new_cap,
                current: state.cap,
            });
        }
        state.cap = new_cap;
        Ok(())
    }

    /// False once 70 percent is spent.
    pub fn can_start_task(&self) -> bool {
        let state = self.lock();
        state.spent.percent_of(state.cap) < NO_NEW_TASKS_PERCENT
    }

    /// Count spend from before a restart, so the cap holds across a crash. No task owns it.
    pub fn seed(&self, spent: Cost) {
        let mut state = self.lock();
        state.spent = state.spent + spent;
    }

    pub fn set_ceiling(&self, task: &TaskId, ceiling: Cost) {
        self.lock().ceilings.insert(task.clone(), ceiling);
    }

    /// Add spend for a task. Returns the events this spend caused, each at most once per run
    /// (a ceiling event repeats while the task stays over its ceiling).
    pub fn record(&self, task: &TaskId, cost: Cost) -> Vec<BudgetEvent> {
        let mut state = self.lock();
        let task_total = state.per_task.get(task).copied().unwrap_or(Cost::ZERO) + cost;
        state.per_task.insert(task.clone(), task_total);
        let mut events = self.add_spend(&mut state, cost);
        if let Some(ceiling) = state
            .ceilings
            .get(task)
            .copied()
            .filter(|c| task_total > *c)
        {
            events.push(BudgetEvent::CeilingHit {
                task: task.clone(),
                cost: task_total,
                ceiling,
            });
        }
        state.pending.extend(events.iter().cloned());
        events
    }

    /// Add a time guess to the total spend. A guess never counts toward a task's ceiling, so it
    /// cannot end a run by itself. The cap still sees it.
    pub fn record_guess(&self, cost: Cost) -> Vec<BudgetEvent> {
        let mut state = self.lock();
        let events = self.add_spend(&mut state, cost);
        state.pending.extend(events.iter().cloned());
        events
    }

    /// Count spend toward the cap. Fires the 70 and 95 percent thresholds once each.
    fn add_spend(&self, state: &mut State, cost: Cost) -> Vec<BudgetEvent> {
        state.spent = state.spent + cost;
        let percent = state.spent.percent_of(state.cap);
        let mut events = Vec::new();
        if percent >= NO_NEW_TASKS_PERCENT && !state.fired_70 {
            state.fired_70 = true;
            events.push(BudgetEvent::Threshold70);
        }
        if percent >= KILL_PERCENT && !state.fired_95 {
            state.fired_95 = true;
            self.kill.send_replace(true);
            events.push(BudgetEvent::Threshold95);
        }
        events
    }

    /// Every event since the last call. The engine polls this to emit `BudgetThreshold` and
    /// `TaskCeilingHit`, because the supervisor records spend without knowing about the stream.
    pub fn drain_events(&self) -> Vec<BudgetEvent> {
        std::mem::take(&mut self.lock().pending)
    }
}

/// Typical spend for one task of this tier. The per-task ceiling is twice this.
pub fn tier_estimate(tier: Tier) -> Cost {
    let micro = match tier {
        Tier::Cheap => 20_000,
        Tier::Mid => 80_000,
        Tier::Smart => 300_000,
    };
    Cost::from_micro_usd(micro, true)
}

pub fn default_ceiling(tier: Tier) -> Cost {
    Cost::from_micro_usd(tier_estimate(tier).micro_usd() * CEILING_FACTOR, true)
}

/// A high guess for a CLI that reports no dollar figure: a rate per second by tier.
pub fn estimate_by_time(tier: Tier, elapsed: Duration) -> Cost {
    Cost::from_micro_usd(
        guess_micro_per_second(tier).saturating_mul(elapsed.as_secs()),
        true,
    )
}

/// The guess for one hour of running. A run on a time guess alone shows this rate to the user.
pub fn guess_per_hour(tier: Tier) -> Cost {
    let micro = guess_micro_per_second(tier).saturating_mul(SECONDS_PER_HOUR);
    Cost::from_micro_usd(micro, true)
}

fn guess_micro_per_second(tier: Tier) -> u64 {
    match tier {
        Tier::Cheap => 150,
        Tier::Mid => 500,
        Tier::Smart => 1_500,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usd(text: &str) -> Cost {
        Cost::parse_usd(text).unwrap()
    }

    fn task() -> TaskId {
        TaskId::new("T-01")
    }

    #[test]
    fn thresholds_fire_once_and_95_sets_the_kill_signal() {
        let (budget, kill) = Budget::new(usd("2.00"));
        assert_eq!(
            budget.record(&task(), usd("1.00")),
            Vec::<BudgetEvent>::new()
        );
        assert_eq!(
            budget.record(&task(), usd("0.40")),
            vec![BudgetEvent::Threshold70]
        );
        assert!(!*kill.borrow());
        assert_eq!(
            budget.record(&task(), usd("0.50")),
            vec![BudgetEvent::Threshold95]
        );
        assert!(*kill.borrow());
        assert_eq!(
            budget.record(&task(), usd("0.05")),
            Vec::<BudgetEvent>::new()
        );
    }

    #[test]
    fn drain_events_returns_each_event_once() {
        let (budget, _kill) = Budget::new(usd("1.00"));
        budget.record(&task(), usd("0.75"));
        assert_eq!(budget.drain_events(), vec![BudgetEvent::Threshold70]);
        assert_eq!(budget.drain_events(), Vec::<BudgetEvent>::new());
    }

    #[test]
    fn no_new_task_starts_after_70_percent() {
        let (budget, _kill) = Budget::new(usd("1.00"));
        assert!(budget.can_start_task());
        budget.record(&task(), usd("0.70"));
        assert!(!budget.can_start_task());
    }

    #[test]
    fn cap_can_be_lowered_but_never_raised() {
        let (budget, _kill) = Budget::new(usd("2.00"));
        assert!(budget.lower_cap(usd("1.50")).is_ok());
        assert_eq!(budget.cap(), usd("1.50"));
        assert!(matches!(
            budget.lower_cap(usd("1.51")),
            Err(BudgetError::CapRaise { .. })
        ));
        assert_eq!(budget.cap(), usd("1.50"));
    }

    #[test]
    fn lowering_the_cap_can_trip_a_threshold() {
        let (budget, kill) = Budget::new(usd("2.00"));
        budget.record(&task(), usd("1.00"));
        budget.lower_cap(usd("1.00")).unwrap();
        let events = budget.record(&task(), Cost::ZERO);
        assert!(events.contains(&BudgetEvent::Threshold95));
        assert!(*kill.borrow());
    }

    #[test]
    fn ceiling_is_reported_while_a_task_stays_over_it() {
        let (budget, _kill) = Budget::new(usd("10.00"));
        budget.set_ceiling(&task(), usd("0.10"));
        assert_eq!(
            budget.record(&task(), usd("0.05")),
            Vec::<BudgetEvent>::new()
        );
        let events = budget.record(&task(), usd("0.06"));
        assert_eq!(
            events,
            vec![BudgetEvent::CeilingHit {
                task: task(),
                cost: usd("0.11"),
                ceiling: usd("0.10")
            }]
        );
        assert_eq!(budget.task_spent(&task()), usd("0.11"));
        assert_eq!(budget.spent(), usd("0.11"));
    }

    #[test]
    fn default_ceiling_is_twice_the_tier_estimate() {
        for tier in [Tier::Cheap, Tier::Mid, Tier::Smart] {
            assert_eq!(
                default_ceiling(tier).micro_usd(),
                tier_estimate(tier).micro_usd() * 2
            );
            assert!(default_ceiling(tier).is_estimated());
        }
    }

    #[test]
    fn time_estimates_grow_with_tier_and_time() {
        let minute = Duration::from_secs(60);
        assert!(estimate_by_time(Tier::Smart, minute) > estimate_by_time(Tier::Cheap, minute));
        assert!(estimate_by_time(Tier::Cheap, minute * 2) > estimate_by_time(Tier::Cheap, minute));
        assert!(estimate_by_time(Tier::Mid, minute).is_estimated());
    }

    #[test]
    fn a_cheap_agent_guess_reads_as_54_cents_an_hour() {
        assert_eq!(guess_per_hour(Tier::Cheap).to_string(), "$0.54");
    }

    #[test]
    fn a_guess_never_fires_the_task_ceiling() {
        let (budget, _kill) = Budget::new(usd("10.00"));
        budget.set_ceiling(&task(), usd("0.10"));
        assert_eq!(budget.record_guess(usd("0.50")), Vec::<BudgetEvent>::new());
        assert_eq!(budget.task_spent(&task()), Cost::ZERO);
        assert_eq!(budget.spent(), usd("0.50"));
    }

    #[test]
    fn a_guess_still_fires_the_cap_threshold_and_the_kill_signal() {
        let (budget, kill) = Budget::new(usd("1.00"));
        assert_eq!(
            budget.record_guess(usd("0.96")),
            vec![BudgetEvent::Threshold70, BudgetEvent::Threshold95]
        );
        assert!(*kill.borrow());
    }
}
