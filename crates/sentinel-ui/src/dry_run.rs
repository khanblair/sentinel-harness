//! Pure logic for the dry run step: stage titles, how a stage looks, how the API's stage rows
//! become the rows the page shows, the notes the page lists, and the result sentences. No browser
//! calls here, so the tests run on the host.

use sentinel_core::dry_run::{DryRunStage, DryRunStageRow, DryRunStatus, StageState};

/// How many notes the page lists. The newest ones stay on screen.
const MAX_NOTES_SHOWN: usize = 5;

/// How one stage row looks on screen. The API has no waiting state: a stage it does not mention
/// is one the page shows as waiting or running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageLook {
    Waiting,
    Running,
    Done,
    Failed,
}

/// The stage title, the same words as the wizard's stage list.
pub fn stage_title(stage: DryRunStage) -> &'static str {
    match stage {
        DryRunStage::Plan => "Plan",
        DryRunStage::LockFiles => "Lock files",
        DryRunStage::AgentsWork => "Agents work",
        DryRunStage::RunTests => "Run tests",
        DryRunStage::Review => "Review",
        DryRunStage::Report => "Report",
    }
}

/// The shape and the color of a stage row. A waiting stage is a muted ring. A running stage has
/// no SVG shape: it is the `spin` sentinel, which the view draws as a spinner.
pub fn stage_look(look: StageLook) -> (&'static str, &'static str) {
    match look {
        StageLook::Waiting => ("ring", "muted"),
        StageLook::Running => ("spin", "accent"),
        StageLook::Done => ("dot", "ok"),
        StageLook::Failed => ("oct", "danger"),
    }
}

/// The CSS class of a stage row. The prototype defines only `on` and `done`, so a failed stage
/// uses `done`, and its red shape carries the failure.
pub fn stage_class(look: StageLook) -> &'static str {
    match look {
        StageLook::Waiting => "todo",
        StageLook::Running => "on",
        StageLook::Done | StageLook::Failed => "done",
    }
}

/// The state a stage has in these rows, if it has one.
pub fn state_of(rows: &[DryRunStageRow], stage: DryRunStage) -> Option<StageState> {
    rows.iter()
        .find(|row| row.stage == stage)
        .map(|row| row.state)
}

/// The first stage that failed, in run order.
pub fn failed_stage(rows: &[DryRunStageRow]) -> Option<DryRunStage> {
    DryRunStage::ALL
        .into_iter()
        .find(|stage| state_of(rows, *stage) == Some(StageState::Failed))
}

/// The look of each stage, in run order. A done stage is done and a failed stage is failed. While
/// the run goes on, the first stage that is neither gets the spinner and the stages after it
/// wait. A run that ended never shows a spinner, because nothing is running any more.
pub fn row_states(stages: &[DryRunStageRow], running: bool) -> Vec<StageLook> {
    let mut spinner_given = !running;
    DryRunStage::ALL
        .into_iter()
        .map(|stage| match state_of(stages, stage) {
            Some(StageState::Done) => StageLook::Done,
            Some(StageState::Failed) => StageLook::Failed,
            _ if !spinner_given => {
                spinner_given = true;
                StageLook::Running
            }
            _ => StageLook::Waiting,
        })
        .collect()
}

/// The newest notes the page lists, newest last, at most `MAX_NOTES_SHOWN`.
pub fn recent_notes(notes: &[String]) -> &[String] {
    let start = notes.len().saturating_sub(MAX_NOTES_SHOWN);
    &notes[start..]
}

/// The note under a failed stage: the last note the daemon sent, if there is one.
pub fn last_note(notes: &[String]) -> Option<&str> {
    notes.last().map(String::as_str)
}

/// The result sentence after a run that finished.
pub fn result_text(verified: u32, total: u32) -> String {
    format!("{verified} of {total} sample tasks verified.")
}

/// The sentence that says where a failed run stopped.
pub fn failure_text(stage: DryRunStage) -> String {
    format!(
        "The dry run stopped at {}. Your agents or the sample tests did not finish.",
        stage_title(stage)
    )
}

