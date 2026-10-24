//! The guide's thirteen steps and the Try it buttons under each one. The copy is the prototype's
//! `GUIDE` array. The modal that shows it is `components/guide_modal.rs`.

use crate::layout::PanelId;

/// What a Try it button does in the real app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideAction {
    /// Opens Settings on this page key.
    Settings(&'static str),
    Panel(PanelId),
    /// The prototype starts runs here. The guide only puts the cursor in the message box,
    /// so a click never spends budget.
    FocusComposer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuideTry {
    pub label: &'static str,
    pub action: GuideAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuideStep {
    pub title: &'static str,
    /// HTML. The `k` spans mark commands and file names.
    pub body: &'static str,
    pub tries: &'static [GuideTry],
}

pub const GUIDE_STEPS: &[GuideStep] = &[
    GuideStep {
        title: "Install",
        body: r#"Build Sentinel once and install Tailscale. Run <span class="k">sentinel doctor</span> to check git, Tailscale, the keychain and the agent CLIs it can find."#,
        tries: &[GuideTry {
            label: "Open the install step",
            action: GuideAction::Settings("general"),
        }],
    },
    GuideStep {
        title: "Set up",
        body: "Choose the repository, turn on at least 3 agents across 2 tools, set the budget cap and permission mode, pair your phone, and finish with a dry run on the sample repo.",
        tries: &[GuideTry {
            label: "Open setup",
            action: GuideAction::Settings("general"),
        }],
    },
    GuideStep {
        title: "Write the backlog",
        body: r#"Put a <span class="k">backlog.md</span> in the repository. One heading per task, with a sentence or two on what done means. Do not describe files. Sentinel finds the scope itself."#,
        tries: &[GuideTry {
            label: "Use the sample backlog",
            action: GuideAction::FocusComposer,
        }],
    },
    GuideStep {
        title: "Read the plan",
        body: "Sentinel reads the backlog and the code, then shows scopes, hidden dependencies, contradictions and tasks that claim to be done. Check this before you let anything write.",
        tries: &[GuideTry {
            label: "Plan only first",
            action: GuideAction::FocusComposer,
        }],
    },
    GuideStep {
        title: "Answer decisions",
        body: "Contradictions need you. Pick one task, or skip both. If you do not answer, Sentinel skips both and records a Contradiction. It never guesses.",
        tries: &[],
    },
    GuideStep {
        title: "Pick permissions and Brain mode",
        body: "Under the message box choose how much agents may do alone. Agents can only write files their task has reserved, whatever you pick. For live runs keep the Brain on read-only.",
        tries: &[GuideTry {
            label: "Open permissions",
            action: GuideAction::Panel(PanelId::Permissions),
        }],
    },
    GuideStep {
        title: "Watch the stream",
        body: "The stream shows plans, reserved files, agent steps, checkpoints, tests and verdicts. Open panels from the top bar. Click the agent avatars to focus one agent, open its terminal, or kill it.",
        tries: &[
            GuideTry {
                label: "Open the board",
                action: GuideAction::Panel(PanelId::Board),
            },
            GuideTry {
                label: "Open the terminal",
                action: GuideAction::Panel(PanelId::Terminal),
            },
        ],
    },
    GuideStep {
        title: "Approve requests",
        body: "When an agent needs a file its task has not reserved, a card asks you. Allow if it is free and sensible. Deny to keep it to its own files.",
        tries: &[],
    },
    GuideStep {
        title: "Trust the verdicts",
        body: "A task is Verified only when the full suite passes on a clean tree, a new test exists, a different agent reviewed the diff, and no existing test was edited. Quality notes are warnings only.",
        tries: &[GuideTry {
            label: "Open the report",
            action: GuideAction::Panel(PanelId::Report),
        }],
    },
    GuideStep {
        title: "Handle blocked tasks",
        body: "A blocked task says why. Raise the tier, narrow the scope, or fix the cause (a missing mock, say) and run it again. Do not argue with the report.",
        tries: &[],
    },
    GuideStep {
        title: "Watch the budget",
        body: "The cost chip is always in the header. At 70% no new tasks start. At 95% workers stop. You can lower the cap, never raise it.",
        tries: &[GuideTry {
            label: "Open cost",
            action: GuideAction::Panel(PanelId::Cost),
        }],
    },
    GuideStep {
        title: "Control it from your phone",
        body: "Pair over Tailscale and the phone shows the whole harness. It can pause, kill an agent, answer cards, lower the cap and review the Brain. It cannot raise the cap or touch files.",
        tries: &[GuideTry {
            label: "Open Remote",
            action: GuideAction::Panel(PanelId::Remote),
        }],
    },
    GuideStep {
        title: "Review the Brain",
        body: "After a run, open the Brain. Read the run note, approve only the lessons that helped, and fix any note that is wrong. Brain care runs on a schedule and proposes improvements. You accept or dismiss each one.",
        tries: &[
            GuideTry {
                label: "Open the Brain",
                action: GuideAction::Panel(PanelId::Brain),
            },
            GuideTry {
                label: "Brain care schedule",
                action: GuideAction::Settings("brain"),
            },
        ],
    },
];

/// The step after `current`, stopping at the last one.
pub fn next_index(current: usize, total: usize) -> usize {
    (current + 1).min(total.saturating_sub(1))
}

/// The step before `current`, stopping at the first one.
pub fn previous_index(current: usize) -> usize {
    current.saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guide_has_thirteen_steps() {
        assert_eq!(GUIDE_STEPS.len(), 13);
    }

    #[test]
    fn every_guide_step_has_a_title_and_a_body() {
        for step in GUIDE_STEPS {
            assert!(!step.title.is_empty(), "a step has no title");
            assert!(!step.body.is_empty(), "{} has no body", step.title);
        }
    }

    #[test]
    fn every_try_it_button_has_a_label() {
        for step in GUIDE_STEPS {
            for button in step.tries {
                assert!(
                    !button.label.is_empty(),
                    "{} has an unlabeled button",
                    step.title
                );
            }
        }
    }

    #[test]
    fn first_step_copy_matches_the_prototype() {
        assert_eq!(GUIDE_STEPS[0].title, "Install");
        assert!(GUIDE_STEPS[0].body.starts_with("Build Sentinel once"));
    }

    #[test]
    fn next_index_stops_at_the_last_step() {
        assert_eq!(next_index(0, 13), 1);
        assert_eq!(next_index(12, 13), 12);
    }

    #[test]
    fn previous_index_stops_at_the_first_step() {
        assert_eq!(previous_index(0), 0);
        assert_eq!(previous_index(5), 4);
    }
}
