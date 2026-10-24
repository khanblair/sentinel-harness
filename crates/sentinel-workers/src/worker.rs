//! The contract between the supervisor, the adapters, and the scheduler.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use sentinel_core::{Cost, TaskId, ToolId};
use tokio::sync::{mpsc, watch};

use crate::budget::Budget;

/// A program to start. An adapter builds it from a profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLine {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    /// Text written to the program's stdin, then closed. `None` leaves stdin empty.
    pub stdin: Option<String>,
    pub workdir: PathBuf,
}

/// What to run for one task attempt.
#[derive(Debug, Clone)]
pub struct RunSpec {
    pub task: TaskId,
    pub agent: ToolId,
    pub model: String,
    pub prompt: String,
    pub workdir: PathBuf,
    pub timeout: Duration,
    /// The paths the task may write: files and folders of its scope. A worker that writes files
    /// itself refuses a write outside them. A CLI agent ignores this, and the write guard
    /// catches it instead.
    pub scope: Vec<String>,
}

/// One line of agent output, tagged so the stream can show who said it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputLine {
    pub task: TaskId,
    pub agent: ToolId,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndKind {
    Completed,
    Failed,
    TimedOut,
    Killed,
    /// The per-task cost ceiling was hit.
    CostCeiling,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    pub end: EndKind,
    pub exit_code: Option<i32>,
    /// The last lines of output, at most 200.
    pub output_tail: Vec<String>,
    pub cost: Cost,
}

/// Handles a worker needs while it runs.
pub struct RunControl {
    /// Becomes `true` when every worker must stop (the 95 percent budget kill, or a user kill).
    pub kill: watch::Receiver<bool>,
    pub lines: mpsc::UnboundedSender<OutputLine>,
    pub budget: Arc<Budget>,
}

/// Anything that can do one task attempt: a CLI adapter, or the `OpenRouter` loop.
pub trait Worker: Send + Sync {
    fn run(&self, spec: RunSpec, ctrl: RunControl) -> BoxFuture<'_, RunOutcome>;
}
