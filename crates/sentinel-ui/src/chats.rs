//! Pure logic of the chat list and the chat visuals: grouping by project, row icons, counts, which
//! chat is open, rename, the chat menu, and the attachment pickers. Nothing here touches the DOM
//! or a signal, so each rule has a unit test (`chats/tests.rs`).

use std::collections::BTreeSet;

use sentinel_core::chat::NEW_CHAT_TITLE;
use sentinel_core::{
    Attachment, AttachmentId, AttachmentKind, ChatId, ChatInfo, ChatRunState, Snapshot,
};

/// The group that holds archived chats. It comes last and starts collapsed.
pub const ARCHIVED_GROUP: &str = "Archived";

/// Most folders the Add a folder list shows.
pub const FOLDER_LIMIT: usize = 60;

/// Characters kept in an attachment chip, the ellipsis included.
pub const CHIP_NAME_CHARS: usize = 24;

/// The largest upload the page sends, in bytes.
const UPLOAD_LIMIT_BYTES: u64 = 1_000_000;

/// Chats of one project, or the archived group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectGroup {
    pub project: String,
    pub chats: Vec<ChatRow>,
}

/// One chat row of the sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRow {
    pub id: ChatId,
    pub title: String,
    pub pinned: bool,
    pub archived: bool,
    pub icon: RowIcon,
    pub count: String,
    pub needs_you: bool,
}

/// The status icon at the end of a chat row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowIcon {
    Spinner,
    Paused,
    Done,
    Blocked,
    Planned,
    Stopped,
    None,
}

/// A chat whose title is being edited in its row. `title` is the name before the edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameState {
    pub chat: ChatId,
    pub title: String,
}

/// The icon for a chat's run state. An idle chat has no icon.
pub fn row_icon(state: ChatRunState) -> RowIcon {
    match state {
        ChatRunState::Idle => RowIcon::None,
        ChatRunState::Planned => RowIcon::Planned,
        ChatRunState::Running => RowIcon::Spinner,
        ChatRunState::Paused => RowIcon::Paused,
        ChatRunState::Done => RowIcon::Done,
        ChatRunState::Blocked => RowIcon::Blocked,
        ChatRunState::Stopped => RowIcon::Stopped,
    }
}

/// `verified/total` once the chat has tasks, else nothing.
pub fn count_text(chat_info: &ChatInfo) -> String {
    if chat_info.total > 0 {
        format!("{}/{}", chat_info.verified, chat_info.total)
    } else {
        String::new()
    }
}

fn row_from(chat_info: &ChatInfo) -> ChatRow {
    ChatRow {
        id: chat_info.id.clone(),
        title: chat_info.title.clone(),
        pinned: chat_info.pinned,
        archived: chat_info.archived,
        icon: row_icon(chat_info.run),
        count: count_text(chat_info),
        needs_you: chat_info.needs_you,
    }
}

/// A title matches when the search text (already trimmed and lower case) is inside it.
/// An empty search matches every title.
fn matches_query(title: &str, needle: &str) -> bool {
    needle.is_empty() || title.to_lowercase().contains(needle)
}

/// Chats grouped by project, in the order each project first appears. Archived chats go to one
/// `Archived` group at the end. The query filters by title, ignoring case and outer spaces.
/// Groups with no matching chat are left out.
pub fn group_chats(chats: &[ChatInfo], query: &str) -> Vec<ProjectGroup> {
    let needle = query.trim().to_lowercase();
    let mut groups: Vec<ProjectGroup> = Vec::new();
    let mut archived: Vec<ChatRow> = Vec::new();
    for chat_info in chats
        .iter()
        .filter(|chat_info| matches_query(&chat_info.title, &needle))
    {
        if chat_info.archived {
            archived.push(row_from(chat_info));
        } else {
            push_into_project(&mut groups, &chat_info.project, row_from(chat_info));
        }
    }
    if !archived.is_empty() {
        groups.push(ProjectGroup {
            project: ARCHIVED_GROUP.to_owned(),
            chats: archived,
        });
    }
    groups
}

fn push_into_project(groups: &mut Vec<ProjectGroup>, project: &str, row: ChatRow) {
    match groups.iter_mut().find(|group| group.project == project) {
        Some(group) => group.chats.push(row),
        None => groups.push(ProjectGroup {
            project: project.to_owned(),
            chats: vec![row],
        }),
    }
}

fn has_chat(snapshot: &Snapshot, id: &ChatId) -> bool {
    snapshot.chats.iter().any(|chat| chat.id == *id)
}

/// The chat the stream opens on. An open chat that still exists stays open, archived or not.
/// Otherwise the chat of the live run, then the first chat that is not archived.
pub fn pick_open_chat(snapshot: &Snapshot, open: Option<&ChatId>) -> Option<ChatId> {
    open.filter(|id| has_chat(snapshot, id))
        .cloned()
        .or_else(|| snapshot.run.as_ref().map(|run| run.chat.clone()))
        .or_else(|| {
            snapshot
                .chats
                .iter()
                .find(|chat| !chat.archived)
                .map(|chat| chat.id.clone())
        })
}

/// The title of a chat. A chat the snapshot does not hold, or no chat, shows "New chat".
pub fn chat_title(snapshot: &Snapshot, id: Option<&ChatId>) -> String {
    id.and_then(|id| snapshot.chats.iter().find(|chat| chat.id == *id))
        .map_or_else(|| NEW_CHAT_TITLE.to_owned(), |chat| chat.title.clone())
}

