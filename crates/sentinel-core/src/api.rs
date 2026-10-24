//! Request and response bodies of the HTTP API (`docs/architecture.md`, API). The daemon and the
//! UI both use these types, so a route change is a compile error on both sides.

use serde::{Deserialize, Serialize};

use crate::ids::{DeviceId, TaskId};
use crate::note::BrainMode;
use crate::permission::PermissionMode;
use crate::snapshot::{AgentCapabilities, Routing};
use crate::state::TailscaleState;

/// Every error response has this body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
}

/// `POST /api/run`. With no fields it runs the whole backlog of the chosen repository.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRequest {
    /// Path of a backlog file. The daemon's configured backlog is used when this is `None`.
    pub backlog: Option<String>,
    /// Run only this task.
    pub task: Option<TaskId>,
}

/// `POST /api/cap/lower`. There is no route that raises the cap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LowerCap {
    /// Dollars as text, for example `1.50`.
    pub cap: String,
}

/// `POST /api/decisions/:id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnswerDecision {
    /// A task id from the decision, or `skip_both`.
    pub answer: String,
}

/// `POST /api/approvals/:id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnswerApproval {
    pub allow: bool,
}

/// `PUT /api/permissions`. Fields left out stay as they are.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionsUpdate {
    pub mode: Option<PermissionMode>,
    pub capabilities: Option<Vec<AgentCapabilities>>,
}

/// `PUT /api/routing`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingUpdate {
    pub routing: Routing,
}

/// `PUT /api/tools/:id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolToggle {
    pub enabled: bool,
}

/// `POST /api/brain/mode`. From a phone only a stricter mode is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetBrainMode {
    pub mode: BrainMode,
}

/// `GET /api/tailscale/status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailscaleStatus {
    pub state: TailscaleState,
    /// The tailnet address, such as `100.101.102.103`, when running.
    pub address: Option<String>,
    /// The machine name, when `MagicDNS` is on.
    pub machine_name: Option<String>,
}

/// `POST /api/pair/start`. Shown on the computer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairStart {
    pub tailscale: TailscaleStatus,
    /// The link a phone opens. Empty until Tailscale runs.
    pub url: String,
    /// The one-time code. Empty until Tailscale runs.
    pub code: String,
    /// The link as a QR code, an SVG document.
    pub qr_svg: String,
    pub expires_in_seconds: u32,
}

/// `POST /api/pair`. Sent by the phone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairSubmit {
    pub code: String,
    pub device_name: String,
}

/// The phone gets its token once. The daemon keeps only a hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairResult {
    pub device: DeviceId,
    pub token: String,
}

/// `POST /api/repo/check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoCheckRequest {
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Ok,
    NeedsAction,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRow {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
    /// The label of a fix button, when one can fix it.
    pub fix: Option<String>,
}

/// The git, backlog.md, and clean-tree rows for a chosen folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoCheckResponse {
    pub path: String,
    pub rows: Vec<CheckRow>,
    pub usable: bool,
}

/// One entry of `GET /api/fs/list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderEntry {
    pub name: String,
    pub path: String,
}

/// `POST /api/doctor` and the install check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub rows: Vec<CheckRow>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_request_defaults_to_the_whole_backlog() {
        let request: RunRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(request, RunRequest::default());
    }

    #[test]
    fn permissions_update_leaves_missing_fields_alone() {
        let update: PermissionsUpdate = serde_json::from_str("{\"mode\":\"bypass\"}").unwrap();
        assert_eq!(update.mode, Some(PermissionMode::Bypass));
        assert_eq!(update.capabilities, None);
    }
}
