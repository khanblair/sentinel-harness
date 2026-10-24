//! Chat levels and the routes each level may call. A route that no level allows is refused.

use std::collections::BTreeSet;

use crate::quiet::QuietHours;

/// Words for the refusal text. Each one needs the computer, so no chat can do it.
pub const NEVER: [&str; 4] = [
    "raise the cap",
    "Bypass permissions",
    "edit notes or tests",
    "shell commands",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Notify,
    Control,
    Code,
    Reports,
}

/// Every route a command or a button may call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    Status,
    Pause,
    Resume,
    Stop,
    Kill,
    LowerCap,
    Approve,
    Decide,
    AgentMessage,
    DiffSummary,
    AddTask,
    StartRun,
    Report,
    Daily,
    BrainSearch,
}

/// How much of an event a chat gets. The order goes from least to most.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Detail {
    Status,
    Summary,
    #[default]
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventKind {
    ApprovalNeeded,
    DecisionNeeded,
    Budget70,
    Budget95,
    TaskBlocked,
    WaveFinished,
    RunFinished,
    BrainProposals,
    Errors,
}

/// What one private chat gets and may do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatSettings {
    pub levels: BTreeSet<Level>,
    pub detail: Detail,
    pub events: BTreeSet<EventKind>,
    pub quiet_hours: Option<QuietHours>,
}

pub fn allowed(levels: &BTreeSet<Level>, action: Action) -> bool {
    levels.iter().any(|level| level_allows(*level, action))
}

pub fn refusal_text(what: &str) -> String {
    format!("{what}. That needs the computer. Do it there.")
}

/// Notify only sends. It allows no route.
fn level_allows(level: Level, action: Action) -> bool {
    match level {
        Level::Notify => false,
        Level::Control => matches!(
            action,
            Action::Status
                | Action::Pause
                | Action::Resume
                | Action::Stop
                | Action::Kill
                | Action::LowerCap
                | Action::Approve
                | Action::Decide
        ),
        Level::Code => matches!(
            action,
            Action::AgentMessage
                | Action::DiffSummary
                | Action::AddTask
                | Action::StartRun
                | Action::BrainSearch
        ),
        Level::Reports => matches!(action, Action::Report | Action::Daily),
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, Level, allowed};
    use std::collections::BTreeSet;

    fn only(level: Level) -> BTreeSet<Level> {
        BTreeSet::from([level])
    }

    #[test]
    fn notify_allows_no_route() {
        assert!(!allowed(&only(Level::Notify), Action::Status));
        assert!(!allowed(&only(Level::Notify), Action::Report));
    }

    #[test]
    fn control_allows_only_the_control_routes() {
        let control = only(Level::Control);
        assert!(allowed(&control, Action::Pause));
        assert!(allowed(&control, Action::LowerCap));
        assert!(allowed(&control, Action::Decide));
        assert!(!allowed(&control, Action::StartRun));
        assert!(!allowed(&control, Action::Report));
    }

    #[test]
    fn code_allows_only_the_code_routes() {
        let code = only(Level::Code);
        assert!(allowed(&code, Action::AddTask));
        assert!(allowed(&code, Action::StartRun));
        assert!(!allowed(&code, Action::Stop));
    }

    #[test]
    fn reports_allow_only_report_and_daily() {
        let reports = only(Level::Reports);
        assert!(allowed(&reports, Action::Daily));
        assert!(!allowed(&reports, Action::Kill));
    }

    #[test]
    fn two_levels_add_their_routes() {
        let both = BTreeSet::from([Level::Control, Level::Reports]);
        assert!(allowed(&both, Action::Stop));
        assert!(allowed(&both, Action::Report));
    }

    #[test]
    fn the_refusal_names_the_computer() {
        assert_eq!(
            super::refusal_text("Raise the cap"),
            "Raise the cap. That needs the computer. Do it there."
        );
    }
}
