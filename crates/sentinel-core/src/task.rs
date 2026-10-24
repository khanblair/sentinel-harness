//! Tasks, their statuses, and the tier that decides which worker takes them.

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::ids::TaskId;
use crate::scope::Scope;

/// Worker class. Docs and code say cheap, mid, smart. The UI says easy, normal, hard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Cheap,
    Mid,
    Smart,
}

impl Tier {
    /// The next tier up, or `None` at the top.
    pub fn escalate(self) -> Option<Tier> {
        match self {
            Tier::Cheap => Some(Tier::Mid),
            Tier::Mid => Some(Tier::Smart),
            Tier::Smart => None,
        }
    }

    pub fn ui_label(self) -> &'static str {
        match self {
            Tier::Cheap => "easy",
            Tier::Mid => "normal",
            Tier::Smart => "hard",
        }
    }
}

/// See the Task state machine in `docs/architecture.md`. Only the verifier may set `Verified`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Planned,
    Ready,
    Waiting,
    Reserved,
    Running,
    Checkpointed,
    Verifying,
    Verified,
    Failed,
    Blocked,
    Contradiction,
    Skipped,
    Partial,
}

impl TaskStatus {
    /// The exact label from `docs/ui-tokens.md`, Status Mapping.
    pub fn label(self) -> &'static str {
        match self {
            TaskStatus::Planned | TaskStatus::Ready => "Ready",
            TaskStatus::Waiting => "Waiting",
            TaskStatus::Reserved | TaskStatus::Running => "Running",
            TaskStatus::Checkpointed | TaskStatus::Verifying => "Verifying",
            TaskStatus::Verified => "Verified",
            TaskStatus::Failed | TaskStatus::Blocked => "Blocked",
            TaskStatus::Contradiction => "Contradiction",
            TaskStatus::Skipped => "Skipped",
            TaskStatus::Partial => "Partial",
        }
    }

    fn name(self) -> &'static str {
        match self {
            TaskStatus::Planned => "planned",
            TaskStatus::Ready => "ready",
            TaskStatus::Waiting => "waiting",
            TaskStatus::Reserved => "reserved",
            TaskStatus::Running => "running",
            TaskStatus::Checkpointed => "checkpointed",
            TaskStatus::Verifying => "verifying",
            TaskStatus::Verified => "verified",
            TaskStatus::Failed => "failed",
            TaskStatus::Blocked => "blocked",
            TaskStatus::Contradiction => "contradiction",
            TaskStatus::Skipped => "skipped",
            TaskStatus::Partial => "partial",
        }
    }

    /// No further work happens on a task in a terminal status.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            TaskStatus::Verified
                | TaskStatus::Blocked
                | TaskStatus::Contradiction
                | TaskStatus::Skipped
                | TaskStatus::Partial
        )
    }

    /// True when the state machine allows `self` to move to `next`.
    pub fn can_move_to(self, next: TaskStatus) -> bool {
        use TaskStatus as S;
        matches!(
            (self, next),
            (
                S::Planned,
                S::Ready | S::Contradiction | S::Skipped | S::Partial
            ) | (S::Ready, S::Reserved | S::Waiting | S::Skipped)
                | (S::Waiting, S::Reserved | S::Ready)
                | (S::Reserved, S::Running | S::Failed | S::Ready)
                | (S::Running, S::Checkpointed | S::Failed | S::Ready)
                | (S::Checkpointed, S::Verifying)
                | (S::Verifying, S::Verified | S::Failed | S::Partial)
                | (S::Failed, S::Ready | S::Blocked)
        )
    }

    pub fn move_to(self, next: TaskStatus) -> Result<TaskStatus, CoreError> {
        if self.can_move_to(next) {
            Ok(next)
        } else {
            Err(CoreError::InvalidTransition {
                from: self.name(),
                to: next.name(),
            })
        }
    }
}

/// Flags the planner sets.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFlags {
    /// The backlog says it is done but the code does not agree, or the reverse.
    pub already_done: bool,
    /// Tasks that cannot both be true with this one.
    pub contradicts: Vec<TaskId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub body: String,
    pub status: TaskStatus,
    pub tier: Tier,
    pub attempts: u32,
    pub scope: Scope,
    pub flags: TaskFlags,
}

impl Task {
    pub fn new(id: TaskId, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            body: body.into(),
            status: TaskStatus::Planned,
            tier: Tier::Cheap,
            attempts: 0,
            scope: Scope::new(),
            flags: TaskFlags::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    /// Plain order: run after.
    Order,
    /// The task must read the final form of a file the other task writes.
    ReadAfterWrite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    pub task: TaskId,
    pub depends_on: TaskId,
    pub kind: DependencyKind,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escalate_climbs_then_stops() {
        assert_eq!(Tier::Cheap.escalate(), Some(Tier::Mid));
        assert_eq!(Tier::Mid.escalate(), Some(Tier::Smart));
        assert_eq!(Tier::Smart.escalate(), None);
    }

    #[test]
    fn happy_path_is_allowed_in_order() {
        let path = [
            TaskStatus::Planned,
            TaskStatus::Ready,
            TaskStatus::Reserved,
            TaskStatus::Running,
            TaskStatus::Checkpointed,
            TaskStatus::Verifying,
            TaskStatus::Verified,
        ];
        for pair in path.windows(2) {
            assert!(
                pair[0].can_move_to(pair[1]),
                "{:?} to {:?}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn only_verifying_can_reach_verified() {
        for from in [
            TaskStatus::Running,
            TaskStatus::Checkpointed,
            TaskStatus::Failed,
        ] {
            assert!(!from.can_move_to(TaskStatus::Verified), "{from:?}");
        }
    }

    #[test]
    fn terminal_statuses_cannot_move() {
        for from in [
            TaskStatus::Verified,
            TaskStatus::Blocked,
            TaskStatus::Skipped,
        ] {
            assert!(from.is_terminal());
            assert!(from.move_to(TaskStatus::Ready).is_err());
        }
    }

    #[test]
    fn failed_task_can_retry_or_block() {
        assert!(TaskStatus::Failed.can_move_to(TaskStatus::Ready));
        assert!(TaskStatus::Failed.can_move_to(TaskStatus::Blocked));
    }

    #[test]
    fn labels_match_the_token_doc() {
        assert_eq!(TaskStatus::Verified.label(), "Verified");
        assert_eq!(TaskStatus::Running.label(), "Running");
        assert_eq!(TaskStatus::Contradiction.label(), "Contradiction");
    }
}
