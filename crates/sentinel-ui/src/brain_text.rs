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

/// The panel's one line about the mode.
pub fn mode_sentence(mode: BrainMode) -> &'static str {
    match mode {
        BrainMode::Off => "Off",
        BrainMode::ReadOnly => "Read-only: agents read notes and write nothing",
        BrainMode::Learn => "Learn: runs add notes and lesson candidates",
    }
}

pub const LOADING_NOTES: &str = "Loading notes...";
pub const LOADING_NOTE: &str = "Loading the note...";
pub const LOADING_GRAPH: &str = "Loading the graph...";
pub const LOADING_PROPOSALS: &str = "Loading proposals...";
pub const NO_NOTES_TEXT: &str = "The Brain has no notes yet.";
pub const NO_NOTES_MATCH: &str = "No notes match.";
pub const BUILD_NOTES_LABEL: &str = "Build notes from this project";
pub const READING_PROJECT_LABEL: &str = "Reading the project";
pub const ASK_COMPUTER_BUILD_TEXT: &str = "Ask the computer to build the notes.";
pub const LEARN_SWITCH_TEXT: &str = "This turns the Brain to Learn mode so it may write notes.";
pub const SWITCH_TO_LEARN_LABEL: &str = "Switch to Learn";
pub const NO_LINKS_TEXT: &str = "No links yet. Build notes from this project, or write a note that links another with [[its name]].";
pub const NOTHING_TO_REVIEW_TEXT: &str =
    "Nothing to review. Run care now to check the notes against the project.";
pub const RUN_CARE_LABEL: &str = "Run care now";

/// The toast after a seed. The switch sentence comes first when the Brain had to change mode.
pub fn seed_done_text(switched: bool, created: u32) -> String {
    let word = if created == 1 { "note" } else { "notes" };
    let made = format!("Created {created} {word}.");
    if switched {
        format!("Brain switched to Learn. {made}")
    } else {
        made
    }
}

/// The toast after a care run, with the number of proposals it returned.
pub fn care_done_text(count: u64) -> String {
    match count {
        0 => "Brain care found nothing to propose.".to_owned(),
        1 => "Brain care found 1 proposal.".to_owned(),
        many => format!("Brain care found {many} proposals."),
    }
}

/// `1 link`, `3 links`, or `No links` for a note's link count.
pub fn links_text(count: usize) -> String {
    match count {
        0 => "No links".to_owned(),
        1 => "1 link".to_owned(),
        many => format!("{many} links"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn author_lines_name_the_writer() {
        assert_eq!(author_label("you"), "by you");
        assert_eq!(author_label("care"), "by Brain care");
        assert_eq!(author_label("run"), "by Sentinel");
    }

    #[test]
    fn a_lesson_state_has_a_label_and_a_color() {
        assert_eq!(lesson_state_label(LessonState::Proven), "Proven");
        assert_eq!(lesson_state_color(LessonState::Rejected), "danger");
        assert_eq!(proposal_kind_label(ProposalKind::Merge), "Merge");
    }

    #[test]
    fn the_proposals_tab_shows_the_open_count_only_when_there_are_some() {
        assert_eq!(proposals_tab_label(0), "Proposals");
        assert_eq!(proposals_tab_label(3), "Proposals 3");
    }

    #[test]
    fn a_budget_must_be_a_positive_dollar_amount() {
        assert_eq!(budget_error("0.10"), None);
        assert!(budget_error("0").is_some());
        assert!(budget_error("ten").is_some());
    }

    #[test]
    fn the_meta_line_names_the_author_date_and_uses() {
        assert_eq!(
            note_meta_line("you", Some("Oct 20"), 6),
            "by you · confirmed Oct 20 · used 6 times"
        );
        assert_eq!(note_meta_line("run", None, 1), "by Sentinel · used 1 time");
    }

    #[test]
    fn a_new_note_needs_a_title() {
        assert!(new_note_error("   ").is_some());
        assert_eq!(new_note_error("Fees"), None);
    }

    #[test]
    fn each_mode_has_its_own_sentence() {
        assert_eq!(mode_sentence(BrainMode::Off), "Off");
        assert_eq!(
            mode_sentence(BrainMode::ReadOnly),
            "Read-only: agents read notes and write nothing"
        );
        assert_eq!(
            mode_sentence(BrainMode::Learn),
            "Learn: runs add notes and lesson candidates"
        );
    }

    #[test]
    fn the_seed_toast_says_when_the_brain_switched_to_learn() {
        assert_eq!(
            seed_done_text(true, 12),
            "Brain switched to Learn. Created 12 notes."
        );
        assert_eq!(seed_done_text(false, 1), "Created 1 note.");
    }

    #[test]
    fn the_care_toast_counts_the_proposals() {
        assert_eq!(care_done_text(0), "Brain care found nothing to propose.");
        assert_eq!(care_done_text(1), "Brain care found 1 proposal.");
        assert_eq!(care_done_text(4), "Brain care found 4 proposals.");
    }

    #[test]
    fn a_link_count_reads_in_words() {
        assert_eq!(links_text(0), "No links");
        assert_eq!(links_text(1), "1 link");
        assert_eq!(links_text(3), "3 links");
    }
}