/// The toast shown when a message is sent while another chat holds the live run.
pub fn busy_notice(snapshot: &Snapshot, live: Option<&ChatId>) -> String {
    format!(
        "Another chat is working in this project: {}. Open it or stop it.",
        chat_title(snapshot, live)
    )
}

/// The new title typed into a row, or `None` when nothing should be saved: the name is empty
/// after trimming, or it is the same as the current title.
pub fn commit_rename(current: &str, typed: &str) -> Option<String> {
    let name = typed.trim();
    (!name.is_empty() && name != current).then(|| name.to_owned())
}

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn sextet(value: u32) -> char {
    let index = usize::try_from(value & 0x3f).unwrap_or(0);
    char::from(BASE64_ALPHABET[index])
}

/// Standard base64 with `=` padding. Used for uploads, so no crate is needed.
pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let mut block = [0_u8; 3];
        block[..chunk.len()].copy_from_slice(chunk);
        let n = (u32::from(block[0]) << 16) | (u32::from(block[1]) << 8) | u32::from(block[2]);
        let chars = [sextet(n >> 18), sextet(n >> 12), sextet(n >> 6), sextet(n)];
        let kept = chunk.len() + 1;
        for (index, symbol) in chars.into_iter().enumerate() {
            out.push(if index < kept { symbol } else { '=' });
        }
    }
    out
}

/// The distinct parent folders of the repository files, sorted, at most `FOLDER_LIMIT`.
pub fn folders_of(files: &[String]) -> Vec<String> {
    let folders: BTreeSet<&str> = files
        .iter()
        .filter_map(|path| path.rsplit_once('/').map(|(folder, _)| folder))
        .filter(|folder| !folder.is_empty())
        .collect();
    folders
        .into_iter()
        .take(FOLDER_LIMIT)
        .map(str::to_owned)
        .collect()
}

/// The files whose path contains the query, ignoring case and outer spaces. An empty query keeps all.
pub fn filter_files(files: &[String], query: &str) -> Vec<String> {
    let needle = query.trim().to_lowercase();
    files
        .iter()
        .filter(|path| needle.is_empty() || path.to_lowercase().contains(&needle))
        .cloned()
        .collect()
}

fn shorten(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let kept: String = text.chars().take(limit.saturating_sub(1)).collect();
    format!("{kept}\u{2026}")
}

/// The text on an attachment chip. A note shows its text; anything else shows its name.
pub fn chip_label(attachment: &Attachment) -> String {
    let full = match attachment.kind {
        AttachmentKind::Note => attachment.text.as_deref().unwrap_or_default(),
        AttachmentKind::File
        | AttachmentKind::Folder
        | AttachmentKind::Backlog
        | AttachmentKind::Upload => attachment.name.as_str(),
    };
    shorten(full, CHIP_NAME_CHARS)
}

/// The icon name for an attachment kind.
pub fn chip_icon(kind: AttachmentKind) -> &'static str {
    match kind {
        AttachmentKind::File => "file",
        AttachmentKind::Folder => "folder",
        AttachmentKind::Backlog => "list",
        AttachmentKind::Note => "edit",
        AttachmentKind::Upload => "upload",
    }
}

/// An upload above the limit is never sent. The limit is 1 MB.
pub fn too_large(size: u64) -> bool {
    size > UPLOAD_LIMIT_BYTES
}

/// The ids of the pending attachments of one kind. A single choice replaces these.
pub fn same_kind_ids(attachments: &[Attachment], kind: AttachmentKind) -> Vec<AttachmentId> {
    attachments
        .iter()
        .filter(|attachment| attachment.kind == kind)
        .map(|attachment| attachment.id.clone())
        .collect()
}

/// A path whose extension is `md`, in any letter case.
fn is_markdown(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("md"))
}

/// The four kinds of the composer's add dialogs that pick repository paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerKind {
    Files,
    Folder,
    Backlog,
}

impl PickerKind {
    pub fn attachment_kind(self) -> AttachmentKind {
        match self {
            PickerKind::Files => AttachmentKind::File,
            PickerKind::Folder => AttachmentKind::Folder,
            PickerKind::Backlog => AttachmentKind::Backlog,
        }
    }

    /// Folder and backlog take one choice. Files take many.
    pub fn single(self) -> bool {
        self != PickerKind::Files
    }

    /// Folders come only from the project, so only files and backlogs have a device tab.
    pub fn has_device_tab(self) -> bool {
        self != PickerKind::Folder
    }

    /// The rows the picker offers, before the filter: files, folders, or `.md` files.
    pub fn rows(self, files: &[String]) -> Vec<String> {
        match self {
            PickerKind::Files => files.to_vec(),
            PickerKind::Folder => folders_of(files),
            PickerKind::Backlog => files
                .iter()
                .filter(|path| is_markdown(path))
                .cloned()
                .collect(),
        }
    }

    /// The primary button text for `count` picked rows.
    pub fn confirm_label(self, count: usize) -> String {
        match self {
            PickerKind::Files if count == 1 => "Add 1 file".to_owned(),
            PickerKind::Files => format!("Add {count} files"),
            PickerKind::Folder => "Add folder".to_owned(),
            PickerKind::Backlog => "Use file".to_owned(),
        }
    }
}

pub use menu::{MenuAction, MenuEntry, menu_items, title_menu_items};

mod menu;

#[cfg(test)]
#[path = "chats/tests.rs"]
mod tests;
