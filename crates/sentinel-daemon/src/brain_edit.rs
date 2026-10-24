//! Changes a person makes in the Brain panel. These ignore the Brain mode, which limits only what
//! Sentinel writes by itself. Writing or pinning a note is the computer only. Reviewing a lesson is
//! open to a phone too.

use sentinel_brain::note::{Author, Note};
use sentinel_brain::{Vault, strength};
use sentinel_core::brain_api::{
    LessonReview, NewNote, NoteDetail, PinNote, ReviewLesson, SaveNote,
};
use sentinel_core::{LessonState, NoteId, NoteType};

use crate::brain::{note_text, note_written, require_pc, slug, version_of, write_note};
use crate::brain_notes::{find_note, note_detail};
use crate::controls::Caller;
use crate::engine::{Engine, EngineError, Result};

const STALE_NOTE: &str = "This note changed. Reload it and try again.";

impl Engine {
    /// Replace the body of a note. Refused when the note changed since `save.version` was read.
    pub fn save_note(&self, id: &NoteId, save: SaveNote, caller: &Caller) -> Result<NoteDetail> {
        require_pc(caller)?;
        let state = self.brain()?;
        let mut note = find_note(&state, id)?;
        let current = note_text(&state.vault, &note);
        if version_of(&current) != save.version {
            return Err(EngineError::Conflict(STALE_NOTE.to_owned()));
        }
        note.body = save.body;
        note.by = Author::You;
        write_note(&state, &note, Some(Vault::content_hash(&current)))?;
        note_written(self, &note.id);
        note_detail(&state, &note)
    }

    /// Write a new note you wrote. Its id is a slug of the title.
    pub fn new_note(&self, new: NewNote, caller: &Caller) -> Result<NoteDetail> {
        require_pc(caller)?;
        if new.note_type == NoteType::Lesson {
            return Err(EngineError::Invalid(
                "Lessons come from runs. Write another kind of note.".to_owned(),
            ));
        }
        let title = new.title.trim().to_owned();
        let id = NoteId::new(slug(&title));
        if id.as_str().is_empty() {
            return Err(EngineError::Invalid(
                "A note needs a title with letters or digits.".to_owned(),
            ));
        }
        let state = self.brain()?;
        if state.vault.read(&id)?.is_some() {
            return Err(EngineError::Conflict(format!(
                "A note called {id} already exists."
            )));
        }
        let note = Note {
            id,
            note_type: new.note_type,
            title,
            tags: Vec::new(),
            by: Author::You,
            strength: strength::INITIAL,
            confirmed: None,
            pinned: false,
            lesson: None,
            body: new.body,
            extra: Vec::new(),
        };
        write_note(&state, &note, None)?;
        note_written(self, &note.id);
        note_detail(&state, &note)
    }

    /// Pin or unpin a note. A pinned note is always loaded for agents.
    pub fn pin_note(&self, id: &NoteId, pin: PinNote, caller: &Caller) -> Result<NoteDetail> {
        require_pc(caller)?;
        let state = self.brain()?;
        let mut note = find_note(&state, id)?;
        note.pinned = pin.pinned;
        write_note(&state, &note, None)?;
        note_written(self, &note.id);
        note_detail(&state, &note)
    }

    /// Approve, reject, or retire a lesson. Any caller may do this, a phone included.
    pub fn review_lesson(&self, id: &NoteId, review: ReviewLesson) -> Result<NoteDetail> {
        let state = self.brain()?;
        let mut note = find_note(&state, id)?;
        let target = lesson_target(review.action);
        let Some(lesson) = note.lesson.as_mut() else {
            return Err(EngineError::Invalid(
                "This note is not a lesson.".to_owned(),
            ));
        };
        lesson.state = target;
        write_note(&state, &note, None)?;
        self.emit_quiet(&sentinel_core::Event::LessonStateChanged {
            note: note.id.clone(),
            state: target,
        });
        note_detail(&state, &note)
    }
}

fn lesson_target(action: LessonReview) -> LessonState {
    match action {
        LessonReview::Approve => LessonState::Proven,
        LessonReview::Reject => LessonState::Rejected,
        LessonReview::Retire => LessonState::Retired,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approve_reject_and_retire_map_to_their_states() {
        assert_eq!(lesson_target(LessonReview::Approve), LessonState::Proven);
        assert_eq!(lesson_target(LessonReview::Reject), LessonState::Rejected);
        assert_eq!(lesson_target(LessonReview::Retire), LessonState::Retired);
    }
}
