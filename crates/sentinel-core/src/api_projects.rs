//! Project, data-reset, and key bodies of the HTTP API. `api.rs` re-exports them, so each one keeps
//! its `sentinel_core::api` path.

use serde::{Deserialize, Serialize};

/// One project of the app: a folder with its own chats and its own `.sentinel` state. `GET
/// /api/projects` lists them. The first one is the primary: its state also holds the paired
/// phones, the saved keys, Telegram, and the routines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub primary: bool,
    /// True while a run is live in this project.
    pub live: bool,
    pub chats: Vec<crate::ChatInfo>,
}

/// `POST /api/projects`: add a folder as a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddProject {
    pub path: String,
}

/// What a data reset can remove. The first four belong to one project. The rest belong to the
/// app and live in the primary project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataScope {
    /// Chats, their events, and their uploads.
    Chats,
    /// The Brain notes and their index.
    Brain,
    /// Runs: tasks, reservations, checkpoints, approvals, decisions, costs, and the report.
    History,
    /// The project settings, back to the defaults.
    Settings,
    /// Every paired phone. This computer stays signed in.
    Devices,
    /// The keys Sentinel keeps in the system keychain.
    Keys,
    /// The Telegram settings and log.
    Telegram,
    /// The routines and their briefs.
    Routines,
    /// The extra projects in the list. The folders are not touched.
    Projects,
    /// The setup wizard starts again.
    Onboarding,
}

impl DataScope {
    pub const PROJECT: [DataScope; 4] = [
        DataScope::Chats,
        DataScope::Brain,
        DataScope::History,
        DataScope::Settings,
    ];
    pub const APP: [DataScope; 6] = [
        DataScope::Devices,
        DataScope::Keys,
        DataScope::Telegram,
        DataScope::Routines,
        DataScope::Projects,
        DataScope::Onboarding,
    ];

    pub fn is_project(self) -> bool {
        Self::PROJECT.contains(&self)
    }
}

/// `POST /api/data/project` and `POST /api/data/app`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClearData {
    pub scopes: Vec<DataScope>,
}

/// One row of `GET /api/keys`. The hint is the last four characters of a saved key, and only
/// for a key of at least twelve characters. No other part of a key leaves the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyInfo {
    pub provider: String,
    pub label: String,
    pub has_key: bool,
    pub hint: Option<String>,
}

/// `PUT /api/keys/{provider}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBody {
    pub key: String,
}

/// `POST /api/keys/{provider}/test`. The message is one plain sentence and never holds the key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyTest {
    pub provider: String,
    pub ok: bool,
    pub message: String,
}
