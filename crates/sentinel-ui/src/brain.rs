//! Brain panel logic: link parsing, strength classes, the phone rule, the list load states, and the
//! empty-state choices. The labels and copy live in `brain_text.rs`. Plain functions only, so the
//! tests run on the host.

use std::fmt::Write;

use sentinel_core::brain_api::{GraphNodeView, LessonReview, NoteSummary};
use sentinel_core::{BrainMode, LessonState, NoteId, NoteType};

use crate::api::ApiFailure;
pub use crate::brain_text::{
    ASK_COMPUTER_BUILD_TEXT, BUILD_NOTES_LABEL, BrainTab, LEARN_SWITCH_TEXT, LOADING_GRAPH,
    LOADING_NOTE, LOADING_NOTES, LOADING_PROPOSALS, NO_LINKS_TEXT, NO_NOTES_MATCH, NO_NOTES_TEXT,
    NOTHING_TO_REVIEW_TEXT, READING_PROJECT_LABEL, RUN_CARE_LABEL, SWITCH_TO_LEARN_LABEL,
    answer_done_text, budget_error, care_done_text, lesson_state_color, lesson_state_label,
    links_text, mode_name, mode_sentence, new_note_error, note_meta_line, pin_done_text,
    proposal_kind_label, proposals_tab_label, review_done_text, review_label, seed_done_text,
    type_label,
};

/// Shown in place of a Brain view when the daemon has no Brain route for it yet.
pub const UNAVAILABLE: &str = "The Brain is not available yet in this build.";

/// Every note type, in the order the type filter and the graph legend use.
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

/// What a list or a note shows: a loading line until the first answer, then the rows, or the
/// text of the failure.
#[derive(Debug, Clone, PartialEq)]
pub enum LoadState<T> {
    Loading,
    Ready(T),
    Failed(String),
}

/// What an empty vault offers. A phone cannot write notes, so it is told to ask the computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyChoice {
    BuildNotes,
    AskComputer,
}

/// The color token of a type, the same colors as the prototype's `NT` table.
pub fn type_color_token(note_type: NoteType) -> &'static str {
    match note_type {
        NoteType::Convention | NoteType::HowTo => "info",
        NoteType::Decision => "accent",
        NoteType::Fix | NoteType::Lesson => "ok",
        NoteType::File | NoteType::Glossary => "muted",
        NoteType::Task => "lock",
        NoteType::Run => "warn",
    }
}

/// The value the daemon takes for `type=`, the same as the serde name.
pub fn type_query_value(note_type: NoteType) -> &'static str {
    match note_type {
        NoteType::Convention => "convention",
        NoteType::Decision => "decision",
        NoteType::Fix => "fix",
        NoteType::File => "file",
        NoteType::Task => "task",
        NoteType::Run => "run",
        NoteType::Lesson => "lesson",
        NoteType::Glossary => "glossary",
        NoteType::HowTo => "how_to",
    }
}

/// The type a filter value names, or `None` for "All types".
pub fn type_from_value(value: &str) -> Option<NoteType> {
    NOTE_TYPES
        .into_iter()
        .find(|note_type| type_query_value(*note_type) == value)
}

/// `/api/brain/notes` with the search text and the type filter. Empty search text is left out.
pub fn notes_path(query: &str, note_type: Option<NoteType>) -> String {
    let mut path = format!("/api/brain/notes?q={}", percent_encode(query.trim()));
    if let Some(note_type) = note_type {
        path.push_str("&type=");
        path.push_str(type_query_value(note_type));
    }
    path
}

/// Escapes text for a query string. Letters, digits, and `-_.~` pass through.
pub fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            // Writing to a String cannot fail, so the Result is dropped.
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

/// A strength bar class: low under 0.34, medium under 0.67, high from there.
pub fn strength_class(strength: f32) -> &'static str {
    if strength < 0.34 {
        "low"
    } else if strength < 0.67 {
        "medium"
    } else {
        "high"
    }
}

/// A paired phone can only tighten the mode. The computer may pick any mode, in any window width.
pub fn phone_may_pick(current: BrainMode, target: BrainMode, phone: bool) -> bool {
    !phone || current.phone_may_set(target)
}

/// Building notes writes them, which only learn mode allows. Any other mode switches first.
pub fn mode_needs_switch(mode: BrainMode) -> bool {
    mode != BrainMode::Learn
}

/// The notes tab offers to build the notes on the computer, and asks a phone to do it instead.
pub fn empty_notes_choice(phone: bool) -> EmptyChoice {
    if phone {
        EmptyChoice::AskComputer
    } else {
        EmptyChoice::BuildNotes
    }
}

/// Run care now is a computer action, the same as the care run itself.
pub fn care_offered(phone: bool) -> bool {
    !phone
}

/// A graph with no notes shows the empty text in place of the canvas.
pub fn graph_is_empty(nodes: &[GraphNodeView]) -> bool {
    nodes.is_empty()
}

/// No search text and no type filter means the list is the whole vault. Only then does an empty
/// list mean the vault is empty.
pub fn is_unfiltered(query: &str, filter: &str) -> bool {
    query.trim().is_empty() && filter.is_empty()
}

/// The links that touch one note, from either end.
pub fn link_count(links: &[(NoteId, NoteId)], id: &NoteId) -> usize {
    links
        .iter()
        .filter(|(from, to)| from == id || to == id)
        .count()
}

/// Pinned notes first. The sort is stable, so the other notes keep the daemon's order.
pub fn pinned_first(notes: &mut [NoteSummary]) {
    notes.sort_by_key(|note| !note.pinned);
}

/// True for the 404 a daemon without the Brain routes gives.
pub fn is_missing(failure: &ApiFailure) -> bool {
    matches!(failure, ApiFailure::Status { code: 404, .. })
}