/// The last words of a run that ended: the count when the daemon counted the tasks, else the
/// failed stage. A run that still goes on has no outcome yet.
pub fn outcome_text(status: &DryRunStatus) -> Option<String> {
    if status.running {
        return None;
    }
    match (status.verified, status.total) {
        (Some(verified), Some(total)) => Some(result_text(verified, total)),
        _ => failed_stage(&status.stages).map(failure_text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(stage: DryRunStage, progress: StageState) -> DryRunStageRow {
        DryRunStageRow {
            stage,
            state: progress,
        }
    }

    fn status(
        running: bool,
        counts: Option<(u32, u32)>,
        stages: Vec<DryRunStageRow>,
    ) -> DryRunStatus {
        DryRunStatus {
            running,
            stages,
            verified: counts.map(|(verified, _)| verified),
            total: counts.map(|(_, total)| total),
            notes: Vec::new(),
        }
    }

    fn notes(count: usize) -> Vec<String> {
        (1..=count).map(|number| format!("note {number}")).collect()
    }

    #[test]
    fn each_stage_has_its_own_title_in_run_order() {
        let titles: Vec<&str> = DryRunStage::ALL.into_iter().map(stage_title).collect();
        assert_eq!(
            titles,
            [
                "Plan",
                "Lock files",
                "Agents work",
                "Run tests",
                "Review",
                "Report"
            ]
        );
    }

    #[test]
    fn a_waiting_stage_is_a_muted_ring() {
        assert_eq!(stage_look(StageLook::Waiting), ("ring", "muted"));
        assert_eq!(stage_class(StageLook::Waiting), "todo");
    }

    #[test]
    fn a_running_stage_gets_the_spinner_sentinel() {
        assert_eq!(stage_look(StageLook::Running).0, "spin");
        assert_eq!(stage_class(StageLook::Running), "on");
    }

    #[test]
    fn a_done_stage_is_a_green_dot_and_a_failed_one_a_red_octagon() {
        assert_eq!(stage_look(StageLook::Done), ("dot", "ok"));
        assert_eq!(stage_look(StageLook::Failed), ("oct", "danger"));
        assert_eq!(stage_class(StageLook::Done), "done");
    }

    #[test]
    fn the_failed_stage_is_the_first_failure_in_run_order() {
        let rows = [
            row(DryRunStage::Plan, StageState::Done),
            row(DryRunStage::RunTests, StageState::Failed),
            row(DryRunStage::Review, StageState::Failed),
        ];
        assert_eq!(failed_stage(&rows), Some(DryRunStage::RunTests));
        assert_eq!(failed_stage(&rows[..1]), None);
    }

    #[test]
    fn a_run_where_every_stage_is_done_shows_all_done() {
        let rows: Vec<DryRunStageRow> = DryRunStage::ALL
            .into_iter()
            .map(|stage| row(stage, StageState::Done))
            .collect();
        assert_eq!(row_states(&rows, false), vec![StageLook::Done; 6]);
    }

    #[test]
    fn a_third_stage_without_a_row_is_the_running_one_after_two_done() {
        let rows = [
            row(DryRunStage::Plan, StageState::Done),
            row(DryRunStage::LockFiles, StageState::Done),
        ];
        assert_eq!(
            row_states(&rows, true),
            [
                StageLook::Done,
                StageLook::Done,
                StageLook::Running,
                StageLook::Waiting,
                StageLook::Waiting,
                StageLook::Waiting,
            ]
        );
    }

    #[test]
    fn a_failed_stage_shows_failed_and_the_stages_after_it_wait() {
        let rows = [
            row(DryRunStage::Plan, StageState::Done),
            row(DryRunStage::RunTests, StageState::Failed),
        ];
        assert_eq!(
            row_states(&rows, false),
            [
                StageLook::Done,
                StageLook::Waiting,
                StageLook::Waiting,
                StageLook::Failed,
                StageLook::Waiting,
                StageLook::Waiting,
            ]
        );
    }

    #[test]
    fn a_run_that_has_not_started_shows_every_stage_waiting() {
        assert_eq!(row_states(&[], false), vec![StageLook::Waiting; 6]);
    }

    #[test]
    fn a_run_that_just_started_shows_the_first_stage_running() {
        let looks = row_states(&[], true);
        assert_eq!(looks[0], StageLook::Running);
        assert_eq!(looks[1..], [StageLook::Waiting; 5]);
    }

    #[test]
    fn a_run_that_ended_unfinished_shows_no_spinner() {
        let rows = [row(DryRunStage::Plan, StageState::Done)];
        assert!(!row_states(&rows, false).contains(&StageLook::Running));
    }

    #[test]
    fn the_result_sentence_counts_verified_tasks_of_the_total() {
        assert_eq!(result_text(3, 3), "3 of 3 sample tasks verified.");
        assert_eq!(result_text(1, 3), "1 of 3 sample tasks verified.");
    }

    #[test]
    fn the_notes_list_keeps_the_newest_five_in_order() {
        let all = notes(7);
        assert_eq!(recent_notes(&all), &all[2..]);
        assert_eq!(recent_notes(&all[..3]), &all[..3]);
    }

    #[test]
    fn the_note_under_a_failed_stage_is_the_last_one() {
        assert_eq!(last_note(&notes(3)), Some("note 3"));
        assert_eq!(last_note(&[]), None);
    }

    #[test]
    fn a_run_that_is_still_going_has_no_outcome() {
        let going = status(true, None, vec![row(DryRunStage::Plan, StageState::Done)]);
        assert_eq!(outcome_text(&going), None);
    }

    #[test]
    fn a_counted_run_says_how_many_tasks_verified() {
        let counted = status(false, Some((2, 3)), vec![]);
        assert_eq!(
            outcome_text(&counted).as_deref(),
            Some("2 of 3 sample tasks verified.")
        );
    }

    #[test]
    fn a_run_that_failed_before_counting_names_the_failed_stage() {
        let failed = status(
            false,
            None,
            vec![row(DryRunStage::Review, StageState::Failed)],
        );
        assert_eq!(
            outcome_text(&failed).as_deref(),
            Some(failure_text(DryRunStage::Review).as_str())
        );
    }

    #[test]
    fn the_failure_sentence_names_the_stage() {
        assert_eq!(
            failure_text(DryRunStage::LockFiles),
            "The dry run stopped at Lock files. Your agents or the sample tests did not finish."
        );
    }
}
