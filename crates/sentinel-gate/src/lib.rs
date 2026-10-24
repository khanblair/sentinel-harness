//! Permission logic as pure functions and in-memory approval cards. No I/O,
//! no clock, no store: the caller reads the mode and capabilities, then
//! persists what this crate says.

pub mod book;
pub mod policy;

pub use book::ApprovalBook;
pub use policy::{
    ExtensionAction, WaveGate, capability_allows, default_capabilities, extension_action,
    next_mode_after_plan_approval, plan_gate, wave_gate, write_is_never_an_approval,
};
