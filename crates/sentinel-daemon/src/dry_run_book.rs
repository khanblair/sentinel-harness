//! What the dry run shows: each stage's state, the counts at the end, and the notes the wizard
//! lists beside the stages. It reads the second engine's events and writes no file.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use sentinel_core::dry_run::{DryRunStage, DryRunStageRow, DryRunStatus, StageState};
use sentinel_core::{Event, TaskId, TaskStatus};

use crate::agents::FALLBACK_PLAN_NOTE;
use crate::dry_run_preflight::{NO_AGENT_PASSED, Preflight};

/// The notes the wizard lists. The newest stay.
const MAX_NOTES: usize = 5;
/// A note is cut at this many characters, so one long message cannot fill the panel.
const NOTE_CHARS: usize = 160;
/// Once this wave has ended, a run where every task is blocked or failed has no work left.
const HOPELESS_AFTER_WAVE: u32 = 2;
const HOPELESS_NOTE: &str = "Every task is blocked or failed, so the dry run stops here.";
/// The last sentence of the verifier's message for a task that goes back for another try.
const RETRY_END: &str = "It goes back for another try.";

/// A stage that changed state. The engine emits one event for each.
pub(crate) type Change = (DryRunStage, StageState);

/// The one dry run the daemon keeps. A daemon has one dry run, so the state is not per engine.
static BOOK: OnceLock<Mutex<Book>> = OnceLock::new();

pub(crate) fn book() -> &'static Mutex<Book> {
    BOOK.get_or_init(|| Mutex::new(Book::default()))
}

/// The dry run as the wizard sees it. Its methods are the only way the stages change.
#[derive(Debug, Default)]
pub(crate) struct Book {
    running: bool,
    stages: Vec<(DryRunStage, StageState)>,
    verified: Option<u32>,
    total: Option<u32>,
    notes: Vec<String>,
    /// How many tasks the plan has, and the last status each task reached.
    planned: usize,
    task_states: HashMap<TaskId, TaskStatus>,
    waves_ended: u32,
}

impl Book {
    /// Start a dry run. Refused while one is going.
    pub(crate) fn claim(&mut self) -> bool {
        if self.running {
            return false;
        }
        *self = Book {
            running: true,
            ..Book::default()
        };
        true
    }

    fn state_of(&self, stage: DryRunStage) -> Option<StageState> {
        self.stages
            .iter()
            .find(|(s, _)| *s == stage)
            .map(|(_, state)| *state)
    }

    /// Set one stage. The change is returned only when the state differs.
    fn set(&mut self, step: DryRunStage, value: StageState) -> Option<Change> {
        match self.stages.iter_mut().find(|(s, _)| *s == step) {
            Some(row) if row.1 == value => return None,
            Some(row) => row.1 = value,
            None => self.stages.push((step, value)),
        }
        Some((step, value))
    }

    /// Mark every stage before `last` done, and `last` itself with `outcome`.
    fn reach(&mut self, last: DryRunStage, outcome: StageState) -> Vec<Change> {
        let before = DryRunStage::ALL.into_iter().take_while(|s| *s != last);
        before
            .chain(std::iter::once(last))
            .filter_map(|s| {
                let target = if s == last { outcome } else { StageState::Done };
                self.set(s, target)
            })
            .collect()
    }

    /// Start a stage, unless it already has a state. A stage never goes back to running.
    pub(crate) fn begin(&mut self, stage: DryRunStage) -> Vec<Change> {
        if self.state_of(stage).is_some() {
            return Vec::new();
        }
        self.reach(stage, StageState::Running)
    }

    /// Apply one event of the second engine to the stages.
    pub(crate) fn handle(&mut self, event: &Event) -> Vec<Change> {
        self.track(event);
        if let Event::RunFinished { verified, total } = event {
            self.verified = Some(*verified);
            self.total = Some(*total);
        }
        // Starting or ending a stage also settles the stages before it, so a missed event
        // never leaves an earlier stage open.
        if matches!(
            event,
            Event::TaskStatus {
                status: TaskStatus::Running,
                ..
            }
        ) {
            return self.begin(DryRunStage::AgentsWork);
        }
        match event.kind().as_str() {
            "PlanReady" => self.reach(DryRunStage::LockFiles, StageState::Done),
            "BarrierStart" => self.begin(DryRunStage::RunTests),
            "BarrierEnd" => self.reach(DryRunStage::RunTests, StageState::Done),
            "Verdict" => self.reach(DryRunStage::Review, StageState::Done),
            "ReportReady" => self.reach(DryRunStage::Report, StageState::Done),
            _ => Vec::new(),
        }
    }

