//! A quality note: a smell or slop warning on agent code. Never changes a verdict.

use serde::{Deserialize, Serialize};

use crate::ids::TaskId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityKind {
    Size,
    Duplication,
    Slop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityNote {
    pub task: TaskId,
    pub kind: QualityKind,
    /// `path:line` in the agent's diff.
    pub location: String,
    /// A short stable code, such as `long_function`. Never task text.
    pub detail_code: String,
}
