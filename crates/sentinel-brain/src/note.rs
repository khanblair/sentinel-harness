//! Note format: front matter, body, and the type vocabulary of the Brain vault.

use std::fmt::Display;
use std::str::FromStr;

use sentinel_core::{LessonState, NoteId, NoteType};

use crate::body;
use crate::error::{BrainError, Result};
use crate::frontmatter::{self, Pairs};

/// Every note type, in folder order.
pub const NOTE_TYPES: [NoteType; 9] = [
    NoteType::Convention,
    NoteType::Decision,
    NoteType::Fix,
    NoteType::File,
    NoteType::Task,
    NoteType::Run,
    NoteType::Lesson,
    NoteType::Glossary,
    NoteType::HowTo,
];

/// Front matter keys with a fixed meaning. Every other key is kept as `extra`.
const KNOWN_KEYS: [&str; 15] = [
    "id",
    "type",
    "title",
    "tags",
    "by",
    "strength",
    "confirmed",
    "pinned",
    "kind",
    "key",
    "value",
    "evidence",
    "helped",
    "hurt",
    "state",
];

/// Author of a note, matching the `by:` front matter field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Author {
    You,
    Run,
    Care,
}

impl Author {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "you" => Some(Author::You),
            "run" => Some(Author::Run),
            "care" => Some(Author::Care),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Author::You => "you",
            Author::Run => "run",
            Author::Care => "care",
        }
    }
}

/// Typed fields of a lesson note. A lesson holds no free text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonFields {
    pub kind: String,
    pub key: String,
    pub value: String,
    pub evidence: u32,
    pub helped: u32,
    pub hurt: u32,
    pub state: LessonState,
}

/// One Brain note: front matter fields plus the markdown body.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub id: NoteId,
    pub note_type: NoteType,
    pub title: String,
    pub tags: Vec<String>,
    pub by: Author,
    pub strength: f32,
    /// Date the note was last confirmed, `YYYY-MM-DD`.
    pub confirmed: Option<String>,
    pub pinned: bool,
    pub lesson: Option<LessonFields>,
    pub body: String,
    /// Front matter keys this crate does not know, kept in file order.
    pub extra: Vec<(String, String)>,
}

/// Parse a note from its markdown text. `path` only labels errors.
pub fn parse_note(text: &str, path: &str) -> Result<Note> {
    let (front, body_text) =
        frontmatter::split(text).ok_or_else(|| format_error(path, "missing front matter"))?;
    let pairs = frontmatter::pairs(front).map_err(|reason| format_error(path, reason))?;
    let fields = Fields { path, pairs };

    let note_type = fields.note_type()?;
    let strength: f32 = fields.parse_or("strength", 0.5)?;
    if !(0.0..=1.0).contains(&strength) {
        return Err(fields.error("strength outside 0 to 1"));
    }
    let lesson = if note_type == NoteType::Lesson {
        Some(parse_lesson(&fields)?)
    } else {
        None
    };
    if lesson.is_some() && body::has_free_text(body_text) {
        return Err(fields.error("lesson note holds free text"));
    }

    Ok(Note {
        id: NoteId::new(fields.required("id")?),
        note_type,
        title: fields.required("title")?.to_string(),
        tags: parse_tags(fields.get("tags")),
        by: fields.author()?,
        strength,
        confirmed: fields.get("confirmed").map(str::to_string),
        pinned: fields.parse_or("pinned", false)?,
        lesson,
        body: body_text.to_string(),
        extra: fields.extra(),
    })
}

/// Render a note to markdown with a stable field order.
pub fn render_note(note: &Note) -> String {
    let mut lines = vec![
        line("id", note.id.as_str()),
        line("type", note_type_name(note.note_type)),
        line("title", &frontmatter::quote_if_needed(&note.title)),
        line("tags", &format!("[{}]", note.tags.join(", "))),
        line("by", note.by.as_str()),
        line("strength", &note.strength.to_string()),
    ];
    if let Some(date) = &note.confirmed {
        lines.push(line("confirmed", &frontmatter::quote_if_needed(date)));
    }
    lines.push(line("pinned", &note.pinned.to_string()));
    if let Some(lesson) = &note.lesson {
        lines.extend(lesson_lines(lesson));
    }
    lines.extend(
        note.extra
            .iter()
            .map(|(key, value)| line(key, &frontmatter::quote_if_needed(value))),
    );

    let mut out = format!("---\n{}\n---\n", lines.join("\n"));
    if !note.body.is_empty() {
        out.push_str(&note.body);
        out.push('\n');
    }
    out
}

/// Reject a note that cannot be written and read back: lesson fields must match the type.
pub fn check_note(note: &Note) -> Result<()> {
    let is_lesson = note.note_type == NoteType::Lesson;
    if is_lesson != note.lesson.is_some() {
        return Err(BrainError::Format {
            path: note.id.as_str().to_string(),
            reason: "lesson fields must match the lesson type".to_string(),
        });
    }
    if is_lesson && body::has_free_text(&note.body) {
        return Err(BrainError::Format {
            path: note.id.as_str().to_string(),
            reason: "lesson note holds free text".to_string(),
        });
    }
    Ok(())
}

