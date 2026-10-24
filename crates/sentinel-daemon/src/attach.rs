//! Attachments: what the composer plus button adds to a chat. A file, a folder, or a backlog is a
//! path inside the repository. A note is text. An upload is a file kept under `.sentinel/chats/`,
//! outside the tracked tree.

use std::fs;
use std::path::{Component, Path};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use sentinel_core::api::AddAttachment;
use sentinel_core::chat::MAX_UPLOAD_BYTES;
use sentinel_core::{Attachment, AttachmentId, AttachmentKind, ChatId};

use crate::engine::{Engine, EngineError, Result};

/// The longest note.
const MAX_NOTE_BYTES: usize = 20_000;
/// A file this small has its text copied into the prompt. A bigger one goes in as a path.
const INLINE_FILE_BYTES: u64 = 4_000;
/// The most file text one request copies into prompts.
const INLINE_TOTAL_BYTES: usize = 12_000;
/// The most names listed for one folder.
const FOLDER_NAMES: usize = 30;
/// The longest chip name for a note.
const NOTE_NAME_CHARS: usize = 24;
/// The most skill text one skill chip keeps.
pub(crate) const SKILL_TEXT_MAX: usize = 8 * 1024;

impl Engine {
    /// Add an attachment to a chat. It goes with the next message.
    pub fn add_attachment(&self, chat: &ChatId, request: &AddAttachment) -> Result<Attachment> {
        let record = self.chat_record(chat)?;
        if record.archived_at.is_some() {
            return Err(EngineError::Refused(
                "This chat is archived. Unarchive it first.".to_owned(),
            ));
        }
        let id = AttachmentId::generate();
        let parts = if request.content_base64.is_some() {
            self.save_upload(chat, &id, request)?
        } else {
            self.describe(request)?
        };
        let attachment = Attachment {
            id,
            chat: chat.clone(),
            kind: parts.kind,
            name: parts.name,
            path: parts.path,
            text: parts.text,
            size: parts.size,
            sent: false,
            created_at: self.now(),
        };
        self.store().insert_attachment(&attachment)?;
        Ok(attachment)
    }

    /// The attachments of a chat that no message carried yet.
    pub fn pending_attachments(&self, chat: &ChatId) -> Result<Vec<Attachment>> {
        self.chat_record(chat)?;
        Ok(self.store().list_attachments(chat, true)?)
    }

    /// Remove an attachment that no message carried yet, and its upload.
    pub fn remove_attachment(&self, chat: &ChatId, id: &AttachmentId) -> Result<()> {
        let attachment = self
            .store()
            .get_attachment(id)?
            .filter(|a| &a.chat == chat)
            .ok_or_else(|| EngineError::NotFound(format!("attachment {id}")))?;
        if attachment.sent {
            return Err(EngineError::Refused(
                "A message already carried this attachment.".to_owned(),
            ));
        }
        self.store().delete_attachment(id)?;
        if let Some(path) = attachment.path.as_deref().filter(|p| is_upload_path(p)) {
            // The file may already be gone, which is what was asked for.
            let _ = fs::remove_file(self.repo_root().join(path));
        }
        Ok(())
    }

    fn describe(&self, request: &AddAttachment) -> Result<Parts> {
        match request.kind {
            AttachmentKind::Note => note_parts(request.text.as_deref().unwrap_or_default()),
            AttachmentKind::File | AttachmentKind::Backlog => {
                self.path_parts(request.kind, request.path.as_deref(), false)
            }
            AttachmentKind::Folder => self.path_parts(request.kind, request.path.as_deref(), true),
            AttachmentKind::Skill => self.skill_parts(request.name.as_deref()),
            AttachmentKind::Upload => Err(EngineError::Invalid(
                "An upload needs its bytes in content_base64.".to_owned(),
            )),
        }
    }

    /// A skill chip. Its path is the skill name, and its text is the skill body, cut to 8 KB.
    fn skill_parts(&self, name: Option<&str>) -> Result<Parts> {
        let detail = self.skill_detail(name.unwrap_or_default())?;
        let text = cut_at_char(&detail.body, SKILL_TEXT_MAX).to_owned();
        Ok(Parts {
            kind: AttachmentKind::Skill,
            name: detail.info.name.clone(),
            path: Some(detail.info.name),
            size: text.len() as u64,
            text: Some(text),
        })
    }