    /// Keep the counts that the hopeless check reads, and the notes that come from messages.
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "the dry run follows a few of the many event kinds"
    )]
    fn track(&mut self, event: &Event) {
        match event {
            Event::PlanReady { tasks, .. } => {
                self.planned = usize::try_from(*tasks).unwrap_or(usize::MAX);
            }
            Event::TaskStatus { task, status } | Event::Verdict { task, status, .. } => {
                self.task_states.insert(task.clone(), *status);
            }
            Event::BarrierEnd { .. } => self.waves_ended += 1,
            Event::Message { from, text, .. } if from.as_str() == "sentinel" => {
                self.note_message(text);
            }
            _ => {}
        }
    }

    /// The fallback plan and each retry reach the wizard as notes. Other messages do not.
    fn note_message(&mut self, text: &str) {
        if text == FALLBACK_PLAN_NOTE {
            self.push_note(text);
        } else if let Some(note) = retry_note(text) {
            self.push_note(&note);
        }
    }

    fn push_note(&mut self, text: &str) {
        self.notes.push(text.chars().take(NOTE_CHARS).collect());
        let surplus = self.notes.len().saturating_sub(MAX_NOTES);
        self.notes.drain(..surplus);
    }

    /// Record the agent check. When no agent passed, the run ends here with `agents_work` failed.
    pub(crate) fn finish_preflight(&mut self, preflight: &Preflight) -> Vec<Change> {
        for note in &preflight.notes {
            self.push_note(note);
        }
        if !preflight.usable.is_empty() {
            return Vec::new();
        }
        self.push_note(NO_AGENT_PASSED);
        self.set(DryRunStage::AgentsWork, StageState::Failed)
            .into_iter()
            .collect()
    }

    /// True once the second wave has ended and every planned task is blocked or failed.
    pub(crate) fn hopeless(&self) -> bool {
        let all_lost = self
            .task_states
            .values()
            .all(|status| matches!(status, TaskStatus::Blocked | TaskStatus::Failed));
        self.waves_ended >= HOPELESS_AFTER_WAVE
            && self.planned > 0
            && self.task_states.len() == self.planned
            && all_lost
    }

    /// The run stops early: say why, and fail the stage that is still open.
    pub(crate) fn give_up(&mut self) -> Vec<Change> {
        self.push_note(HOPELESS_NOTE);
        self.close_open()
    }

    /// A run that ended with a stage still open failed that stage.
    pub(crate) fn close_open(&mut self) -> Vec<Change> {
        let open: Vec<DryRunStage> = self
            .stages
            .iter()
            .filter(|(_, state)| *state == StageState::Running)
            .map(|(stage, _)| *stage)
            .collect();
        open.into_iter()
            .filter_map(|stage| self.set(stage, StageState::Failed))
            .collect()
    }

    /// The dry run is over. Returns the verified and total counts.
    pub(crate) fn end(&mut self) -> (u32, u32) {
        self.running = false;
        (self.verified.unwrap_or(0), self.total.unwrap_or(0))
    }

    pub(crate) fn status(&self) -> DryRunStatus {
        let stages = DryRunStage::ALL
            .into_iter()
            .filter_map(|stage| {
                self.state_of(stage)
                    .map(|state| DryRunStageRow { stage, state })
            })
            .collect();
        DryRunStatus {
            running: self.running,
            stages,
            verified: self.verified,
            total: self.total,
            notes: self.notes.clone(),
        }
    }
}

