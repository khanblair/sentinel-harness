//! Brain panel logic: link parsing, strength classes, the phone rule, the graph layout, and the care
//! form values. The labels and copy live in `brain_text.rs`. Plain functions only, so the tests run
//! on the host.

use std::f64::consts::{FRAC_PI_2, TAU};
use std::fmt::Write;

use sentinel_core::brain_api::{CareFrequency, GraphNodeView, LessonReview};
use sentinel_core::{BrainMode, LessonState, NoteId, NoteType};

use crate::api::ApiFailure;
pub use crate::brain_text::{
    BrainTab, answer_done_text, budget_error, lesson_state_color, lesson_state_label, mode_name,
    mode_sentence, new_note_error, note_meta_line, pin_done_text, proposal_kind_label,
    proposals_tab_label, review_done_text, review_label, type_label,
};
use crate::layout::Layout;

/// Shown in place of a Brain view when the daemon has no Brain route for it yet.
pub const UNAVAILABLE: &str = "The Brain is not available yet in this build.";

/// Every note type, in the order the type filter and the graph use.
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

/// Canvas size of the graph, in SVG units.
pub const GRAPH_WIDTH: f64 = 400.0;
pub const GRAPH_HEIGHT: f64 = 340.0;

/// Notes a care run adds to the schedule when it runs after every N runs.
pub const CARE_RUNS: u32 = 5;

/// One piece of a note body: plain text, or a `[[id]]` or `[[id|label]]` link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Text(String),
    Link { id: String, label: Option<String> },
}

/// Splits a note body into text and links. An unclosed `[[` stays as text.
pub fn link_segments(body: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            break;
        };
        if start > 0 {
            segments.push(Segment::Text(rest[..start].to_owned()));
        }
        segments.push(link_segment(&after[..end]));
        rest = &after[end + 2..];
    }
    if !rest.is_empty() {
        segments.push(Segment::Text(rest.to_owned()));
    }
    segments
}

fn link_segment(inner: &str) -> Segment {
    match inner.split_once('|') {
        Some((id, label)) => Segment::Link {
            id: id.trim().to_owned(),
            label: non_empty(label.trim()),
        },
        None => Segment::Link {
            id: inner.trim().to_owned(),
            label: None,
        },
    }
}

fn non_empty(text: &str) -> Option<String> {
    (!text.is_empty()).then(|| text.to_owned())
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

/// A phone can only tighten the mode. Every other layout may pick any mode.
pub fn phone_may_pick(current: BrainMode, target: BrainMode, layout: Layout) -> bool {
    layout != Layout::Phone || current.phone_may_set(target)
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

/// A note dot on the graph canvas.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub id: NoteId,
    pub title: String,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub color: &'static str,
}

/// Places each note on a ring for its type. The ring radius comes from a hash of the id, so a
/// note stays where it was on the next load.
pub fn layout(nodes: &[GraphNodeView], width: f64, height: f64) -> Vec<Placed> {
    let (cx, cy) = (width / 2.0, height / 2.0);
    let mut placed = Vec::with_capacity(nodes.len());
    for (type_index, note_type) in NOTE_TYPES.iter().enumerate() {
        let group: Vec<&GraphNodeView> = nodes
            .iter()
            .filter(|node| node.note_type == *note_type)
            .collect();
        for (index, node) in group.iter().enumerate() {
            let angle = group_angle(type_index, index, group.len());
            let radius = ring_radius(node.id.as_str());
            placed.push(Placed {
                id: node.id.clone(),
                title: node.title.clone(),
                x: cx + angle.cos() * radius * 1.3,
                y: cy + angle.sin() * radius * 0.95,
                radius: dot_radius(node.backlinks),
                color: type_color_token(*note_type),
            });
        }
    }
    placed
}

/// The angle of one note: each type gets a sector of the circle, notes spread inside it.
fn group_angle(type_index: usize, index: usize, group_len: usize) -> f64 {
    let within = (as_f64(index) + 0.5) / as_f64(group_len);
    let slot = (as_f64(type_index) + within) / as_f64(NOTE_TYPES.len());
    slot * TAU - FRAC_PI_2
}

fn ring_radius(id: &str) -> f64 {
    80.0 + f64::from(stable_hash(id) % 70)
}

/// Dots grow with the number of backlinks, up to 10 px.
fn dot_radius(backlinks: u32) -> f64 {
    4.0 + (f64::from(backlinks) * 1.2).min(6.0)
}

/// FNV-1a. Std's hasher is seeded per run, so it cannot place a note the same way twice.
pub fn stable_hash(text: &str) -> u32 {
    text.bytes().fold(2_166_136_261_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(16_777_619)
    })
}

fn as_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).unwrap_or(u32::MAX))
}

/// The two ends of a link, or `None` when an end is not on the canvas.
pub fn edge_points(placed: &[Placed], link: &(NoteId, NoteId)) -> Option<[(f64, f64); 2]> {
    let from = placed.iter().find(|dot| dot.id == link.0)?;
    let to = placed.iter().find(|dot| dot.id == link.1)?;
    Some([(from.x, from.y), (to.x, to.y)])
}

/// The choices of the care schedule select, as (value, label).
pub const FREQUENCY_CHOICES: [(&str, &str); 5] = [
    ("off", "Off"),
    ("daily", "Daily"),
    ("weekly", "Weekly"),
    ("monthly", "Monthly"),
    ("runs", "After every 5 runs"),
];