    fn path_parts(
        &self,
        kind: AttachmentKind,
        path: Option<&str>,
        want_dir: bool,
    ) -> Result<Parts> {
        let relative = clean_relative(path.unwrap_or_default())?;
        let absolute = self.repo_path().join(&relative);
        let meta = fs::metadata(&absolute)
            .map_err(|_| EngineError::NotFound(format!("{relative} in this project")))?;
        if meta.is_dir() != want_dir {
            let wanted = if want_dir { "folder" } else { "file" };
            return Err(EngineError::Invalid(format!(
                "{relative} is not a {wanted}."
            )));
        }
        Ok(Parts {
            kind,
            name: relative.clone(),
            path: Some(relative),
            text: None,
            size: if want_dir { 0 } else { meta.len() },
        })
    }

    fn save_upload(
        &self,
        chat: &ChatId,
        id: &AttachmentId,
        request: &AddAttachment,
    ) -> Result<Parts> {
        let kind = match request.kind {
            AttachmentKind::Backlog => AttachmentKind::Backlog,
            AttachmentKind::Upload => AttachmentKind::Upload,
            AttachmentKind::File
            | AttachmentKind::Folder
            | AttachmentKind::Note
            | AttachmentKind::Skill => {
                return Err(EngineError::Invalid(
                    "An upload is an upload or a backlog.".to_owned(),
                ));
            }
        };
        let bytes = STANDARD
            .decode(request.content_base64.as_deref().unwrap_or_default())
            .map_err(|_| EngineError::Invalid("The file bytes are not valid base64.".to_owned()))?;
        if bytes.len() > MAX_UPLOAD_BYTES {
            return Err(EngineError::Invalid(
                "Too large. The limit is 1 MB.".to_owned(),
            ));
        }
        let name = safe_name(request.name.as_deref().unwrap_or("upload"));
        let relative = format!(".sentinel/chats/{chat}/attachments/{id}-{name}");
        let target = self.repo_root().join(&relative);
        if let Some(folder) = target.parent() {
            fs::create_dir_all(folder)?;
        }
        fs::write(&target, &bytes)?;
        Ok(Parts {
            kind,
            name,
            path: Some(relative),
            text: None,
            size: bytes.len() as u64,
        })
    }
}

/// The fields of an attachment before it has an id and a time.
struct Parts {
    kind: AttachmentKind,
    name: String,
    path: Option<String>,
    text: Option<String>,
    size: u64,
}

fn note_parts(text: &str) -> Result<Parts> {
    let text = text.trim();
    if text.is_empty() {
        return Err(EngineError::Invalid("The note is empty.".to_owned()));
    }
    if text.len() > MAX_NOTE_BYTES {
        return Err(EngineError::Invalid("The note is too long.".to_owned()));
    }
    Ok(Parts {
        kind: AttachmentKind::Note,
        name: text.chars().take(NOTE_NAME_CHARS).collect(),
        path: None,
        text: Some(text.to_owned()),
        size: text.len() as u64,
    })
}

/// A repository path with no way out of the repository and no hidden internals.
pub(crate) fn clean_relative(path: &str) -> Result<String> {
    let trimmed = path.trim().trim_start_matches("./");
    if trimmed.is_empty() {
        return Err(EngineError::Invalid("A path is needed.".to_owned()));
    }
    let mut parts = Vec::new();
    for component in Path::new(trimmed).components() {
        if let Some(name) = plain_name(component)? {
            parts.push(name);
        }
    }
    Ok(parts.join("/"))
}

/// The name a path component adds, nothing for `.`, or why the component is not allowed.
fn plain_name(component: Component<'_>) -> Result<Option<String>> {
    match component {
        Component::Normal(name) => {
            let name = name.to_string_lossy().into_owned();
            if name == ".git" || name == ".sentinel" {
                return Err(EngineError::Refused(format!("{name} is not available.")));
            }
            Ok(Some(name))
        }
        Component::CurDir => Ok(None),
        Component::ParentDir | Component::RootDir | Component::Prefix(_) => Err(
            EngineError::Refused("A path must stay inside the project.".to_owned()),
        ),
    }
}

