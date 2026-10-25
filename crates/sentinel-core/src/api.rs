//! Request and response bodies of the HTTP API (`docs/architecture.md`, API). The daemon and the
//! UI both use these types, so a route change is a compile error on both sides.

use serde::{Deserialize, Serialize};

use crate::chat::AttachmentKind;
use crate::event::EventRecord;
use crate::ids::{AttachmentId, ChatId, DeviceId, TaskId};
use crate::intent::Intent;
use crate::note::BrainMode;
use crate::permission::PermissionMode;
use crate::snapshot::{AgentCapabilities, Routing};
use crate::state::TailscaleState;

pub use crate::api_projects::{
    AddProject, ClearData, DataScope, KeyBody, KeyInfo, KeyTest, ProjectInfo,
};

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
    /// The chat the run belongs to. A chat is made when this is `None`.
    #[serde(default)]
    pub chat: Option<ChatId>,
    /// What the user asked for in plain words. The planner turns it into tasks.
    #[serde(default)]
    pub text: Option<String>,
    /// Attachments of the chat that go with this request.
    #[serde(default)]
    pub attachments: Vec<AttachmentId>,
    /// The type of the message. The daemon works it out when this is `None`.
    #[serde(default)]
    pub intent: Option<Intent>,
}

/// `POST /api/chats`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateChat {
    /// A title. Empty means "New chat" until the first message names it.
    #[serde(default)]
    pub title: Option<String>,
}

/// `PUT /api/chats/:id`. Every field is optional. Only what is present changes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateChat {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub pinned: Option<bool>,
    #[serde(default)]
    pub archived: Option<bool>,
    /// A tool id, or `auto` (or an empty string) to pick the agent for each task again.
    #[serde(default)]
    pub agent: Option<String>,
}

/// `POST /api/chats/:id/attachments`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddAttachment {
    pub kind: AttachmentKind,
    /// A repository path, for a file, folder, or backlog.
    #[serde(default)]
    pub path: Option<String>,
    /// The text of a note.
    #[serde(default)]
    pub text: Option<String>,
    /// The file name of an upload.
    #[serde(default)]
    pub name: Option<String>,
    /// The bytes of an upload, base64.
    #[serde(default)]
    pub content_base64: Option<String>,
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

/// `POST /api/tools/:id/test`. The message is one plain sentence and never holds a key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolTest {
    pub tool: String,
    pub ok: bool,
    pub message: String,
    pub millis: u64,
}

/// `POST /api/brain/mode`. From a phone only a stricter mode is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetBrainMode {
    pub mode: BrainMode,
}

/// `POST /api/brain/seed`: how many project notes the seed wrote, and how many already existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedReport {
    pub created: u32,
    pub existing: u32,
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

/// `GET /api/repo/tree?path=`. One folder of the repository and its direct subfolders. Paths use
/// `/` and are relative to the repository root. The root has an empty path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoTree {
    pub path: String,
    /// The folder above this one. `None` at the root.
    pub parent: Option<String>,
    pub folders: Vec<TreeFolder>,
    /// Files directly in this folder, not counting subfolders.
    pub files_here: u32,
}

/// One subfolder of a `RepoTree`. The counts are direct children only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeFolder {
    pub name: String,
    pub path: String,
    pub files: u32,
    pub folders: u32,
}

/// `POST /api/doctor` and the install check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub rows: Vec<CheckRow>,
}

/// A frame the client sends on `/ws`. Control never travels over the socket: it uses the routes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ClientFrame {
    /// The first frame. A browser cannot set headers on a WebSocket, so the token travels here.
    Auth {
        token: String,
    },
    /// Send every event after this id, then live events.
    Subscribe {
        after: i64,
    },
    Ping,
}

/// A frame the daemon sends on `/ws`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ServerFrame {
    Event { record: EventRecord },
    Pong,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_use_an_op_tag() {
        let json = serde_json::to_string(&ClientFrame::Subscribe { after: 4 }).unwrap();
        assert_eq!(json, "{\"op\":\"subscribe\",\"after\":4}");
        assert_eq!(
            serde_json::to_string(&ServerFrame::Pong).unwrap(),
            "{\"op\":\"pong\"}"
        );
    }

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

/// Whether the always-on service is set up on this computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    Installed,
    NotInstalled,
}

/// `GET /api/whoami`. Who is calling: the computer, or a paired phone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhoAmI {
    pub from_pc: bool,
    pub name: String,
}

/// `GET /api/host`. What the host card in Settings shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostStatus {
    pub service: ServiceState,
    /// True when the daemon runs without native dialogs.
    pub headless: bool,
    /// Seconds since the daemon started.
    pub uptime_seconds: u64,
    /// The last heartbeat, RFC 3339, when there is one.
    pub last_heartbeat: Option<String>,
    pub tailscale: TailscaleStatus,
    pub agents_ready: u32,
    pub agents_total: u32,
    pub repo_path: Option<String>,
}

/// One file of the working tree, as `GET /api/changes` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedFile {
    /// Relative to the repository root.
    pub path: String,
    /// One of `modified`, `added`, `deleted`, `renamed`, `untracked`.
    pub status: String,
    pub added: u32,
    pub removed: u32,
}

/// `GET /api/changes/diff`. The diff of one file against HEAD, cut at 200 KB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeDiff {
    pub path: String,
    pub diff: String,
    pub truncated: bool,
}

/// One task of the backlog file, before a run plans it. `GET /api/backlog/tasks`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacklogTask {
    pub id: String,
    pub title: String,
    /// The first non-empty line of the task body, at most 160 characters.
    pub summary: String,
}
