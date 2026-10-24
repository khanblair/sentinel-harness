//! Proposals: changes to notes, saved as files in `_proposals/`. A change to a note you wrote needs an accept.

use std::path::{Path, PathBuf};

use sentinel_core::{LessonState, NoteId, ProposalKind};

use crate::body::{generated_block, replace_generated};
use crate::error::{BrainError, Result};
use crate::frontmatter::{self, Pairs, quote_if_needed};
use crate::note::{Author, Note, parse_note, render_note};
use crate::strength;
use crate::vault::{Vault, is_note_file};

const PROPOSAL_FOLDER: &str = "_proposals";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalState {
    Open,
    Accepted,
    Dismissed,
}

/// One proposed change. `note` is the new or merged note; `target` is the note it changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposal {
    pub id: NoteId,
    pub kind: ProposalKind,
    pub target: Option<NoteId>,
    pub why: String,
    pub note: Option<Note>,
    pub state: ProposalState,
}

/// What `apply` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    Added(NoteId),
    Updated(NoteId),
    Retired(NoteId),
    Merged(NoteId),
    /// Auto-apply refused: the change is to a note you wrote. Only an accept by you applies it.
    NeedsAccept,
}

/// Save a proposal as `_proposals/<id>.md`. The front matter holds the fields. The body holds the proposed note.
pub fn write_proposal(vault_root: &Path, p: &Proposal) -> Result<PathBuf> {
    check_proposal_id(p.id.as_str())?;
    let dir = vault_root.join(PROPOSAL_FOLDER);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.md", p.id.as_str()));
    std::fs::write(&path, proposal_text(p))?;
    Ok(path)
}

/// Every proposal file in `_proposals/`, sorted by file name. An absent folder gives none.
pub fn read_proposals(vault_root: &Path) -> Result<Vec<Proposal>> {
    let dir = vault_root.join(PROPOSAL_FOLDER);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if is_note_file(&path) {
            paths.push(path);
        }
    }
    paths.sort();
    paths.iter().map(|path| read_proposal(path)).collect()
}

/// Apply a proposal. With `auto`, a change to a note you wrote is refused with `NeedsAccept`.
pub fn apply(vault: &Vault, p: &Proposal, auto: bool) -> Result<Applied> {
    if auto && targets_user_note(vault, p)? {
        return Ok(Applied::NeedsAccept);
    }
    match p.kind {
        ProposalKind::Add => apply_add(vault, p),
        ProposalKind::Update => apply_update(vault, p),
        ProposalKind::Retire => apply_retire(vault, p),
        ProposalKind::Merge => apply_merge(vault, p),
    }
}

fn apply_add(vault: &Vault, p: &Proposal) -> Result<Applied> {
    let note = proposed_note(p)?;
    vault.write(note, None)?;
    Ok(Applied::Added(note.id.clone()))
}

/// Replace only the generated block of the target. Text you wrote outside the markers stays.
fn apply_update(vault: &Vault, p: &Proposal) -> Result<Applied> {
    let proposed = proposed_note(p)?;
    let mut current = target_note(vault, p)?;
    let new_text =
        generated_block(&proposed.body).map_or_else(|| proposed.body.clone(), str::to_string);
    current.body = replace_generated(&current.body, &new_text);
    vault.write(&current, None)?;
    Ok(Applied::Updated(current.id.clone()))
}

fn apply_retire(vault: &Vault, p: &Proposal) -> Result<Applied> {
    let mut current = target_note(vault, p)?;
    match current.lesson.as_mut() {
        Some(lesson) => lesson.state = LessonState::Retired,
        None => current.strength = strength::FLOOR,
    }
    vault.write(&current, None)?;
    Ok(Applied::Retired(current.id.clone()))
}

/// Write the merged note. The old note's file is left alone.
fn apply_merge(vault: &Vault, p: &Proposal) -> Result<Applied> {
    let merged = proposed_note(p)?;
    vault.write(merged, None)?;
    Ok(Applied::Merged(merged.id.clone()))
}

fn targets_user_note(vault: &Vault, p: &Proposal) -> Result<bool> {
    match &p.target {
        None => Ok(false),
        Some(id) => Ok(vault.read(id)?.is_some_and(|note| note.by == Author::You)),
    }
}

fn target_note(vault: &Vault, p: &Proposal) -> Result<Note> {
    let id = p
        .target
        .as_ref()
        .ok_or_else(|| BrainError::NotFound(format!("proposal {} has no target", p.id)))?;
    vault
        .read(id)?
        .ok_or_else(|| BrainError::NotFound(id.as_str().to_string()))
}