pub fn frequency_value(frequency: CareFrequency) -> &'static str {
    match frequency {
        CareFrequency::Off => "off",
        CareFrequency::Daily => "daily",
        CareFrequency::Weekly => "weekly",
        CareFrequency::Monthly => "monthly",
        CareFrequency::AfterRuns(_) => "runs",
    }
}

pub fn frequency_from_value(value: &str) -> CareFrequency {
    match value {
        "daily" => CareFrequency::Daily,
        "weekly" => CareFrequency::Weekly,
        "monthly" => CareFrequency::Monthly,
        "runs" => CareFrequency::AfterRuns(CARE_RUNS),
        _ => CareFrequency::Off,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, note_type: NoteType, backlinks: u32) -> GraphNodeView {
        GraphNodeView {
            id: NoteId::new(id),
            title: id.to_owned(),
            note_type,
            backlinks,
        }
    }

    fn link(id: &str, label: Option<&str>) -> Segment {
        Segment::Link {
            id: id.to_owned(),
            label: label.map(str::to_owned),
        }
    }

    #[test]
    fn plain_text_is_one_text_segment() {
        assert_eq!(
            link_segments("Run the tests."),
            vec![Segment::Text("Run the tests.".to_owned())]
        );
    }

    #[test]
    fn a_link_splits_the_text_around_it() {
        assert_eq!(
            link_segments("See [[conv-tests]] first."),
            vec![
                Segment::Text("See ".to_owned()),
                link("conv-tests", None),
                Segment::Text(" first.".to_owned()),
            ]
        );
    }

    #[test]
    fn a_pipe_label_names_the_link() {
        assert_eq!(
            link_segments("[[conv-tests|the test rule]]"),
            vec![link("conv-tests", Some("the test rule"))]
        );
    }

    #[test]
    fn an_unclosed_link_stays_as_text() {
        assert_eq!(
            link_segments("open [[never closed"),
            vec![Segment::Text("open [[never closed".to_owned())]
        );
    }

    #[test]
    fn an_empty_body_has_no_segments() {
        assert_eq!(link_segments(""), Vec::new());
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
        assert!(phone_may_pick(
            BrainMode::Learn,
            BrainMode::Off,
            Layout::Phone
        ));
        assert!(phone_may_pick(
            BrainMode::Learn,
            BrainMode::ReadOnly,
            Layout::Phone
        ));
        assert!(!phone_may_pick(
            BrainMode::Off,
            BrainMode::Learn,
            Layout::Phone
        ));
        assert!(phone_may_pick(
            BrainMode::Off,
            BrainMode::Learn,
            Layout::Desktop
        ));
        assert!(phone_may_pick(
            BrainMode::Off,
            BrainMode::Learn,
            Layout::Tablet
        ));
    }

    #[test]
    fn a_care_schedule_value_round_trips() {
        assert_eq!(frequency_value(frequency_from_value("runs")), "runs");
        assert_eq!(
            frequency_from_value("runs"),
            CareFrequency::AfterRuns(CARE_RUNS)
        );
        assert_eq!(frequency_from_value("unknown"), CareFrequency::Off);
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

    #[test]
    fn the_hash_is_the_same_on_every_call() {
        assert_eq!(stable_hash("conv-tests"), stable_hash("conv-tests"));
        assert_ne!(stable_hash("conv-tests"), stable_hash("conv-money"));
    }

    #[test]
    fn the_layout_places_every_node_and_keeps_its_type_together() {
        let nodes = vec![
            node("a", NoteType::Fix, 0),
            node("b", NoteType::Fix, 0),
            node("c", NoteType::Glossary, 0),
        ];
        let placed = layout(&nodes, GRAPH_WIDTH, GRAPH_HEIGHT);
        assert_eq!(placed.len(), 3);
        assert_eq!(placed[0].color, "ok");
        assert_eq!(placed[2].color, "muted");
    }

    #[test]
    fn the_layout_is_the_same_for_the_same_notes() {
        let nodes = vec![node("a", NoteType::Run, 2)];
        assert_eq!(
            layout(&nodes, GRAPH_WIDTH, GRAPH_HEIGHT),
            layout(&nodes, GRAPH_WIDTH, GRAPH_HEIGHT)
        );
    }

    #[test]
    fn a_dot_grows_with_backlinks_and_stops_at_ten() {
        assert!((dot_radius(0) - 4.0).abs() < f64::EPSILON);
        assert!(dot_radius(2) > dot_radius(1));
        assert!((dot_radius(100) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn an_empty_graph_has_no_dots() {
        assert_eq!(layout(&[], GRAPH_WIDTH, GRAPH_HEIGHT), Vec::new());
    }

    #[test]
    fn an_edge_needs_both_ends_on_the_canvas() {
        let nodes = vec![node("a", NoteType::Fix, 0), node("b", NoteType::Fix, 0)];
        let placed = layout(&nodes, GRAPH_WIDTH, GRAPH_HEIGHT);
        let both = (NoteId::from("a"), NoteId::from("b"));
        let gone = (NoteId::from("a"), NoteId::from("zz"));
        assert!(edge_points(&placed, &both).is_some());
        assert!(edge_points(&placed, &gone).is_none());
    }
}
