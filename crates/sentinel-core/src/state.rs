//! Small state enums the UI and the daemon share.

use serde::{Deserialize, Serialize};

use crate::ids::{DecisionId, TaskId};

/// See the Harness state machine in `docs/architecture.md`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessState {
    #[default]
    Idle,
    Onboarding,
    Ready,
    Running,
    Paused,
    Stopping,
    Done,
    Crashed,
    Resuming,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TailscaleState {
    NotInstalled,
    SignedOut,
    Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Connected,
    NeedsLogin,
    Missing,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionState {
    Pending,
    Answered,
    /// No answer before the timeout: both tasks are skipped.
    Defaulted,
}

/// A question only the user can answer, such as two contradicting tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub id: DecisionId,
    pub prompt: String,
    pub tasks: Vec<TaskId>,
    pub state: DecisionState,
    /// The chosen task id, or `skip_both`.
    pub answer: Option<String>,
}