fn proposed_note(p: &Proposal) -> Result<&Note> {
    p.note.as_ref().ok_or_else(|| BrainError::Format {
        path: p.id.as_str().to_string(),
        reason: "proposal has no note".to_string(),
    })
}

fn proposal_text(p: &Proposal) -> String {
    let front = [
        format!("id: {}", p.id.as_str()),
        format!("kind: {}", kind_name(p.kind)),
        format!(
            "target: {}",
            quote_if_needed(p.target.as_ref().map_or("", NoteId::as_str))
        ),
        format!("state: {}", state_name(p.state)),
        format!("why: {}", quote_if_needed(&p.why)),
    ]
    .join("\n");
    let note_text = p.note.as_ref().map(render_note).unwrap_or_default();
    // The reader strips one final newline, so one extra newline keeps the note text whole.
    format!("---\n{front}\n---\n{note_text}\n")
}

fn read_proposal(path: &Path) -> Result<Proposal> {
    let label = path.to_string_lossy().into_owned();
    let text = std::fs::read_to_string(path)?;
    let (front, rest) =
        frontmatter::split(&text).ok_or_else(|| format_error(&label, "missing front matter"))?;
    let pairs = frontmatter::pairs(front).map_err(|reason| format_error(&label, reason))?;
    let get = |key: &str| field(&pairs, &label, key);

    let kind_raw = get("kind")?;
    let kind = parse_kind(kind_raw)
        .ok_or_else(|| format_error(&label, format!("unknown kind {kind_raw}")))?;
    let state_raw = get("state")?;
    let state = parse_state(state_raw)
        .ok_or_else(|| format_error(&label, format!("unknown state {state_raw}")))?;
    let target = match get("target")? {
        "" => None,
        raw => Some(NoteId::new(raw)),
    };
    let note = if rest.trim().is_empty() {
        None
    } else {
        Some(parse_note(rest, &label)?)
    };
    Ok(Proposal {
        id: NoteId::new(get("id")?),
        kind,
        target,
        why: get("why")?.to_string(),
        note,
        state,
    })
}

fn field<'a>(pairs: &'a Pairs, label: &str, key: &str) -> Result<&'a str> {
    pairs
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
        .ok_or_else(|| format_error(label, format!("missing {key}")))
}

fn check_proposal_id(id: &str) -> Result<()> {
    let bad = id.is_empty() || id.starts_with('.') || id.contains(['/', '\\']);
    if bad {
        return Err(format_error(id, "proposal id is not a safe file name"));
    }
    Ok(())
}

fn kind_name(kind: ProposalKind) -> &'static str {
    match kind {
        ProposalKind::Add => "add",
        ProposalKind::Update => "update",
        ProposalKind::Retire => "retire",
        ProposalKind::Merge => "merge",
    }
}

fn parse_kind(raw: &str) -> Option<ProposalKind> {
    [
        ProposalKind::Add,
        ProposalKind::Update,
        ProposalKind::Retire,
        ProposalKind::Merge,
    ]
    .into_iter()
    .find(|kind| kind_name(*kind) == raw)
}

fn state_name(state: ProposalState) -> &'static str {
    match state {
        ProposalState::Open => "open",
        ProposalState::Accepted => "accepted",
        ProposalState::Dismissed => "dismissed",
    }
}

fn parse_state(raw: &str) -> Option<ProposalState> {
    [
        ProposalState::Open,
        ProposalState::Accepted,
        ProposalState::Dismissed,
    ]
    .into_iter()
    .find(|state| state_name(*state) == raw)
}

