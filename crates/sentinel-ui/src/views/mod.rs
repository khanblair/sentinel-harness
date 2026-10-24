//! Views that hang off the shell. Panels and modal bodies land here one at a time; until a
//! panel is built, its module re-exports a stub from `panel_stubs`.

pub mod modal_stubs;
pub mod panel_stubs;
pub mod stream_slot;

pub mod agents {
    pub use super::panel_stubs::AgentsPanel;
}

pub mod board;

pub mod brain {
    pub use super::panel_stubs::BrainPanel;
}

pub mod changes;

pub mod cost {
    pub use super::panel_stubs::CostPanel;
}

pub mod files;

pub mod permissions {
    pub use super::panel_stubs::PermissionsPanel;
}

pub mod remote {
    pub use super::panel_stubs::RemotePanel;
}

pub mod report {
    pub use super::panel_stubs::ReportPanel;
}

pub mod reserved;

pub mod telegram {
    pub use super::panel_stubs::TelegramPanel;
}

pub mod terminal;
