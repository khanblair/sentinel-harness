//! The dry run: a short rehearsal on a sample project that ships with Sentinel
//! (`docs/product-scope.md`, F8). These are the stage names and the shapes the daemon sends and
//! the UI reads.

use serde::{Deserialize, Serialize};

/// The six steps of a dry run, in the order they happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DryRunStage {
    Plan,
    LockFiles,
    AgentsWork,
    RunTests,
    Review,
    Report,
}

impl DryRunStage {
    /// Every stage, in order.
    pub const ALL: [DryRunStage; 6] = [
        DryRunStage::Plan,
        DryRunStage::LockFiles,
        DryRunStage::AgentsWork,
        DryRunStage::RunTests,
        DryRunStage::Review,
        DryRunStage::Report,
    ];
}

/// Where a stage stands. A stage that never started has no state at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageState {
    Running,
    Done,
    Failed,
}

/// One stage that has a state, as `GET /api/dry-run` returns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DryRunStageRow {
    pub stage: DryRunStage,
    pub state: StageState,
}

/// The last dry run: whether one is going, its stages in order, and the counts once it ends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DryRunStatus {
    pub running: bool,
    pub stages: Vec<DryRunStageRow>,
    pub verified: Option<u32>,
    pub total: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stage_serializes_as_its_snake_case_name() {
        let json = serde_json::to_string(&DryRunStage::LockFiles).unwrap();
        assert_eq!(json, "\"lock_files\"");
        let state = serde_json::to_string(&StageState::Failed).unwrap();
        assert_eq!(state, "\"failed\"");
    }

    #[test]
    fn all_lists_the_six_stages_in_order() {
        assert_eq!(DryRunStage::ALL.len(), 6);
        assert_eq!(DryRunStage::ALL[0], DryRunStage::Plan);
        assert_eq!(DryRunStage::ALL[5], DryRunStage::Report);
    }
}