/// Letters, digits, dot, dash, and underscore only, and never empty or hidden.
fn safe_name(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .map_or_else(|| "upload".to_owned(), |n| n.to_string_lossy().into_owned());
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_start_matches('.');
    if trimmed.is_empty() {
        "upload".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn is_upload_path(path: &str) -> bool {
    path.starts_with(".sentinel/chats/")
}

/// The longest prefix of `text` that is at most `max` bytes and ends on a character boundary.
pub(crate) fn cut_at_char(text: &str, max: usize) -> &str {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// The text that goes into every task of a run: paths, small files, folder listings, and notes.
/// Backlogs are the source of the tasks, so they are not context.
pub(crate) fn context_text(repo: &Path, attachments: &[Attachment]) -> String {
    let mut out = String::new();
    let mut inlined = 0_usize;
    for attachment in attachments {
        let line = match attachment.kind {
            // Skills reach the prompt through `skill_text`, which also caps them.
            AttachmentKind::Backlog | AttachmentKind::Skill => continue,
            AttachmentKind::Note => format!("- Note: {}", attachment.text.as_deref().unwrap_or("")),
            AttachmentKind::Folder => folder_line(repo, attachment),
            AttachmentKind::File | AttachmentKind::Upload => {
                file_line(repo, attachment, &mut inlined)
            }
        };
        out.push_str(&line);
        out.push('\n');
    }
    if out.is_empty() {
        return out;
    }
    format!("Attached by the user:\n{}", out.trim_end())
}

fn folder_line(repo: &Path, attachment: &Attachment) -> String {
    let path = attachment.path.as_deref().unwrap_or("");
    let names: Vec<String> = fs::read_dir(repo.join(path))
        .map(|entries| {
            let mut names: Vec<String> = entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| !n.starts_with('.'))
                .collect();
            names.sort();
            names.truncate(FOLDER_NAMES);
            names
        })
        .unwrap_or_default();
    format!("- Folder {path}/ holds: {}", names.join(", "))
}

fn file_line(repo: &Path, attachment: &Attachment, inlined: &mut usize) -> String {
    let path = attachment.path.as_deref().unwrap_or("");
    let label = format!("- File {path}");
    let small = attachment.size <= INLINE_FILE_BYTES && *inlined < INLINE_TOTAL_BYTES;
    let text = small
        .then(|| fs::read_to_string(repo.join(path)).ok())
        .flatten()
        .filter(|text| *inlined + text.len() <= INLINE_TOTAL_BYTES);
    match text {
        Some(text) => {
            *inlined += text.len();
            format!("{label}:\n```\n{}\n```", text.trim_end())
        }
        None => label,
    }
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::*;

    fn attachment(
        kind: AttachmentKind,
        path: Option<&str>,
        text: Option<&str>,
        size: u64,
    ) -> Attachment {
        Attachment {
            id: AttachmentId::new("a"),
            chat: ChatId::new("c"),
            kind,
            name: path.unwrap_or("note").to_owned(),
            path: path.map(str::to_owned),
            text: text.map(str::to_owned),
            size,
            sent: false,
            created_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn a_path_may_not_leave_the_project_or_name_internals() {
        assert!(clean_relative("../secret").is_err());
        assert!(clean_relative("/etc/passwd").is_err());
        assert!(clean_relative(".git/config").is_err());
        assert!(clean_relative("src/.sentinel/x").is_err());
        assert_eq!(clean_relative("./src//lib.rs").unwrap(), "src/lib.rs");
    }

    #[test]
    fn upload_names_are_made_safe() {
        assert_eq!(safe_name("../../etc/pass wd.txt"), "pass_wd.txt");
        assert_eq!(safe_name(".hidden"), "hidden");
        assert_eq!(safe_name("..."), "upload");
    }

    #[test]
    fn a_note_needs_text_and_gets_a_short_name() {
        assert!(note_parts("  ").is_err());
        let parts = note_parts("Use the builder pattern for every new struct here").unwrap();
        assert_eq!(parts.name.chars().count(), NOTE_NAME_CHARS);
    }

    #[test]
    fn context_includes_a_small_file_a_folder_listing_and_a_note() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "pub fn a() {}\n").unwrap();
        fs::write(dir.path().join("src/b.rs"), "").unwrap();
        let items = [
            attachment(AttachmentKind::File, Some("src/lib.rs"), None, 14),
            attachment(AttachmentKind::Folder, Some("src"), None, 0),
            attachment(AttachmentKind::Note, None, Some("Keep it small."), 14),
            attachment(AttachmentKind::Backlog, Some("backlog.md"), None, 5),
        ];
        let text = context_text(dir.path(), &items);
        assert!(text.starts_with("Attached by the user:"), "{text}");
        assert!(text.contains("pub fn a() {}"), "{text}");
        assert!(text.contains("Folder src/ holds: b.rs, lib.rs"), "{text}");
        assert!(text.contains("Note: Keep it small."), "{text}");
        assert!(!text.contains("backlog.md"), "{text}");
    }

    #[test]
    fn a_big_file_goes_in_as_a_path_only() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("big.txt"), "x".repeat(5000)).unwrap();
        let items = [attachment(
            AttachmentKind::File,
            Some("big.txt"),
            None,
            5000,
        )];
        let text = context_text(dir.path(), &items);
        assert_eq!(text, "Attached by the user:\n- File big.txt");
    }

    #[test]
    fn no_attachments_give_no_context() {
        assert_eq!(context_text(Path::new("."), &[]), "");
    }
}
