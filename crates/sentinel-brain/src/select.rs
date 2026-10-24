//! Agent memory: pick the Brain notes for one task within a token cap.

use sentinel_core::NoteId;

use crate::error::{BrainError, Result};
use crate::index::Index;
use crate::note::Note;
use crate::vault::Vault;

pub const DEFAULT_TOKEN_CAP: usize = 500;
const SEARCH_LIMIT: u32 = 20;
const HEADING: &str = "## Project memory";
const CHARS_PER_TOKEN: usize = 4;

/// What a task asks for. `query` is the search text, empty for none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectRequest {
    pub task_files: Vec<String>,
    pub query: String,
    pub token_cap: usize,
}

impl Default for SelectRequest {
    fn default() -> Self {
        SelectRequest {
            task_files: Vec::new(),
            query: String::new(),
            token_cap: DEFAULT_TOKEN_CAP,
        }
    }
}

/// Why a note was loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    Pinned,
    Lesson,
    LinkedFile,
    SearchHit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedNote {
    pub id: NoteId,
    pub title: String,
    pub reason: Why,
}

/// The notes chosen for a task. `tokens` is the estimated size of the prompt text.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Selection {
    pub notes: Vec<SelectedNote>,
    pub tokens: usize,
    /// Notes that did not fit under the cap.
    pub skipped: usize,
}

struct Candidate {
    note: Note,
    why: Why,
}

/// Pick notes in order: pinned, proven lessons, linked to the task's files, search hits.
/// Each group goes strongest first. Notes are added while they fit under the cap.
pub fn select_notes(index: &Index, vault: &Vault, request: &SelectRequest) -> Result<Selection> {
    let candidates = gather(index, vault, request)?;
    Ok(pack(candidates, request.token_cap))
}

fn gather(index: &Index, vault: &Vault, request: &SelectRequest) -> Result<Vec<Candidate>> {
    let mut out = Vec::new();
    add_group(vault, index.pinned()?, Why::Pinned, &mut out)?;
    add_group(vault, proven_lesson_ids(index)?, Why::Lesson, &mut out)?;

    let mut linked = Vec::new();
    for path in &request.task_files {
        linked.extend(index.notes_linking_to_file(path)?);
    }
    add_group(vault, linked, Why::LinkedFile, &mut out)?;

    if !request.query.trim().is_empty() {
        let hits = index.search(&request.query, None, SEARCH_LIMIT)?;
        let ids = hits.into_iter().map(|hit| hit.id).collect();
        add_group(vault, ids, Why::SearchHit, &mut out)?;
    }
    Ok(out)
}

/// Load one group, skip what is already known, and append it strongest first.
fn add_group(vault: &Vault, ids: Vec<NoteId>, why: Why, out: &mut Vec<Candidate>) -> Result<()> {
    let mut group: Vec<Note> = Vec::new();
    for id in ids {
        if is_known(out, &group, &id) {
            continue;
        }
        if let Some(note) = vault.read(&id)? {
            group.push(note);
        }
    }
    group.sort_by(|a, b| {
        b.strength
            .total_cmp(&a.strength)
            .then_with(|| a.id.cmp(&b.id))
    });
    out.extend(group.into_iter().map(|note| Candidate { note, why }));
    Ok(())
}

fn is_known(out: &[Candidate], group: &[Note], id: &NoteId) -> bool {
    out.iter().any(|candidate| candidate.note.id == *id) || group.iter().any(|note| note.id == *id)
}

fn proven_lesson_ids(index: &Index) -> Result<Vec<NoteId>> {
    let mut statement = index
        .conn
        .prepare("SELECT id FROM notes WHERE state = 'proven' ORDER BY id")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    rows.map(|row| row.map(NoteId::new).map_err(BrainError::from))
        .collect()
}

/// Take candidates while the prompt text stays under the cap. Skip the rest and count them.
fn pack(candidates: Vec<Candidate>, cap: usize) -> Selection {
    let mut selection = Selection::default();
    let mut blocks: Vec<String> = Vec::new();
    for candidate in candidates {
        blocks.push(block_text(&candidate.note));
        if estimate_tokens(&compose(&blocks)) <= cap {
            selection.notes.push(SelectedNote {
                id: candidate.note.id.clone(),
                title: candidate.note.title.clone(),
                reason: candidate.why,
            });
        } else {
            blocks.pop();
            selection.skipped += 1;
        }
    }
    selection.tokens = estimate_tokens(&compose(&blocks));
    selection
}

impl Selection {
    /// The line the stream shows, for example `Brain loaded 3 notes: 1 pinned, 1 search hit.`
    pub fn summary_line(&self) -> String {
        if self.notes.is_empty() {
            return "Brain loaded no notes.".to_string();
        }
        let parts: Vec<String> = GROUP_LABELS
            .iter()
            .filter_map(|(why, one, many)| {
                let count = self.notes.iter().filter(|note| note.reason == *why).count();
                (count > 0).then(|| format!("{count} {}", if count == 1 { one } else { many }))
            })
            .collect();
        let total = self.notes.len();
        let noun = if total == 1 { "note" } else { "notes" };
        format!("Brain loaded {total} {noun}: {}.", parts.join(", "))
    }

