//! Lessons: typed notes that record what helped or hurt an agent. A lesson holds no free text.

use sentinel_core::{LessonState, NoteId, NoteType};

use crate::error::{BrainError, Result};
use crate::note::{Author, LessonFields, Note};
use crate::schedule::Date;
use crate::strength;

/// A proven lesson unused this many days is retired.
pub const RETIRE_AFTER_DAYS: u32 = 90;
const HELPED_TO_PROVE: u32 = 2;
const MAX_TEXT_CHARS: usize = 120;

/// What one use of a lesson showed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Helped,
    Hurt,
    Neutral,
}

/// A new candidate lesson. Its id is derived from the kind and key, so the same lesson gets the same note.
pub fn new_lesson(kind: &str, key: &str, value: &str, now: Date) -> Note {
    Note {
        id: lesson_id(kind, key),
        note_type: NoteType::Lesson,
        title: format!("{kind} {key}"),
        tags: Vec::new(),
        by: Author::Run,
        strength: strength::INITIAL,
        confirmed: Some(now.to_string()),
        pinned: false,
        lesson: Some(LessonFields {
            kind: kind.to_string(),
            key: key.to_string(),
            value: value.to_string(),
            evidence: 0,
            helped: 0,
            hurt: 0,
            state: LessonState::Candidate,
        }),
        body: String::new(),
        extra: Vec::new(),
    }
}

/// Count one use and move the state. Rejected and retired lessons do not change by use.
pub fn record_use(note: &mut Note, outcome: Outcome) {
    let Some(lesson) = note.lesson.as_mut() else {
        return;
    };
    match outcome {
        Outcome::Helped => lesson.helped = lesson.helped.saturating_add(1),
        Outcome::Hurt => lesson.hurt = lesson.hurt.saturating_add(1),
        Outcome::Neutral => {}
    }
    lesson.evidence = lesson.evidence.saturating_add(1);
    lesson.state = next_state(lesson, outcome);
}

/// Retire a proven lesson that has gone unused for `limit_days`. Returns true when it retired.
pub fn retire_if_unused(note: &mut Note, days_unused: u32, limit_days: u32) -> bool {
    let Some(lesson) = note.lesson.as_mut() else {
        return false;
    };
    if lesson.state == LessonState::Proven && days_unused >= limit_days {
        lesson.state = LessonState::Retired;
        return true;
    }
    false
}

/// Fail when a lesson text holds a task title or runs past 120 characters.
pub fn forbid_task_text(value: &str, task_titles: &[String]) -> Result<()> {
    if value.chars().count() > MAX_TEXT_CHARS {
        return Err(lesson_error("lesson text is longer than 120 characters"));
    }
    let lowered = value.to_lowercase();
    let holds_title = task_titles
        .iter()
        .map(|title| title.trim().to_lowercase())
        .filter(|title| !title.is_empty())
        .any(|title| lowered.contains(&title));
    if holds_title {
        return Err(lesson_error("lesson text holds a task title"));
    }
    Ok(())
}

/// Run `forbid_task_text` on every typed field of a lesson.
pub fn check_lesson_text(lesson: &LessonFields, task_titles: &[String]) -> Result<()> {
    [&lesson.kind, &lesson.key, &lesson.value]
        .into_iter()
        .try_for_each(|text| forbid_task_text(text, task_titles))
}

fn next_state(lesson: &LessonFields, outcome: Outcome) -> LessonState {
    match lesson.state {
        LessonState::Rejected | LessonState::Retired => lesson.state,
        LessonState::Candidate | LessonState::Proven => {
            if outcome == Outcome::Hurt {
                LessonState::Rejected
            } else if lesson.helped >= HELPED_TO_PROVE && lesson.hurt == 0 {
                LessonState::Proven
            } else {
                lesson.state
            }
        }
    }
}

fn lesson_id(kind: &str, key: &str) -> NoteId {
    NoteId::new(format!("lesson-{}-{}", slug(kind), slug(key)))
}