/// Folder name for a note type.
pub fn note_type_dir(note_type: NoteType) -> &'static str {
    match note_type {
        NoteType::Convention => "conventions",
        NoteType::Decision => "decisions",
        NoteType::Fix => "fixes",
        NoteType::File => "files",
        NoteType::Task => "tasks",
        NoteType::Run => "runs",
        NoteType::Lesson => "lessons",
        NoteType::Glossary => "glossary",
        NoteType::HowTo => "how-to",
    }
}

/// Front matter name for a note type. Matches the folder name.
pub fn note_type_name(note_type: NoteType) -> &'static str {
    match note_type {
        NoteType::Convention => "convention",
        NoteType::Decision => "decision",
        NoteType::Fix => "fix",
        NoteType::File => "file",
        NoteType::Task => "task",
        NoteType::Run => "run",
        NoteType::Lesson => "lesson",
        NoteType::Glossary => "glossary",
        NoteType::HowTo => "how-to",
    }
}

/// Parse a front matter type name.
pub fn parse_note_type(raw: &str) -> Option<NoteType> {
    NOTE_TYPES.into_iter().find(|t| note_type_name(*t) == raw)
}

/// Front matter name for a lesson state.
pub fn lesson_state_name(state: LessonState) -> &'static str {
    match state {
        LessonState::Candidate => "candidate",
        LessonState::Proven => "proven",
        LessonState::Rejected => "rejected",
        LessonState::Retired => "retired",
    }
}

fn parse_lesson_state(raw: &str) -> Option<LessonState> {
    [
        LessonState::Candidate,
        LessonState::Proven,
        LessonState::Rejected,
        LessonState::Retired,
    ]
    .into_iter()
    .find(|s| lesson_state_name(*s) == raw)
}

/// Front matter fields of one note, with the path used in errors.
struct Fields<'a> {
    path: &'a str,
    pairs: Pairs,
}

impl Fields<'_> {
    fn error(&self, reason: impl Into<String>) -> BrainError {
        format_error(self.path, reason)
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    fn required(&self, key: &str) -> Result<&str> {
        self.get(key)
            .ok_or_else(|| self.error(format!("missing {key}")))
    }

    fn parse_or<T>(&self, key: &str, default: T) -> Result<T>
    where
        T: FromStr,
        T::Err: Display,
    {
        match self.get(key) {
            None => Ok(default),
            Some(raw) => raw
                .parse()
                .map_err(|_| self.error(format!("bad {key}: {raw}"))),
        }
    }

    fn count(&self, key: &str) -> Result<u32> {
        let raw = self.required(key)?;
        raw.parse()
            .map_err(|_| self.error(format!("bad {key}: {raw}")))
    }

    fn note_type(&self) -> Result<NoteType> {
        let raw = self.required("type")?;
        parse_note_type(raw).ok_or_else(|| self.error(format!("unknown type {raw}")))
    }

    fn author(&self) -> Result<Author> {
        match self.get("by") {
            None => Ok(Author::You),
            Some(raw) => Author::parse(raw).ok_or_else(|| self.error(format!("unknown by {raw}"))),
        }
    }

    fn extra(&self) -> Vec<(String, String)> {
        self.pairs
            .iter()
            .filter(|(key, _)| !KNOWN_KEYS.contains(&key.as_str()))
            .cloned()
            .collect()
    }
}

fn parse_lesson(fields: &Fields<'_>) -> Result<LessonFields> {
    let raw_state = fields.required("state")?;
    let state = parse_lesson_state(raw_state)
        .ok_or_else(|| fields.error(format!("unknown state {raw_state}")))?;
    Ok(LessonFields {
        kind: fields.required("kind")?.to_string(),
        key: fields.required("key")?.to_string(),
        value: fields.required("value")?.to_string(),
        evidence: fields.count("evidence")?,
        helped: fields.count("helped")?,
        hurt: fields.count("hurt")?,
        state,
    })
}

fn lesson_lines(lesson: &LessonFields) -> Vec<String> {
    vec![
        line("kind", &frontmatter::quote_if_needed(&lesson.kind)),
        line("key", &frontmatter::quote_if_needed(&lesson.key)),
        line("value", &frontmatter::quote_if_needed(&lesson.value)),
        line("evidence", &lesson.evidence.to_string()),
        line("helped", &lesson.helped.to_string()),
        line("hurt", &lesson.hurt.to_string()),
        line("state", lesson_state_name(lesson.state)),
    ]
}

fn parse_tags(raw: Option<&str>) -> Vec<String> {
    let Some(inner) = raw
        .map(str::trim)
        .and_then(|s| s.strip_prefix('['))
        .and_then(|s| s.strip_suffix(']'))
    else {
        return Vec::new();
    };
    inner
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}

fn line(key: &str, value: &str) -> String {
    format!("{key}: {value}")
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

    #[test]
    fn parse_tags_reads_bracket_list() {
        assert_eq!(parse_tags(Some("[rust, errors]")), vec!["rust", "errors"]);
        assert_eq!(parse_tags(Some("[]")), Vec::<String>::new());
        assert_eq!(parse_tags(None), Vec::<String>::new());
    }

    #[test]
    fn note_type_names_round_trip() {
        for t in NOTE_TYPES {
            assert_eq!(parse_note_type(note_type_name(t)), Some(t));
        }
        assert_eq!(parse_note_type("unknown"), None);
    }
}
