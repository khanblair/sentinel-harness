//! Views that hang off the shell. Panels and modal bodies land here one at a time; until a
//! panel is built, its module re-exports a stub from `panel_stubs`.

pub mod modal_stubs;
pub mod panel_stubs;
pub mod stream_slot;

pub mod agents;

pub mod board;

pub mod brain {
    pub use super::panel_stubs::BrainPanel;
}

pub mod changes;

pub mod cost;

pub mod files;

pub mod permissions;

pub mod remote {
    pub use super::panel_stubs::RemotePanel;
}

pub mod report;

pub mod reserved;

pub mod telegram {
    pub use super::panel_stubs::TelegramPanel;
}

pub mod terminal;