/// Lowercase ASCII words joined by `-`. Other characters become separators.
fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars().map(|c| c.to_ascii_lowercase()) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn lesson_error(reason: &str) -> BrainError {
    BrainError::Format {
        path: "lesson".to_string(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> Date {
        Date::new(2026, 10, 24).expect("valid date")
    }

    fn candidate() -> Note {
        new_lesson("pattern", "retry-timeout", "raise timeout to 30s", now())
    }

    fn state_of(note: &Note) -> LessonState {
        note.lesson.as_ref().expect("lesson fields").state
    }

    #[test]
    fn new_lesson_starts_as_candidate_with_no_evidence() {
        let note = candidate();
        let lesson = note.lesson.as_ref().expect("lesson fields");
        assert_eq!(lesson.state, LessonState::Candidate);
        assert_eq!(lesson.evidence, 0);
        assert_eq!(note.body, "");
        assert_eq!(note.id.as_str(), "lesson-pattern-retry-timeout");
    }

    #[test]
    fn two_helped_uses_prove_a_candidate() {
        let mut note = candidate();
        record_use(&mut note, Outcome::Helped);
        assert_eq!(state_of(&note), LessonState::Candidate);
        record_use(&mut note, Outcome::Helped);
        assert_eq!(state_of(&note), LessonState::Proven);
    }

    #[test]
    fn neutral_use_counts_as_evidence_only() {
        let mut note = candidate();
        record_use(&mut note, Outcome::Neutral);
        let lesson = note.lesson.as_ref().expect("lesson fields");
        assert_eq!((lesson.evidence, lesson.helped, lesson.hurt), (1, 0, 0));
        assert_eq!(lesson.state, LessonState::Candidate);
    }

    #[test]
    fn a_harmful_use_rejects_a_candidate_or_proven_lesson() {
        let mut candidate_note = candidate();
        record_use(&mut candidate_note, Outcome::Hurt);
        assert_eq!(state_of(&candidate_note), LessonState::Rejected);

        let mut proven = candidate();
        record_use(&mut proven, Outcome::Helped);
        record_use(&mut proven, Outcome::Helped);
        record_use(&mut proven, Outcome::Hurt);
        assert_eq!(state_of(&proven), LessonState::Rejected);
    }

    #[test]
    fn rejected_lesson_does_not_recover_by_use() {
        let mut note = candidate();
        record_use(&mut note, Outcome::Hurt);
        record_use(&mut note, Outcome::Helped);
        record_use(&mut note, Outcome::Helped);
        assert_eq!(state_of(&note), LessonState::Rejected);
    }

    #[test]
    fn unused_proven_lesson_retires_after_the_limit() {
        let mut note = candidate();
        record_use(&mut note, Outcome::Helped);
        record_use(&mut note, Outcome::Helped);
        assert!(!retire_if_unused(
            &mut note,
            RETIRE_AFTER_DAYS - 1,
            RETIRE_AFTER_DAYS
        ));
        assert!(retire_if_unused(
            &mut note,
            RETIRE_AFTER_DAYS,
            RETIRE_AFTER_DAYS
        ));
        assert_eq!(state_of(&note), LessonState::Retired);
    }

    #[test]
    fn candidate_is_not_retired_by_disuse() {
        let mut note = candidate();
        assert!(!retire_if_unused(&mut note, 365, RETIRE_AFTER_DAYS));
        assert_eq!(state_of(&note), LessonState::Candidate);
    }

    #[test]
    fn forbid_rejects_a_seeded_task_title() {
        let titles = vec!["Fix the login race in auth".to_string()];
        let value = "Fix the login race in auth worked well";
        assert!(forbid_task_text(value, &titles).is_err());
        assert!(forbid_task_text("keep retries short", &titles).is_ok());
    }

    #[test]
    fn forbid_matches_titles_ignoring_case() {
        let titles = vec!["Rewrite Parser".to_string()];
        assert!(forbid_task_text("we did a rewrite parser pass", &titles).is_err());
    }

    #[test]
    fn forbid_rejects_text_over_120_characters() {
        let long = "x".repeat(121);
        assert!(forbid_task_text(&long, &[]).is_err());
        assert!(forbid_task_text(&"x".repeat(120), &[]).is_ok());
    }

    #[test]
    fn check_lesson_text_covers_kind_key_and_value() {
        let titles = vec!["Seeded Title".to_string()];
        let note = new_lesson("pattern", "seeded title", "safe value", now());
        let lesson = note.lesson.expect("lesson fields");
        assert!(check_lesson_text(&lesson, &titles).is_err());
    }
}
