//! Worker supervision: spawn agent CLIs, count their cost, and stop them on a signal.

pub mod adapter_cli;
pub mod agent_tools;
pub mod budget;
pub mod context;
pub mod detect;
pub mod openrouter;
pub mod profile;
pub mod router;
pub mod supervisor;
pub mod transcript;
pub mod worker;

mod capture;

pub use budget::{Budget, BudgetError, BudgetEvent};
pub use detect::{DetectOptions, detect_all};
pub use openrouter::{OpenRouterConfig, OpenRouterError, OpenRouterWorker};
pub use profile::{
    GENERIC_ID, LoginCheck, LoginKind, Profile, ProfileError, PromptVia, builtin_profiles,
};
pub use worker::{CommandLine, EndKind, OutputLine, RunControl, RunOutcome, RunSpec, Worker};