    /// The `## Project memory` section. Notes that can no longer be read are left out.
    pub fn prompt_text(&self, vault: &Vault) -> String {
        let blocks: Vec<String> = self
            .notes
            .iter()
            .filter_map(|selected| vault.read(&selected.id).ok().flatten())
            .map(|note| block_text(&note))
            .collect();
        compose(&blocks)
    }
}

const GROUP_LABELS: [(Why, &str, &str); 4] = [
    (Why::Pinned, "pinned", "pinned"),
    (Why::Lesson, "lesson", "lessons"),
    (Why::LinkedFile, "about these files", "about these files"),
    (Why::SearchHit, "search hit", "search hits"),
];

/// One lesson is one line from its typed fields. Other notes keep their title and body.
fn block_text(note: &Note) -> String {
    match &note.lesson {
        Some(lesson) => format!("{} {}: {}", lesson.kind, lesson.key, lesson.value),
        None => format!("### {}\n{}", note.title, without_markers(&note.body).trim()),
    }
}

/// Drop the sentinel marker lines. The text between them stays.
fn without_markers(body: &str) -> String {
    body.lines()
        .filter(|line| !line.trim().starts_with("<!-- sentinel:"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn compose(blocks: &[String]) -> String {
    if blocks.is_empty() {
        return String::new();
    }
    format!("{HEADING}\n\n{}\n", blocks.join("\n\n"))
}

fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(CHARS_PER_TOKEN)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selected(reason: Why) -> SelectedNote {
        SelectedNote {
            id: NoteId::new("n"),
            title: "n".to_string(),
            reason,
        }
    }

    fn selection_of(reasons: &[Why]) -> Selection {
        Selection {
            notes: reasons.iter().map(|why| selected(*why)).collect(),
            tokens: 0,
            skipped: 0,
        }
    }

    #[test]
    fn summary_says_no_notes_when_empty() {
        assert_eq!(
            Selection::default().summary_line(),
            "Brain loaded no notes."
        );
    }

    #[test]
    fn summary_counts_each_reason() {
        let selection = selection_of(&[Why::Pinned, Why::LinkedFile, Why::SearchHit]);
        assert_eq!(
            selection.summary_line(),
            "Brain loaded 3 notes: 1 pinned, 1 about these files, 1 search hit."
        );
    }

    #[test]
    fn summary_pluralizes_lessons_and_search_hits() {
        let selection = selection_of(&[Why::Lesson, Why::Lesson, Why::SearchHit, Why::SearchHit]);
        assert_eq!(
            selection.summary_line(),
            "Brain loaded 4 notes: 2 lessons, 2 search hits."
        );
    }

    #[test]
    fn pack_stops_adding_when_the_cap_is_full() {
        let big = |id: &str| Candidate {
            note: test_note(id, &"word ".repeat(100)),
            why: Why::SearchHit,
        };
        let selection = pack(vec![big("a"), big("b"), big("c")], 150);
        assert!(selection.notes.len() < 3);
        assert_eq!(selection.skipped, 3 - selection.notes.len());
        assert!(selection.tokens <= 150);
    }

    #[test]
    fn pack_keeps_a_smaller_note_after_a_big_one_is_skipped() {
        let big = Candidate {
            note: test_note("big", &"word ".repeat(400)),
            why: Why::Pinned,
        };
        let small = Candidate {
            note: test_note("small", "short"),
            why: Why::SearchHit,
        };
        let selection = pack(vec![big, small], 100);
        let ids: Vec<&str> = selection.notes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["small"]);
        assert_eq!(selection.skipped, 1);
    }

    #[test]
    fn lesson_block_is_one_typed_line() {
        let note = crate::lesson::new_lesson("pattern", "retry", "wait 2s", date());
        assert_eq!(block_text(&note), "pattern retry: wait 2s");
    }

    #[test]
    fn block_text_drops_markers_but_keeps_text() {
        let body = "<!-- sentinel:start -->\ngenerated\n<!-- sentinel:end -->";
        assert_eq!(without_markers(body), "generated");
    }

    #[test]
    fn compose_is_empty_without_blocks() {
        assert_eq!(compose(&[]), "");
        assert!(compose(&["one".to_string()]).starts_with(HEADING));
    }

    fn date() -> crate::schedule::Date {
        crate::schedule::Date::new(2026, 10, 24).expect("valid date")
    }

    fn test_note(id: &str, body: &str) -> Note {
        Note {
            id: NoteId::new(id),
            note_type: sentinel_core::NoteType::Convention,
            title: id.to_string(),
            tags: Vec::new(),
            by: crate::note::Author::Run,
            strength: 0.5,
            confirmed: None,
            pinned: false,
            lesson: None,
            body: body.to_string(),
            extra: Vec::new(),
        }
    }
}
