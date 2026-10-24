//! `OnboardingStepper`: the six step names across the top of the setup wizard, and the rules
//! the wizard checks before it moves on (`docs/product-scope.md`, F8).

use leptos::prelude::*;

use crate::components::icon::Icon;

/// The wizard steps in order. Each number is the step the daemon stores in `onboarding.step`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardStep {
    Install,
    Repository,
    Agents,
    Budget,
    Phone,
    DryRun,
}

impl WizardStep {
    pub const ALL: [WizardStep; 6] = [
        WizardStep::Install,
        WizardStep::Repository,
        WizardStep::Agents,
        WizardStep::Budget,
        WizardStep::Phone,
        WizardStep::DryRun,
    ];

    /// An unknown number reads as the last step, so a newer daemon never leaves the page blank.
    pub fn from_number(number: u8) -> Self {
        match number {
            0 => WizardStep::Install,
            1 => WizardStep::Repository,
            2 => WizardStep::Agents,
            3 => WizardStep::Budget,
            4 => WizardStep::Phone,
            _ => WizardStep::DryRun,
        }
    }

    pub fn number(self) -> u8 {
        match self {
            WizardStep::Install => 0,
            WizardStep::Repository => 1,
            WizardStep::Agents => 2,
            WizardStep::Budget => 3,
            WizardStep::Phone => 4,
            WizardStep::DryRun => 5,
        }
    }

    /// The index in `ALL`, for the stepper.
    pub fn index(self) -> usize {
        usize::from(self.number())
    }
}

/// The step after this one, or `None` on the last step.
pub fn next_step(step: WizardStep) -> Option<WizardStep> {
    WizardStep::ALL.get(step.index() + 1).copied()
}

pub fn step_title(step: WizardStep) -> &'static str {
    match step {
        WizardStep::Install => "Install",
        WizardStep::Repository => "Repository",
        WizardStep::Agents => "Agents",
        WizardStep::Budget => "Budget",
        WizardStep::Phone => "Phone",
        WizardStep::DryRun => "Dry run",
    }
}

/// Only the phone and the dry run can be skipped by the step's own rule. The install check
/// has a skip button too, but it is not an onboarding step, so the wizard handles it apart.
pub fn skippable(step: WizardStep) -> bool {
    matches!(step, WizardStep::Phone | WizardStep::DryRun)
}

/// The steps whose own rule is met right now. Budget and Phone have no rule of their own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WizardState {
    pub ready: Vec<WizardStep>,
}

pub fn can_continue(step: WizardStep, state: &WizardState) -> bool {
    match step {
        WizardStep::Budget | WizardStep::Phone => true,
        WizardStep::Install | WizardStep::Repository | WizardStep::Agents | WizardStep::DryRun => {
            state.ready.contains(&step)
        }
    }
}

/// The label of one stepper item. The first step has no number, as in the prototype.
pub fn stepper_label(index: usize, title: &str) -> String {
    if index == 0 {
        title.to_string()
    } else {
        format!("{index} {title}")
    }
}

/// The class of one stepper item: current, done, or not reached yet.
pub fn stepper_class(index: usize, current: usize) -> &'static str {
    match index.cmp(&current) {
        std::cmp::Ordering::Equal => "on",
        std::cmp::Ordering::Less => "ok",
        std::cmp::Ordering::Greater => "",
    }
}

#[component]
pub fn OnboardingStepper(#[prop(into)] current: Signal<WizardStep>) -> impl IntoView {
    view! {
        <div class="steps" id="wSteps">
            {WizardStep::ALL
                .iter()
                .enumerate()
                .map(|(index, step)| {
                    let title = step_title(*step);
                    view! {
                        <span class=move || stepper_class(index, current.get().index())>
                            {move || {
                                (current.get().index() > index)
                                    .then(|| view! { <Icon name="check" size=13 /> })
                            }}
                            {stepper_label(index, title)}
                        </span>
                    }
                })
                .collect_view()}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_numbers_round_trip() {
        for step in WizardStep::ALL {
            assert_eq!(WizardStep::from_number(step.number()), step);
        }
    }

    #[test]
    fn unknown_step_number_reads_as_the_dry_run() {
        assert_eq!(WizardStep::from_number(9), WizardStep::DryRun);
    }

    #[test]
    fn the_last_step_has_no_next_step() {
        assert_eq!(next_step(WizardStep::Install), Some(WizardStep::Repository));
        assert_eq!(next_step(WizardStep::DryRun), None);
    }

    #[test]
    fn titles_match_the_prototype_steps() {
        let titles: Vec<&str> = WizardStep::ALL.iter().map(|s| step_title(*s)).collect();
        assert_eq!(
            titles,
            [
                "Install",
                "Repository",
                "Agents",
                "Budget",
                "Phone",
                "Dry run"
            ]
        );
    }

    #[test]
    fn only_phone_and_dry_run_are_skippable() {
        let skipped: Vec<WizardStep> = WizardStep::ALL
            .into_iter()
            .filter(|s| skippable(*s))
            .collect();
        assert_eq!(skipped, [WizardStep::Phone, WizardStep::DryRun]);
    }

    #[test]
    fn repository_needs_a_usable_folder_before_continue() {
        let empty = WizardState::default();
        let usable = WizardState {
            ready: vec![WizardStep::Repository],
        };
        assert!(!can_continue(WizardStep::Repository, &empty));
        assert!(can_continue(WizardStep::Repository, &usable));
    }

    #[test]
    fn budget_and_phone_always_continue() {
        let empty = WizardState::default();
        assert!(can_continue(WizardStep::Budget, &empty));
        assert!(can_continue(WizardStep::Phone, &empty));
    }

    #[test]
    fn dry_run_needs_a_finished_run_before_continue() {
        let empty = WizardState::default();
        assert!(!can_continue(WizardStep::DryRun, &empty));
    }

    #[test]
    fn stepper_marks_done_current_and_later_steps() {
        assert_eq!(stepper_class(0, 2), "ok");
        assert_eq!(stepper_class(2, 2), "on");
        assert_eq!(stepper_class(4, 2), "");
    }

    #[test]
    fn first_stepper_label_has_no_number() {
        assert_eq!(stepper_label(0, "Install"), "Install");
        assert_eq!(stepper_label(3, "Budget"), "3 Budget");
    }
}
