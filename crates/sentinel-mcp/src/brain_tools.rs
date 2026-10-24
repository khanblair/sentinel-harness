//! The Brain tools. Agents search and read notes, and propose new ones. Nothing here writes a note:
//! a proposal is a file in `_proposals/` that waits for a reviewer.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sentinel_brain::mode::{may_read, may_write};
use sentinel_brain::note::{Author, NOTE_TYPES, Note, note_type_name, parse_note_type};
use sentinel_brain::proposal::{Proposal, ProposalState, write_proposal};
use sentinel_brain::{Index, Vault};
use sentinel_core::{BrainMode, NoteId, NoteType, ProposalKind};
use serde_json::Value;

use crate::protocol::argument;

pub const TOOL_NAMES: [&str; 3] = ["brain_search", "brain_read", "brain_propose"];
const SEARCH_LIMIT: u32 = 8;
const SNIPPET_CHARS: usize = 160;
const BODY_CHARS: usize = 4000;
const OFF: &str = "The Brain is off.";
const READ_ONLY: &str = "The Brain is read-only. An agent cannot propose a note here.";

pub struct BrainTools {
    root: Option<PathBuf>,
    mode: BrainMode,
    task_id: Option<String>,
}

impl BrainTools {
    pub fn new(root: Option<PathBuf>, mode: BrainMode, task_id: Option<String>) -> Self {
        Self {
            root,
            mode,
            task_id,
        }
    }

    /// Run one Brain tool. Errors come back as the text the model reads.
    pub fn call(&self, name: &str, args: &Value) -> Result<String, String> {
        match name {
            "brain_search" => self.search(argument(args, "query")?),
            "brain_read" => self.read(argument(args, "id")?),
            "brain_propose" => self.propose(args),
            other => Err(format!("Unknown tool: {other}.")),
        }
    }

    /// The vault folder, when agents may read the Brain.
    fn reader_root(&self) -> Result<&Path, String> {
        match self.root.as_deref() {
            Some(root) if may_read(self.mode) => Ok(root),
            _ => Err(OFF.to_owned()),
        }
    }

    fn open_vault(&self) -> Result<Vault, String> {
        let root = self.reader_root()?;
        Vault::open(root).map_err(|error| error.to_string())
    }

    fn search(&self, query: &str) -> Result<String, String> {
        let vault = self.open_vault()?;
        let mut index = Index::open_in_memory().map_err(|error| error.to_string())?;
        index.rebuild(&vault).map_err(|error| error.to_string())?;
        let hits = index
            .search(query, None, SEARCH_LIMIT)
            .map_err(|error| error.to_string())?;
        if hits.is_empty() {
            return Ok("No notes match.".to_owned());
        }
        let lines: Vec<String> = hits
            .iter()
            .map(|hit| {
                let snippet = cut_chars(&hit.snippet, SNIPPET_CHARS);
                format!(
                    "{} | {} | {} | {snippet}",
                    hit.id,
                    hit.title,
                    note_type_name(hit.note_type)
                )
            })
            .collect();
        Ok(lines.join("\n"))
    }

    fn read(&self, id: &str) -> Result<String, String> {
        let vault = self.open_vault()?;
        let note = vault
            .read(&NoteId::new(id))
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("No note has the id {id}."))?;
        Ok(format!(
            "{}\n\n{}",
            note.title,
            cut_chars(&note.body, BODY_CHARS)
        ))
    }

    fn propose(&self, args: &Value) -> Result<String, String> {
        let root = self.writer_root()?;
        let title = argument(args, "title")?.trim();
        if title.is_empty() {
            return Err("A proposal needs a title.".to_owned());
        }
        let note_type = proposed_type(argument(args, "note_type")?)?;
        let body = argument(args, "body")?;
        let id = new_proposal_id();
        let proposal = Proposal {
            id: NoteId::new(id.as_str()),
            kind: ProposalKind::Add,
            target: None,
            why: self.why(),
            note: Some(new_note(&id, title, note_type, body)),
            state: ProposalState::Open,
        };
        write_proposal(root, &proposal).map_err(|error| error.to_string())?;
        Ok(format!(
            "Saved proposal {id}. A reviewer decides whether it becomes a note."
        ))
    }

    /// The vault folder, when agents may propose notes. Only `learn` mode allows that.
    fn writer_root(&self) -> Result<&Path, String> {
        let root = self.reader_root()?;
        if may_write(self.mode) {
            Ok(root)
        } else {
            Err(READ_ONLY.to_owned())
        }
    }

    fn why(&self) -> String {
        match &self.task_id {
            Some(task) => format!("Proposed by an agent for task {task}."),
            None => "Proposed by an agent.".to_owned(),
        }
    }
}

/// The note types an agent may propose, as the text the tool schema shows. Lesson notes come
/// only from runs, so they are left out.
pub fn proposable_types() -> String {
    let names: Vec<&str> = NOTE_TYPES
        .into_iter()
        .filter(|note_type| *note_type != NoteType::Lesson)
        .map(note_type_name)
        .collect();
    format!("One of: {}.", names.join(", "))
}

/// The brain mode from `SENTINEL_BRAIN_MODE`. Anything unknown or unset is `off`.
pub fn parse_mode(raw: Option<&str>) -> BrainMode {
    match raw {
        Some("learn") => BrainMode::Learn,
        Some("read-only") => BrainMode::ReadOnly,
        _ => BrainMode::Off,
    }
}

fn proposed_type(raw: &str) -> Result<NoteType, String> {
    match parse_note_type(raw) {
        Some(NoteType::Lesson) | None => {
            Err(format!("Unknown note type {raw}. {}", proposable_types()))
        }
        Some(note_type) => Ok(note_type),
    }
}

fn new_note(id: &str, title: &str, note_type: NoteType, body: &str) -> Note {
    Note {
        id: NoteId::new(id),
        note_type,
        title: title.to_owned(),
        tags: Vec::new(),
        by: Author::Run,
        strength: sentinel_brain::strength::INITIAL,
        confirmed: None,
        pinned: false,
        lesson: None,
        body: body.to_owned(),
        extra: Vec::new(),
    }
}

static NEXT_PROPOSAL: AtomicU64 = AtomicU64::new(0);

/// A file-safe id unique within this process and across restarts (time plus a counter).
fn new_proposal_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let sequence = NEXT_PROPOSAL.fetch_add(1, Ordering::Relaxed);
    format!("agent-{nanos}-{sequence}")
}

fn cut_chars(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}
