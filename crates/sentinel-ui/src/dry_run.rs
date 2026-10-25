//! Pure logic for the dry run step: stage titles, how a stage looks, how stage events fold into
//! the rows the page shows, and the result sentences. No browser calls here, so the tests run on
//! the host.

use sentinel_core::dry_run::{DryRunStage, DryRunStageRow, DryRunStatus, StageState};

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

/// The shape and the color of a stage. A stage that never started is a muted ring. A running
/// stage has no SVG shape: it is the `spin` sentinel, which the view draws as a spinner.
pub fn stage_look(state: Option<StageState>) -> (&'static str, &'static str) {
    match state {
        None => ("ring", "muted"),
        Some(StageState::Running) => ("spin", "accent"),
        Some(StageState::Done) => ("dot", "ok"),
        Some(StageState::Failed) => ("oct", "danger"),
    }
}

/// The CSS class of a stage row. The prototype defines only `on` and `done`, so a failed stage
/// uses `done`, and its red shape carries the failure.
pub fn stage_class(state: Option<StageState>) -> &'static str {
    match state {
        None => "todo",
        Some(StageState::Running) => "on",
        Some(StageState::Done | StageState::Failed) => "done",
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
    fn a_stage_that_never_started_is_a_muted_ring() {
        assert_eq!(stage_look(None), ("ring", "muted"));
        assert_eq!(stage_class(None), "todo");
    }

    #[test]
    fn a_running_stage_gets_the_spinner_sentinel() {
        assert_eq!(stage_look(Some(StageState::Running)).0, "spin");
        assert_eq!(stage_class(Some(StageState::Running)), "on");
    }

    #[test]
    fn a_done_stage_is_a_green_dot_and_a_failed_one_a_red_octagon() {
        assert_eq!(stage_look(Some(StageState::Done)), ("dot", "ok"));
        assert_eq!(stage_look(Some(StageState::Failed)), ("oct", "danger"));
        assert_eq!(stage_class(Some(StageState::Done)), "done");
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
    fn the_result_sentence_counts_verified_tasks_of_the_total() {
        assert_eq!(result_text(3, 3), "3 of 3 sample tasks verified.");
        assert_eq!(result_text(1, 3), "1 of 3 sample tasks verified.");
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
