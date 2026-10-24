//! Shared types for the daemon and the UI. One definition, no schema drift.

pub mod api;
pub mod brain_api;
pub mod chat;
pub mod cost;
pub mod dry_run;
pub mod error;
pub mod event;
pub mod ids;
pub mod note;
pub mod permission;
pub mod quality;
pub mod reservation;
pub mod routines;
pub mod scope;
pub mod skills;
pub mod snapshot;
pub mod state;
pub mod task;
pub mod telegram_api;
pub mod tool;

pub use chat::{Attachment, AttachmentKind, AttachmentRef, ChatChange, ChatInfo, ChatRunState};
pub use cost::Cost;
pub use error::CoreError;
pub use event::{Event, EventRecord};
pub use ids::{
    ApprovalId, AttachmentId, ChatId, DecisionId, DeviceId, NoteId, ReservationId, RoutineId,
    RunId, TaskId, ToolId,
};
pub use note::{BrainMode, LessonState, NoteType, ProposalKind};
pub use permission::{Approval, ApprovalKind, ApprovalState, Capability, PermissionMode};
pub use quality::{QualityKind, QualityNote};
pub use reservation::{Reservation, ReservationState};
pub use scope::Scope;
pub use snapshot::{
    AgentCapabilities, AgentCost, CostSnapshot, DeviceInfo, DeviceKind, OnboardingState, RepoInfo,
    Routing, RunInfo, Snapshot,
};
pub use state::{ConnectionState, Decision, DecisionState, HarnessState, TailscaleState};
pub use task::{Dependency, DependencyKind, Task, TaskFlags, TaskStatus, Tier};
pub use tool::{ToolInfo, ToolStatus};
