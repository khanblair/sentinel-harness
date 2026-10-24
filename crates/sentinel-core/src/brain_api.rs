//! Request and response bodies of the Brain routes (`docs/architecture.md`, API). The daemon
//! fills them from the vault and the index, and the Brain panel reads them.

use serde::{Deserialize, Serialize};

use crate::ids::NoteId;
use crate::note::{BrainMode, LessonState, NoteType, ProposalKind};

/// One row of the notes list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteSummary {
    pub id: NoteId,
    pub note_type: NoteType,
    pub title: String,
    pub tags: Vec<String>,
    /// `you`, `run`, or `care`.
    pub by: String,
    /// 0 to 1.
    pub strength: f32,
    /// A pinned note is always loaded for agents.
    pub pinned: bool,
    /// The state of a lesson note, none for other notes.
    pub lesson_state: Option<LessonState>,
}

/// A link from or to a note. A dead link names a note that does not exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteLink {
    pub id: NoteId,
    pub title: String,
    pub dead: bool,
}

/// The typed fields of a lesson.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LessonView {
    pub kind: String,
    pub key: String,
    pub value: String,
    pub evidence: u32,
    pub helped: u32,
    pub hurt: u32,
}

/// One note with its text and its links.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteDetail {
    pub summary: NoteSummary,
    pub body: String,
    /// The `confirmed` date, `YYYY-MM-DD`, when there is one.
    pub confirmed: Option<String>,
    pub uses: u32,
    pub links: Vec<NoteLink>,
    pub backlinks: Vec<NoteLink>,
    pub lesson: Option<LessonView>,
    /// Identifies the version that was read. Send it back with an edit so the daemon can refuse
    /// an edit that would overwrite a newer change.
    pub version: String,
}

/// `PUT /api/brain/notes/:id`. The computer only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveNote {
    pub body: String,
    pub version: String,
}

/// `POST /api/brain/notes`. A new note you write. The computer only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewNote {
    pub note_type: NoteType,
    pub title: String,
    pub body: String,
}

/// `POST /api/brain/notes/:id/pin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinNote {
    pub pinned: bool,
}

/// `GET /api/brain/notes?q=&type=`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteList {
    pub notes: Vec<NoteSummary>,
    /// The folder of the vault, shown in the panel.
    pub vault_path: String,
    pub mode: BrainMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNodeView {
    pub id: NoteId,
    pub title: String,
    pub note_type: NoteType,
    /// How many notes link to this one. The dot grows with it.
    pub backlinks: u32,
}

/// `GET /api/brain/graph`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphView {
    pub nodes: Vec<GraphNodeView>,
    pub links: Vec<(NoteId, NoteId)>,
}

/// What a person can do to a lesson (`POST /api/brain/lessons/:id/review`). A phone may do all
/// three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LessonReview {
    Approve,
    Reject,
    Retire,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewLesson {
    pub action: LessonReview,
}

/// One proposal from a care run (`GET /api/brain/proposals`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalView {
    pub id: String,
    pub kind: ProposalKind,
    pub target: Option<NoteId>,
    pub title: String,
    pub why: String,
    /// True when the target is a note you wrote, so only your accept can apply it.
    pub needs_accept: bool,
}

/// `POST /api/brain/proposals/:id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnswerProposal {
    pub accept: bool,
}

/// How often a care run starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "every", content = "runs")]
pub enum CareFrequency {
    Off,
    Daily,
    Weekly,
    Monthly,
    AfterRuns(u32),
}

/// `GET` and `PUT /api/brain/care`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CareSettings {
    pub frequency: CareFrequency,
    /// Dollars as text. The care run stops at this spend.
    pub budget: String,
    /// Apply proposals to notes Sentinel wrote without asking. Never to a note you wrote.
    pub auto_apply: bool,
    /// Date of the last care run, `YYYY-MM-DD`.
    pub last_run: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn care_frequency_serializes_with_a_tag() {
        let json = serde_json::to_string(&CareFrequency::AfterRuns(5)).unwrap();
        assert_eq!(json, r#"{"every":"after_runs","runs":5}"#);
        let weekly = serde_json::to_string(&CareFrequency::Weekly).unwrap();
        assert_eq!(weekly, r#"{"every":"weekly"}"#);
    }

    #[test]
    fn a_lesson_review_uses_snake_case() {
        let json = serde_json::to_string(&ReviewLesson {
            action: LessonReview::Approve,
        })
        .unwrap();
        assert_eq!(json, r#"{"action":"approve"}"#);
    }
}
