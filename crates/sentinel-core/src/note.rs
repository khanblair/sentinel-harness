//! Brain vocabulary shared with the UI. The vault logic lives in `sentinel-brain`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteType {
    Convention,
    Decision,
    Fix,
    File,
    Task,
    Run,
    Lesson,
    Glossary,
    HowTo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LessonState {
    Candidate,
    Proven,
    Rejected,
    Retired,
}

/// Ordered by how much the Brain may write: `Off < ReadOnly < Learn`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BrainMode {
    Off,
    ReadOnly,
    Learn,
}

impl BrainMode {
    /// A phone may only move the mode toward `Off`. Moving up needs the PC.
    pub fn phone_may_set(self, target: BrainMode) -> bool {
        target <= self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalKind {
    Add,
    Update,
    Retire,
    Merge,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_can_tighten_but_not_loosen() {
        assert!(BrainMode::Learn.phone_may_set(BrainMode::ReadOnly));
        assert!(BrainMode::ReadOnly.phone_may_set(BrainMode::Off));
        assert!(BrainMode::ReadOnly.phone_may_set(BrainMode::ReadOnly));
        assert!(!BrainMode::ReadOnly.phone_may_set(BrainMode::Learn));
        assert!(!BrainMode::Off.phone_may_set(BrainMode::ReadOnly));
    }

    #[test]
    fn brain_mode_serializes_as_kebab_case() {
        assert_eq!(
            serde_json::to_string(&BrainMode::ReadOnly).unwrap(),
            "\"read-only\""
        );
    }
}
