//! Views that hang off the shell. Panels and modal bodies land here one at a time; until a
//! panel is built, its module re-exports a stub from `panel_stubs`.

pub mod agents;
pub mod appearance;
pub mod board;
pub mod budget_step;
pub mod changes;
pub mod composer;
pub mod cost;
pub mod dry_run;
pub mod files;
pub mod folder_picker;
pub mod install;
pub mod onboarding;
pub mod pair_page;
pub mod pairing;
pub mod panel_stubs;
pub mod permissions;
pub mod remote;
pub mod report;
pub mod reserved;
pub mod settings;
pub mod settings_pages;
pub mod stream;
pub mod stream_model;
pub mod telegram {
    pub use super::panel_stubs::TelegramPanel;
}
pub mod brain;
pub mod brain_graph;
pub mod brain_notes;
pub mod brain_proposals;
pub mod brain_settings;
pub mod telegram_slot;
pub mod terminal;