/// The note for a task that went back for another try. The verifier sends this as a message, not
/// as an event, so the note is read from the text. A change to that text breaks the note; a test
/// covers the current wording.
fn retry_note(text: &str) -> Option<String> {
    let (task, rest) = text.split_once(" did not pass: ")?;
    let (reason, _) = rest.split_once(" Its change was undone.")?;
    text.ends_with(RETRY_END)
        .then(|| format!("{task} went back for another try: {reason}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Book {
        Book::default()
    }

    fn message(text: &str) -> Event {
        Event::Message {
            from: "sentinel".to_owned(),
            to: None,
            text: text.to_owned(),
        }
    }

    fn task_event(id: &str, status: TaskStatus) -> Event {
        Event::TaskStatus {
            task: TaskId::new(id),
            status,
        }
    }

    #[test]
    fn a_second_claim_is_refused_until_the_dry_run_ends() {
        let mut book = fresh();
        assert!(book.claim());
        assert!(!book.claim());
        book.end();
        assert!(book.claim());
    }

    #[test]
    fn plan_ready_finishes_plan_and_lock_files() {
        let mut book = fresh();
        book.claim();
        book.begin(DryRunStage::Plan);
        let changes = book.handle(&Event::PlanReady {
            tasks: 3,
            contradictions: 0,
            already_done: 0,
        });
        assert_eq!(
            changes,
            vec![
                (DryRunStage::Plan, StageState::Done),
                (DryRunStage::LockFiles, StageState::Done),
            ]
        );
    }

    #[test]
    fn the_first_running_task_starts_agents_work_once() {
        let mut book = fresh();
        book.claim();
        let running = task_event("T-01", TaskStatus::Running);
        assert_eq!(
            book.handle(&running),
            vec![
                (DryRunStage::Plan, StageState::Done),
                (DryRunStage::LockFiles, StageState::Done),
                (DryRunStage::AgentsWork, StageState::Running),
            ]
        );
        assert_eq!(book.handle(&running), Vec::new());
    }

    #[test]
    fn a_stage_that_is_done_does_not_restart() {
        let mut book = fresh();
        book.claim();
        book.reach(DryRunStage::Review, StageState::Done);
        assert_eq!(book.begin(DryRunStage::AgentsWork), Vec::new());
    }

    #[test]
    fn a_run_that_ends_early_fails_its_open_stage() {
        let mut book = fresh();
        book.claim();
        book.begin(DryRunStage::Plan);
        let changes = book.close_open();
        assert_eq!(changes, vec![(DryRunStage::Plan, StageState::Failed)]);
    }

    #[test]
    fn a_retry_message_becomes_a_note() {
        let mut book = fresh();
        book.claim();
        book.handle(&message(
            "T-04 did not pass: tests failed. Its change was undone. It goes back for another try.",
        ));
        assert_eq!(
            book.status().notes,
            ["T-04 went back for another try: tests failed."]
        );
    }

    #[test]
    fn other_messages_are_not_notes() {
        let mut book = fresh();
        book.claim();
        book.handle(&message("Plan approved. The run goes on in Auto."));
        assert_eq!(book.status().notes, Vec::<String>::new());
    }

    #[test]
    fn the_fallback_plan_message_becomes_a_note() {
        let mut book = fresh();
        book.claim();
        book.handle(&message(FALLBACK_PLAN_NOTE));
        assert_eq!(book.status().notes, [FALLBACK_PLAN_NOTE]);
    }

    #[test]
    fn only_the_newest_five_notes_stay() {
        let mut book = fresh();
        for n in 1..=7 {
            book.push_note(&format!("note {n}"));
        }
        assert_eq!(
            book.status().notes,
            ["note 3", "note 4", "note 5", "note 6", "note 7"]
        );
    }

    #[test]
    fn a_long_note_is_cut_to_160_characters() {
        let mut book = fresh();
        book.push_note(&"n".repeat(300));
        assert_eq!(book.status().notes[0].chars().count(), 160);
    }

    #[test]
    fn when_no_agent_passed_the_run_fails_agents_work() {
        let mut book = fresh();
        book.claim();
        let preflight = Preflight {
            usable: Vec::new(),
            notes: vec!["Skipped a: no reply.".to_owned()],
        };
        let changes = book.finish_preflight(&preflight);
        assert_eq!(changes, vec![(DryRunStage::AgentsWork, StageState::Failed)]);
        assert_eq!(
            book.status().notes,
            ["Skipped a: no reply.", NO_AGENT_PASSED]
        );
    }

    #[test]
    fn a_usable_agent_leaves_the_stages_alone() {
        let mut book = fresh();
        book.claim();
        let preflight = Preflight {
            usable: vec![sentinel_core::ToolId::new("claude")],
            notes: Vec::new(),
        };
        assert_eq!(book.finish_preflight(&preflight), Vec::new());
    }

    #[test]
    fn a_run_with_every_task_lost_after_the_second_wave_is_hopeless() {
        let mut book = fresh();
        book.claim();
        book.handle(&Event::PlanReady {
            tasks: 2,
            contradictions: 0,
            already_done: 0,
        });
        book.handle(&Event::BarrierEnd {
            wave: 1,
            passed: false,
        });
        book.handle(&Event::BarrierEnd {
            wave: 2,
            passed: false,
        });
        book.handle(&task_event("T-01", TaskStatus::Blocked));
        assert!(!book.hopeless());
        book.handle(&task_event("T-02", TaskStatus::Failed));
        assert!(book.hopeless());
    }

    #[test]
    fn a_lost_run_is_not_hopeless_before_the_second_wave_ends() {
        let mut book = fresh();
        book.claim();
        book.handle(&Event::PlanReady {
            tasks: 1,
            contradictions: 0,
            already_done: 0,
        });
        book.handle(&Event::BarrierEnd {
            wave: 1,
            passed: false,
        });
        book.handle(&task_event("T-01", TaskStatus::Blocked));
        assert!(!book.hopeless());
    }

    #[test]
    fn one_verified_task_keeps_the_run_going() {
        let mut book = fresh();
        book.claim();
        book.handle(&Event::PlanReady {
            tasks: 2,
            contradictions: 0,
            already_done: 0,
        });
        for wave in 1..=2 {
            book.handle(&Event::BarrierEnd {
                wave,
                passed: false,
            });
        }
        book.handle(&task_event("T-01", TaskStatus::Blocked));
        book.handle(&Event::Verdict {
            task: TaskId::new("T-02"),
            status: TaskStatus::Verified,
            reason: "Tests pass.".to_owned(),
        });
        assert!(!book.hopeless());
    }
}
