//! Shared types for the daemon and the UI. One definition, no schema drift.

pub mod cost;
pub mod error;
pub mod event;
pub mod ids;
pub mod note;
pub mod permission;
pub mod quality;
pub mod reservation;
pub mod scope;
pub mod state;
pub mod task;
pub mod tool;

pub use cost::Cost;
pub use error::CoreError;
pub use event::{Event, EventRecord};
pub use ids::{ApprovalId, DecisionId, DeviceId, NoteId, ReservationId, RunId, TaskId, ToolId};
pub use note::{BrainMode, LessonState, NoteType, ProposalKind};
pub use permission::{Approval, ApprovalKind, ApprovalState, Capability, PermissionMode};
pub use quality::{QualityKind, QualityNote};
pub use reservation::{Reservation, ReservationState};
pub use scope::Scope;
pub use state::{ConnectionState, Decision, DecisionState, HarnessState, TailscaleState};
pub use task::{Dependency, DependencyKind, Task, TaskFlags, TaskStatus, Tier};
pub use tool::{ToolInfo, ToolStatus};