fn format_error(path: &str, reason: impl Into<String>) -> BrainError {
    BrainError::Format {
        path: path.to_string(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER_TEXT: &str = "My own notes stay here.";

    fn convention(id: &str, by: Author, body: &str) -> Note {
        Note {
            id: NoteId::new(id),
            note_type: sentinel_core::NoteType::Convention,
            title: "Use fixed clocks".to_string(),
            tags: Vec::new(),
            by,
            strength: 0.5,
            confirmed: None,
            pinned: false,
            lesson: None,
            body: body.to_string(),
            extra: Vec::new(),
        }
    }

    fn update_proposal(target: &str, generated: &str) -> Proposal {
        Proposal {
            id: NoteId::new("p-update"),
            kind: ProposalKind::Update,
            target: Some(NoteId::new(target)),
            why: "refresh".to_string(),
            note: Some(convention(
                "proposed",
                Author::Care,
                &replace_generated("", generated),
            )),
            state: ProposalState::Accepted,
        }
    }

    fn open_vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = Vault::open(dir.path().join("brain")).expect("vault opens");
        (dir, vault)
    }

    #[test]
    fn a_note_you_wrote_never_changes_without_an_accept() {
        let (_dir, vault) = open_vault();
        let mine = convention("mine", Author::You, USER_TEXT);
        vault.write(&mine, None).expect("write mine");
        let before = vault.read(&mine.id).expect("read").expect("exists");

        let result = apply(&vault, &update_proposal("mine", "agent text"), true);
        assert_eq!(result.expect("apply runs"), Applied::NeedsAccept);
        assert_eq!(vault.read(&mine.id).expect("read").expect("exists"), before);
    }

    #[test]
    fn accepted_update_keeps_user_text_outside_the_markers() {
        let (_dir, vault) = open_vault();
        let body = format!("{USER_TEXT}\n\n{}", replace_generated("", "old agent text"));
        vault
            .write(&convention("shared", Author::Care, &body), None)
            .expect("write");

        let result = apply(&vault, &update_proposal("shared", "new agent text"), false);
        assert_eq!(
            result.expect("apply runs"),
            Applied::Updated(NoteId::new("shared"))
        );

        let stored = vault
            .read(&NoteId::new("shared"))
            .expect("read")
            .expect("exists");
        assert!(stored.body.contains(USER_TEXT));
        assert!(stored.body.contains("new agent text"));
        assert!(!stored.body.contains("old agent text"));
    }

    #[test]
    fn retire_moves_a_plain_note_to_the_strength_floor() {
        let (_dir, vault) = open_vault();
        vault
            .write(&convention("weak", Author::Run, ""), None)
            .expect("write");
        let proposal = Proposal {
            id: NoteId::new("p-retire"),
            kind: ProposalKind::Retire,
            target: Some(NoteId::new("weak")),
            why: "unused".to_string(),
            note: None,
            state: ProposalState::Accepted,
        };
        let applied = apply(&vault, &proposal, false).expect("apply runs");
        assert_eq!(applied, Applied::Retired(NoteId::new("weak")));
        let stored = vault
            .read(&NoteId::new("weak"))
            .expect("read")
            .expect("exists");
        assert!((stored.strength - strength::FLOOR).abs() < 1e-6);
    }

    #[test]
    fn merge_writes_the_new_note_and_keeps_the_old_one() {
        let (_dir, vault) = open_vault();
        vault
            .write(&convention("old", Author::Run, "old text"), None)
            .expect("write");
        let proposal = Proposal {
            id: NoteId::new("p-merge"),
            kind: ProposalKind::Merge,
            target: Some(NoteId::new("old")),
            why: "duplicate".to_string(),
            note: Some(convention("merged", Author::Care, "merged text")),
            state: ProposalState::Accepted,
        };
        let applied = apply(&vault, &proposal, false).expect("apply runs");
        assert_eq!(applied, Applied::Merged(NoteId::new("merged")));
        assert!(vault.read(&NoteId::new("old")).expect("read").is_some());
        assert!(vault.read(&NoteId::new("merged")).expect("read").is_some());
    }

    #[test]
    fn proposal_round_trips_through_its_file() {
        let (dir, _vault) = open_vault();
        let proposal = update_proposal("shared", "text: with colon");
        let path = write_proposal(dir.path().join("brain").as_path(), &proposal).expect("write");
        assert!(path.ends_with("_proposals/p-update.md"));

        let read = read_proposals(dir.path().join("brain").as_path()).expect("read");
        assert_eq!(read, vec![proposal]);
    }

    #[test]
    fn proposal_without_note_or_target_round_trips() {
        let (dir, _vault) = open_vault();
        let root = dir.path().join("brain");
        let proposal = Proposal {
            id: NoteId::new("p-add-empty"),
            kind: ProposalKind::Add,
            target: None,
            why: String::new(),
            note: None,
            state: ProposalState::Dismissed,
        };
        write_proposal(&root, &proposal).expect("write");
        assert_eq!(read_proposals(&root).expect("read"), vec![proposal]);
    }

    #[test]
    fn add_without_note_is_a_format_error() {
        let (_dir, vault) = open_vault();
        let proposal = Proposal {
            id: NoteId::new("p-empty"),
            kind: ProposalKind::Add,
            target: None,
            why: String::new(),
            note: None,
            state: ProposalState::Accepted,
        };
        assert!(matches!(
            apply(&vault, &proposal, false),
            Err(BrainError::Format { .. })
        ));
    }
}
