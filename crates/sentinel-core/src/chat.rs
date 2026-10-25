//! Chats and attachments (`docs/product-scope.md`, F23). A chat is one conversation in a project.
//! A run is one pass of the harness that a message starts, and a chat holds many runs.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::ids::{AttachmentId, ChatId, ToolId};

/// The most bytes of one uploaded file.
pub const MAX_UPLOAD_BYTES: usize = 1_000_000;
/// The longest title a chat gets from its first message.
pub const AUTO_TITLE_CHARS: usize = 48;
/// The title of a chat that has no message yet.
pub const NEW_CHAT_TITLE: &str = "New chat";
/// The agent value that means "pick the agent for each task by difficulty".
pub const AUTO_AGENT: &str = "auto";

/// What the sidebar icon of a chat shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRunState {
    /// No run yet.
    Idle,
    /// A plan waits for approval.
    Planned,
    Running,
    Paused,
    /// The last run finished and every task is verified.
    Done,
    /// The last run finished with tasks that are not verified.
    Blocked,
    Stopped,
}

/// One row of the chat list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatInfo {
    pub id: ChatId,
    /// The repository name the chat belongs to.
    pub project: String,
    pub title: String,
    pub pinned: bool,
    pub archived: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub run: ChatRunState,
    pub verified: u32,
    pub total: u32,
    /// An approval or decision of this chat waits for the user.
    pub needs_you: bool,
    /// The agent every message of this chat uses. `None` means auto.
    #[serde(default)]
    pub agent: Option<ToolId>,
}

/// What changed in a chat, for the `ChatChanged` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum ChatChange {
    Created,
    Renamed {
        title: String,
    },
    Pinned {
        pinned: bool,
    },
    Archived {
        archived: bool,
    },
    /// The agent of the chat. `None` means auto.
    AgentChanged {
        agent: Option<ToolId>,
    },
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    /// A file of the repository.
    File,
    /// A folder of the repository.
    Folder,
    /// A backlog file of the repository, or an upload used as the backlog.
    Backlog,
    /// Text that goes into the agents' prompts.
    Note,
    /// A file uploaded from the device, kept under `.sentinel/chats/`.
    Upload,
    /// A skill picked by name. Its text goes into the prompts of the run. It has no file.
    Skill,
}

/// Something added with the composer plus button. It goes with the next message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub id: AttachmentId,
    pub chat: ChatId,
    pub kind: AttachmentKind,
    /// The short name shown on the chip.
    pub name: String,
    /// The repository path of a file, folder, or backlog, or the stored path of an upload.
    pub path: Option<String>,
    /// The text of a note.
    pub text: Option<String>,
    pub size: u64,
    /// True once a message carried it.
    pub sent: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// The part of an attachment a stream message shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentRef {
    pub kind: AttachmentKind,
    pub name: String,
}

impl Attachment {
    pub fn reference(&self) -> AttachmentRef {
        AttachmentRef {
            kind: self.kind,
            name: self.name.clone(),
        }
    }
}

/// The title a first message gives its chat: the first line, cut at 48 characters.
pub fn title_from_message(text: &str) -> String {
    let first = text.lines().map(str::trim).find(|line| !line.is_empty());
    let Some(first) = first else {
        return NEW_CHAT_TITLE.to_owned();
    };
    if first.chars().count() <= AUTO_TITLE_CHARS {
        return first.to_owned();
    }
    let cut: String = first.chars().take(AUTO_TITLE_CHARS).collect();
    format!("{}...", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_first_line_becomes_the_title() {
        assert_eq!(
            title_from_message("Add a rate limit\nmore text"),
            "Add a rate limit"
        );
    }

    #[test]
    fn a_long_first_line_is_cut_with_dots() {
        let title = title_from_message(&"word ".repeat(30));
        assert!(title.ends_with("..."));
        assert!(title.chars().count() <= AUTO_TITLE_CHARS + 3);
    }

    #[test]
    fn leading_blank_lines_are_skipped() {
        assert_eq!(title_from_message("\n\n  Fix tests  \nrest"), "Fix tests");
    }

    #[test]
    fn an_empty_message_keeps_the_default_title() {
        assert_eq!(title_from_message("  \n "), NEW_CHAT_TITLE);
    }

    #[test]
    fn attachment_kinds_serialize_in_snake_case() {
        let json = serde_json::to_string(&AttachmentKind::Backlog).unwrap();
        assert_eq!(json, "\"backlog\"");
    }

    #[test]
    fn a_reference_keeps_the_kind_and_the_name() {
        let attachment = Attachment {
            id: AttachmentId::new("a1"),
            chat: ChatId::new("c1"),
            kind: AttachmentKind::File,
            name: "src/lib.rs".to_owned(),
            path: Some("src/lib.rs".to_owned()),
            text: None,
            size: 10,
            sent: false,
            created_at: OffsetDateTime::UNIX_EPOCH,
        };
        assert_eq!(attachment.reference().name, "src/lib.rs");
    }
}
