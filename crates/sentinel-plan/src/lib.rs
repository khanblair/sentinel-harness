//! Planner crate: backlog parsing, task graph, planner contract, contradictions.

pub mod backlog;
pub mod contradiction;
pub mod graph;
mod plan_check;
pub mod planner;

pub use planner::{Plan, PlanRequest, PlannedTask};

use sentinel_core::TaskId;
use thiserror::Error;

/// Errors produced by the planner crate.
#[derive(Debug, Error)]
pub enum PlanError {
    #[error("parse error: {0}")]
    Parse(String),

    #[error("cycle detected: {0:?}")]
    Cycle(Vec<TaskId>),

    #[error("unknown task: {0}")]
    UnknownTask(String),

    #[error("invalid scope: {0}")]
    InvalidScope(String),

    #[error("backend error: {0}")]
    Backend(String),

    #[error("json error: {0}")]
    Json(String),
}

/// Result type for planner operations.
pub type Result<T> = std::result::Result<T, PlanError>;

impl From<serde_json::Error> for PlanError {
    fn from(e: serde_json::Error) -> Self {
        PlanError::Json(e.to_string())
    }
}