/// Approve, Reject, and Retire each hide once the lesson is already in that state.
pub fn review_offered(state: LessonState, action: LessonReview) -> bool {
    match action {
        LessonReview::Approve => !matches!(state, LessonState::Proven | LessonState::Rejected),
        LessonReview::Reject => state != LessonState::Rejected,
        LessonReview::Retire => state != LessonState::Retired,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(id: &str, pinned: bool) -> NoteSummary {
        NoteSummary {
            id: NoteId::new(id),
            note_type: NoteType::Fix,
            title: id.to_owned(),
            tags: Vec::new(),
            by: "you".to_owned(),
            strength: 0.5,
            pinned,
            lesson_state: None,
        }
    }

    #[test]
    fn every_type_has_a_label_and_a_query_value() {
        assert_eq!(type_label(NoteType::HowTo), "How-to");
        assert_eq!(type_query_value(NoteType::HowTo), "how_to");
        assert_eq!(type_from_value("how_to"), Some(NoteType::HowTo));
        assert_eq!(type_from_value("all"), None);
    }

    #[test]
    fn a_search_path_leaves_out_an_empty_type() {
        assert_eq!(notes_path("  fees ", None), "/api/brain/notes?q=fees");
        assert_eq!(
            notes_path("", Some(NoteType::Lesson)),
            "/api/brain/notes?q=&type=lesson"
        );
    }

    #[test]
    fn percent_encoding_escapes_spaces_and_symbols() {
        assert_eq!(percent_encode("a b&c"), "a%20b%26c");
        assert_eq!(percent_encode("keep-_.~"), "keep-_.~");
    }

    #[test]
    fn strength_bands_split_at_0_34_and_0_67() {
        assert_eq!(strength_class(0.0), "low");
        assert_eq!(strength_class(0.33), "low");
        assert_eq!(strength_class(0.34), "medium");
        assert_eq!(strength_class(0.66), "medium");
        assert_eq!(strength_class(0.67), "high");
    }

    #[test]
    fn a_phone_can_tighten_the_brain_mode_only() {
        assert!(phone_may_pick(BrainMode::Learn, BrainMode::Off, true));
        assert!(phone_may_pick(BrainMode::Learn, BrainMode::ReadOnly, true));
        assert!(!phone_may_pick(BrainMode::Off, BrainMode::Learn, true));
    }

    #[test]
    fn a_computer_in_a_narrow_window_may_loosen_the_brain_mode() {
        assert!(phone_may_pick(BrainMode::Off, BrainMode::Learn, false));
        assert!(phone_may_pick(BrainMode::Off, BrainMode::ReadOnly, false));
    }

    #[test]
    fn building_notes_switches_every_mode_but_learn() {
        assert!(mode_needs_switch(BrainMode::Off));
        assert!(mode_needs_switch(BrainMode::ReadOnly));
        assert!(!mode_needs_switch(BrainMode::Learn));
    }

    #[test]
    fn an_empty_vault_offers_the_build_on_the_computer_only() {
        assert_eq!(empty_notes_choice(false), EmptyChoice::BuildNotes);
        assert_eq!(empty_notes_choice(true), EmptyChoice::AskComputer);
    }

    #[test]
    fn a_graph_with_no_notes_is_empty() {
        let node = GraphNodeView {
            id: NoteId::new("a"),
            title: "a".to_owned(),
            note_type: NoteType::Fix,
            backlinks: 0,
        };
        assert!(graph_is_empty(&[]));
        assert!(!graph_is_empty(&[node]));
    }

    #[test]
    fn run_care_is_offered_on_the_computer_only() {
        assert!(care_offered(false));
        assert!(!care_offered(true));
    }

    #[test]
    fn only_an_unfiltered_empty_list_means_an_empty_vault() {
        assert!(is_unfiltered("  ", ""));
        assert!(!is_unfiltered("fees", ""));
        assert!(!is_unfiltered("", "lesson"));
    }

    #[test]
    fn a_link_counts_at_either_end() {
        let links = vec![
            (NoteId::new("a"), NoteId::new("b")),
            (NoteId::new("b"), NoteId::new("c")),
        ];
        assert_eq!(link_count(&links, &NoteId::new("b")), 2);
        assert_eq!(link_count(&links, &NoteId::new("z")), 0);
    }

    #[test]
    fn pinned_notes_come_first_and_keep_their_order() {
        let mut notes = vec![
            summary("a", false),
            summary("b", true),
            summary("c", false),
            summary("d", true),
        ];
        pinned_first(&mut notes);
        let ids: Vec<&str> = notes.iter().map(|note| note.id.as_str()).collect();
        assert_eq!(ids, ["b", "d", "a", "c"]);
    }

    #[test]
    fn a_proven_lesson_offers_retire_and_reject_but_not_approve() {
        assert!(!review_offered(LessonState::Proven, LessonReview::Approve));
        assert!(review_offered(LessonState::Proven, LessonReview::Reject));
        assert!(review_offered(LessonState::Proven, LessonReview::Retire));
    }

    #[test]
    fn a_rejected_lesson_offers_only_retire() {
        assert!(!review_offered(
            LessonState::Rejected,
            LessonReview::Approve
        ));
        assert!(!review_offered(LessonState::Rejected, LessonReview::Reject));
        assert!(review_offered(LessonState::Rejected, LessonReview::Retire));
    }

    #[test]
    fn a_404_is_the_missing_route_and_other_failures_are_not() {
        let missing = ApiFailure::Status {
            code: 404,
            message: "not found".to_owned(),
        };
        let conflict = ApiFailure::Status {
            code: 409,
            message: "changed".to_owned(),
        };
        assert!(is_missing(&missing));
        assert!(!is_missing(&conflict));
    }
}
