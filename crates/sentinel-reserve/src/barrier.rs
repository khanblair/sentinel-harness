//! The barrier between waves: wait for the wave to drain, refuse a dirty tree,
//! then run the full suite on a clean one.

/// What the barrier does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierDecision {
    WaitForTasks,
    RefuseDirty,
    RunSuite,
}

/// The suite result once it has run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierOutcome {
    Passed,
    Failed,
}

pub struct Barrier {
    wave: u32,
    tasks_left: usize,
    last_decision: Option<BarrierDecision>,
}

impl Barrier {
    pub fn new(wave: u32, tasks_in_wave: usize) -> Barrier {
        Barrier {
            wave,
            tasks_left: tasks_in_wave,
            last_decision: None,
        }
    }

    pub fn wave(&self) -> u32 {
        self.wave
    }

    /// One task left the wave: checkpointed, failed, or killed.
    pub fn task_finished(&mut self) {
        self.tasks_left = self.tasks_left.saturating_sub(1);
    }

    pub fn tasks_left(&self) -> usize {
        self.tasks_left
    }

    /// The suite runs only when the wave is empty and the tree is clean.
    pub fn decide(&mut self, tree_clean: bool) -> BarrierDecision {
        let decision = if self.tasks_left > 0 {
            BarrierDecision::WaitForTasks
        } else if tree_clean {
            BarrierDecision::RunSuite
        } else {
            BarrierDecision::RefuseDirty
        };
        self.last_decision = Some(decision);
        decision
    }

    /// A suite result only counts when `decide` asked for the suite.
    pub fn suite_finished(&mut self, passed: bool) -> BarrierOutcome {
        if self.last_decision != Some(BarrierDecision::RunSuite) || !passed {
            return BarrierOutcome::Failed;
        }
        BarrierOutcome::Passed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_while_any_task_is_still_running() {
        let mut barrier = Barrier::new(2, 2);
        barrier.task_finished();
        assert_eq!(barrier.tasks_left(), 1);
        assert_eq!(barrier.decide(true), BarrierDecision::WaitForTasks);
    }

    #[test]
    fn refuses_to_run_on_a_dirty_tree() {
        let mut barrier = Barrier::new(1, 0);
        assert_eq!(barrier.decide(false), BarrierDecision::RefuseDirty);
    }

    #[test]
    fn runs_the_suite_once_the_wave_drains_on_a_clean_tree() {
        let mut barrier = Barrier::new(3, 1);
        assert_eq!(barrier.decide(true), BarrierDecision::WaitForTasks);
        barrier.task_finished();
        assert_eq!(barrier.tasks_left(), 0);
        assert_eq!(barrier.decide(true), BarrierDecision::RunSuite);
    }

    #[test]
    fn task_finished_never_goes_below_zero() {
        let mut barrier = Barrier::new(1, 0);
        barrier.task_finished();
        barrier.task_finished();
        assert_eq!(barrier.tasks_left(), 0);
    }

    #[test]
    fn a_suite_result_before_the_decision_is_failed() {
        let mut barrier = Barrier::new(1, 1);
        assert_eq!(barrier.suite_finished(true), BarrierOutcome::Failed);
    }

    #[test]
    fn a_suite_after_a_refusal_is_failed() {
        let mut barrier = Barrier::new(1, 0);
        assert_eq!(barrier.decide(false), BarrierDecision::RefuseDirty);
        assert_eq!(barrier.suite_finished(true), BarrierOutcome::Failed);
    }

    #[test]
    fn the_suite_reports_its_own_result() {
        let mut barrier = Barrier::new(1, 0);
        assert_eq!(barrier.decide(true), BarrierDecision::RunSuite);
        assert_eq!(barrier.suite_finished(true), BarrierOutcome::Passed);
        let mut other = Barrier::new(1, 0);
        assert_eq!(other.decide(true), BarrierDecision::RunSuite);
        assert_eq!(other.suite_finished(false), BarrierOutcome::Failed);
    }

    #[test]
    fn the_wave_number_is_kept() {
        assert_eq!(Barrier::new(7, 3).wave(), 7);
    }
}
