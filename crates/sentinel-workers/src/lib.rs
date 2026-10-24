//! Worker supervision: spawn agent CLIs, count their cost, and stop them on a signal.

pub mod adapter_cli;
pub mod budget;
pub mod context;
pub mod detect;
pub mod profile;
pub mod router;
pub mod supervisor;
pub mod worker;

pub use budget::{Budget, BudgetError, BudgetEvent};
pub use worker::{CommandLine, EndKind, OutputLine, RunControl, RunOutcome, RunSpec, Worker};
