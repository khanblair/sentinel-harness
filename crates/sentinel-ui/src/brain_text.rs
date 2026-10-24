//! Brain panel copy: labels, button words, and the lines the panel shows after an action. Plain
//! functions only, so the tests run on the host.

use sentinel_core::brain_api::LessonReview;
use sentinel_core::{BrainMode, Cost, LessonState, NoteType, ProposalKind};

pub fn type_label(note_type: NoteType) -> &'static str {
    match note_type {
        NoteType::Convention => "Convention",
        NoteType::Decision => "Decision",
        NoteType::Fix => "Fix",
        NoteType::File => "File",
        NoteType::Task => "Task",
        NoteType::Run => "Run",
        NoteType::Lesson => "Lesson",
        NoteType::Glossary => "Glossary",
        NoteType::HowTo => "How-to",
    }
}

/// The author line of a note: `you`, `care`, or a run (Sentinel).
pub fn author_label(by: &str) -> &'static str {
    match by {
        "you" => "by you",
        "care" => "by Brain care",
        _ => "by Sentinel",
    }
}

/// The mode name the panel shows in its chip, the same words as the daemon.
pub fn mode_name(mode: BrainMode) -> &'static str {
    match mode {
        BrainMode::Off => "off",
        BrainMode::ReadOnly => "read-only",
        BrainMode::Learn => "learn",
    }
}

/// One sentence about the mode, from the prototype.
pub fn mode_sentence(mode: BrainMode) -> &'static str {
    match mode {
        BrainMode::Off => "Off. Nothing is read or written.",
        BrainMode::ReadOnly => {
            "Read only. Agents read notes. Nothing is written. Best for live runs."
        }
        BrainMode::Learn => {
            "Learn. Agents read notes. Every run writes a journal and lesson candidates."
        }
    }
}

pub fn proposal_kind_label(kind: ProposalKind) -> &'static str {
    match kind {
        ProposalKind::Add => "Add",
        ProposalKind::Update => "Update",
        ProposalKind::Retire => "Retire",
        ProposalKind::Merge => "Merge",
    }
}

pub fn lesson_state_label(state: LessonState) -> &'static str {
    match state {
        LessonState::Candidate => "Candidate",
        LessonState::Proven => "Proven",
        LessonState::Rejected => "Rejected",
        LessonState::Retired => "Retired",
    }
}

pub fn lesson_state_color(state: LessonState) -> &'static str {
    match state {
        LessonState::Candidate | LessonState::Retired => "muted",
        LessonState::Proven => "ok",
        LessonState::Rejected => "danger",
    }
}

/// `Proposals`, or `Proposals 2` when proposals are open.
pub fn proposals_tab_label(open: usize) -> String {
    if open == 0 {
        "Proposals".to_owned()
    } else {
        format!("Proposals {open}")
    }
}

/// The three tabs of the Brain panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrainTab {
    Notes,
    Graph,
    Proposals,
}

impl BrainTab {
    pub const ALL: [BrainTab; 3] = [BrainTab::Notes, BrainTab::Graph, BrainTab::Proposals];

    pub fn label(self) -> &'static str {
        match self {
            BrainTab::Notes => "Notes",
            BrainTab::Graph => "Graph",
            BrainTab::Proposals => "Proposals",
        }
    }
}

pub fn review_label(action: LessonReview) -> &'static str {
    match action {
        LessonReview::Approve => "Approve",
        LessonReview::Reject => "Reject",
        LessonReview::Retire => "Retire",
    }
}

pub fn review_done_text(action: LessonReview) -> &'static str {
    match action {
        LessonReview::Approve => "Lesson approved.",
        LessonReview::Reject => "Lesson rejected.",
        LessonReview::Retire => "Lesson retired.",
    }
}

/// `by you · confirmed Oct 20 · used 6 times`. The confirmed part is left out until there is one.
pub fn note_meta_line(by: &str, confirmed: Option<&str>, uses: u32) -> String {
    let mut parts = vec![author_label(by).to_owned()];
    if let Some(date) = confirmed {
        parts.push(format!("confirmed {date}"));
    }
    let times = if uses == 1 { "time" } else { "times" };
    parts.push(format!("used {uses} {times}"));
    parts.join(" · ")
}

pub fn pin_done_text(pinned: bool) -> &'static str {
    if pinned {
        "This note is always loaded for agents."
    } else {
        "This note is no longer always loaded."
    }
}

pub fn answer_done_text(accept: bool) -> &'static str {
    if accept {
        "Proposal accepted."
    } else {
        "Proposal dismissed."
    }
}

/// Why a new note cannot be saved yet, or `None` when it can.
pub fn new_note_error(title: &str) -> Option<&'static str> {
    title.trim().is_empty().then_some("A note needs a title.")
}

/// Why a care budget cannot be saved, or `None` when it can.
pub fn budget_error(text: &str) -> Option<&'static str> {
    match Cost::parse_usd(text.trim()) {
        Err(_) => Some("Enter a dollar amount, such as 0.10."),
        Ok(cost) if cost.micro_usd() == 0 => Some("The budget must be more than $0.00."),
        Ok(_) => None,
    }
}
