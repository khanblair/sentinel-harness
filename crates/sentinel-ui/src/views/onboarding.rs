//! `SetupWizard`: the full-screen setup (prototype `renderWizard`, `wizNav`, `finishSetup`). The
//! host shows it in an `ov wiz` overlay while onboarding is not complete. This component renders
//! the three bands inside that overlay: the step bar, the step body, and the footer.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::Cost;
use sentinel_core::api::{DoctorReport, RepoCheckRequest, RepoCheckResponse};
use serde::Serialize;

use crate::api::{self, ApiFailure};
use crate::components::icon::Icon;
use crate::components::install_checklist::all_found;
use crate::components::onboarding_stepper::{
    OnboardingStepper, WizardState, WizardStep, can_continue, next_step, skippable,
};
use crate::components::repo_checks;
use crate::state::{ToastKind, Ui, refresh, use_ui};
use crate::views::budget_step::{BudgetStep, CapUpdate};
use crate::views::dry_run::DryRunStep;
use crate::views::folder_picker::RepositoryStep;
use crate::views::install::{AgentsStep, InstallStep, enough_agents};
use crate::views::pairing::PhoneStep;

const COMPLETE_PATH: &str = "/api/onboarding/complete";

pub fn step_path(step: WizardStep) -> String {
    format!("/api/onboarding/{}", step.number())
}

pub fn skip_path(step: WizardStep) -> String {
    format!("/api/onboarding/skip/{}", step.number())
}

/// Skip for now shows on the install check and on the skippable steps, until the step's own
/// rule is met. The phone step has no rule, so its skip stays visible.
pub fn can_skip_now(step: WizardStep, state: &WizardState) -> bool {
    (step == WizardStep::Install || skippable(step)) && !state.ready.contains(&step)
}

pub fn continue_label(step: WizardStep) -> &'static str {
    if step == WizardStep::DryRun {
        "Finish setup"
    } else {
        "Continue"
    }
}

/// A cap in micro-dollars, shown in dollars and whole cents. Integer math only.
pub fn dollars_text(micro_usd: u64) -> String {
    format!(
        "{}.{:02}",
        micro_usd / 1_000_000,
        (micro_usd % 1_000_000) / 10_000
    )
}

/// The wizard's shared signals. Steps read and write them through `use_wizard()`.
#[derive(Clone, Copy)]
pub struct Wizard {
    pub ui: Ui,
    pub step: RwSignal<WizardStep>,
    /// The last install check, once it has run.
    pub doctor: RwSignal<Option<DoctorReport>>,
    /// The last repository check for the chosen folder.
    pub repo: RwSignal<Option<RepoCheckResponse>>,
    /// Set by "Continue anyway" on the agents step.
    pub agents_override: RwSignal<bool>,
    pub dry_run_done: RwSignal<bool>,
    pub cap_text: RwSignal<String>,
    /// Set when the daemon refuses a cap change before the first run.
    pub cap_locked: RwSignal<bool>,
    /// Set once the user acts. Until then the page follows the daemon's step and cap, because
    /// the first snapshot can arrive after the wizard opens.
    pub touched: RwSignal<bool>,
}

pub fn use_wizard() -> Wizard {
    expect_context::<Wizard>()
}

impl Wizard {
    /// The wizard resumes at the step the daemon stored.
    pub fn new(ui: Ui) -> Self {
        let (step, cap) = ui
            .snapshot
            .with_untracked(|s| (s.onboarding.step, s.cost.cap.micro_usd()));
        Self {
            ui,
            step: RwSignal::new(WizardStep::from_number(step)),
            doctor: RwSignal::new(None),
            repo: RwSignal::new(None),
            agents_override: RwSignal::new(false),
            dry_run_done: RwSignal::new(false),
            cap_text: RwSignal::new(dollars_text(cap)),
            cap_locked: RwSignal::new(false),
            touched: RwSignal::new(false),
        }
    }

    /// Which steps have met their own rule. The footer and the stepper read this.
    pub fn state(self) -> WizardState {
        let mut ready = Vec::new();
        if self
            .doctor
            .with(|report| report.as_ref().is_some_and(|r| all_found(&r.rows)))
        {
            ready.push(WizardStep::Install);
        }
        if self
            .repo
            .with(|repo| repo.as_ref().is_some_and(repo_checks::can_continue))
        {
            ready.push(WizardStep::Repository);
        }
        if self.agents_override.get() || self.ui.snapshot.with(|s| enough_agents(&s.tools)) {
            ready.push(WizardStep::Agents);
        }
        if self.dry_run_done.get() {
            ready.push(WizardStep::DryRun);
        }
        WizardState { ready }
    }

    pub fn go(self, step: WizardStep) {
        self.step.set(step);
        send_in_order(self.ui, vec![step_path(step)]);
    }

    pub fn back(self) {
        self.touched.set(true);
        let current = self.step.get_untracked();
        self.go(WizardStep::from_number(current.number().saturating_sub(1)));
    }

    /// The path of the folder the repository check ran on, if one ran.
    pub fn repo_path(self) -> Option<String> {
        self.repo
            .with_untracked(|repo| repo.as_ref().map(|r| r.path.clone()))
    }

    /// Continue. Each step saves its choice first where it has one, and the last step
    /// finishes setup.
    pub fn next(self) {
        self.touched.set(true);
        match self.step.get_untracked() {
            WizardStep::Repository => self.save_repo_then_go(),
            WizardStep::Budget => self.save_cap_then_go(),
            WizardStep::DryRun => self.finish(),
            step @ (WizardStep::Install | WizardStep::Agents | WizardStep::Phone) => {
                self.go(next_step(step).unwrap_or(step));
            }
        }
    }

    /// Saves the checked folder, then moves to the agents step. Does nothing without a check.
    fn save_repo_then_go(self) {
        let Some(path) = self.repo_path() else {
            return;
        };
        let body = RepoCheckRequest { path };
        self.save_then_go("/api/repo", body, WizardStep::Agents);
    }

    /// Saves a choice with a PUT, then moves on. A failed save stays on the step.
    fn save_then_go<B: Serialize + 'static>(self, path: &'static str, body: B, next: WizardStep) {
        let (ui, step) = (self.ui, self.step);
        spawn_local(async move {
            match api::send::<_, serde_json::Value>("PUT", path, &body).await {
                Ok(_) => {
                    step.set(next);
                    send_in_order(ui, vec![step_path(next)]);
                }
                Err(error) => ui.show_toast(error.to_string(), ToastKind::Danger),
            }
        });
    }

    /// Skip for now. The install check only moves on. The other skippable steps tell the
    /// daemon, and skipping the last step finishes setup.
    pub fn skip(self) {
        self.touched.set(true);
        let step = self.step.get_untracked();
        if step == WizardStep::Install {
            self.go(WizardStep::Repository);
            return;
        }
        let mut paths = vec![skip_path(step)];
        if let Some(next) = next_step(step) {
            self.step.set(next);
            paths.push(step_path(next));
        } else {
            paths.push(COMPLETE_PATH.to_string());
        }
        send_in_order(self.ui, paths);
    }

    /// Ends setup. The host closes the wizard once the snapshot says it is complete.
    pub fn finish(self) {
        self.touched.set(true);
        let ui = self.ui;
        spawn_local(async move {
            if post_or_toast(ui, COMPLETE_PATH).await {
                ui.setup_open.set(false);
                ui.show_toast("Setup complete", ToastKind::Ok);
                refresh(ui);
            }
        });
    }

    /// Saves the cap, then moves to the phone step. The daemon only lowers a cap after a run
    /// starts, so a refusal (404) makes the field read only and the user continues without it.
    fn save_cap_then_go(self) {
        if self.cap_locked.get_untracked() {
            self.go(WizardStep::Phone);
            return;
        }
        let text = self.cap_text.get_untracked().trim().to_string();
        if Cost::parse_usd(&text).is_err() {
            self.ui
                .show_toast("Enter a budget in dollars, such as 5.00.", ToastKind::Warn);
            return;
        }
        let (ui, step, locked) = (self.ui, self.step, self.cap_locked);
        spawn_local(async move {
            let body = CapUpdate { cap: text };
            match api::send::<_, serde_json::Value>("PUT", "/api/config/cap", &body).await {
                Ok(_) => {
                    step.set(WizardStep::Phone);
                    send_in_order(ui, vec![step_path(WizardStep::Phone)]);
                }
                Err(ApiFailure::Status { code: 404, .. }) => {
                    locked.set(true);
                    ui.show_toast(
                        "The cap can only be lowered after a run starts. It stays as it is.",
                        ToastKind::Info,
                    );
                }
                Err(error) => ui.show_toast(error.to_string(), ToastKind::Danger),
            }
        });
    }
}

/// Sends one request with no body. A failure becomes a toast. Returns whether the request worked.
pub async fn post_or_toast(ui: Ui, path: &str) -> bool {
    post_body_or_toast(ui, path, &serde_json::json!({})).await
}

/// Sends one request with `body`. A failure becomes a toast. Returns whether the request worked.
pub async fn post_body_or_toast<B: Serialize>(ui: Ui, path: &str, body: &B) -> bool {
    match api::post::<_, serde_json::Value>(path, body).await {
        Ok(_) => true,
        Err(error) => {
            ui.show_toast(error.to_string(), ToastKind::Danger);
            false
        }
    }
}

/// Sends the requests one after another, so a skip reaches the daemon before the move after
/// it. The snapshot is reloaded once they have all gone through.
fn send_in_order(ui: Ui, paths: Vec<String>) {
    spawn_local(async move {
        for path in &paths {
            if !post_or_toast(ui, path).await {
                return;
            }
        }
        refresh(ui);
    });
}

#[component]
pub fn SetupWizard() -> impl IntoView {
    let ui = use_ui();
    let wizard = Wizard::new(ui);
    provide_context(wizard);
    // The first snapshot may still be the empty one, so follow the daemon until the user acts.
    Effect::new(move |_| {
        if wizard.touched.get() {
            return;
        }
        let (step, cap) = ui
            .snapshot
            .with(|s| (s.onboarding.step, s.cost.cap.micro_usd()));
        wizard.step.set(WizardStep::from_number(step));
        wizard.cap_text.set(dollars_text(cap));
    });
    view! {
        <div class="wiz-hd">
            <span class="lg">
                <Icon name="shield" size=18 />
                "Sentinel setup"
            </span>
            <OnboardingStepper current=move || wizard.step.get() />
            <button class="btn sm" on:click=move |_| wizard.finish()>"Skip setup"</button>
        </div>
        <div class="wiz-body">
            <div class="wiz-in">{move || step_view(wizard.step.get())}</div>
        </div>
        <WizardFooter />
    }
}

fn step_view(step: WizardStep) -> AnyView {
    match step {
        WizardStep::Install => view! { <InstallStep /> }.into_any(),
        WizardStep::Repository => view! { <RepositoryStep /> }.into_any(),
        WizardStep::Agents => view! { <AgentsStep /> }.into_any(),
        WizardStep::Budget => view! { <BudgetStep /> }.into_any(),
        WizardStep::Phone => view! { <PhoneStep /> }.into_any(),
        WizardStep::DryRun => view! { <DryRunStep /> }.into_any(),
    }
}

#[component]
fn WizardFooter() -> impl IntoView {
    let wizard = use_wizard();
    view! {
        <div class="wiz-ft">
            <button
                class="btn"
                disabled=move || wizard.step.get() == WizardStep::Install
                on:click=move |_| wizard.back()
            >
                "Back"
            </button>
            <span style="flex:1"></span>
            <Show
                when=move || can_skip_now(wizard.step.get(), &wizard.state())
                fallback=|| ()
            >
                <button class="btn" on:click=move |_| wizard.skip()>"Skip for now"</button>
            </Show>
            <button
                class="btn pri"
                disabled=move || !can_continue(wizard.step.get(), &wizard.state())
                on:click=move |_| wizard.next()
            >
                {move || continue_label(wizard.step.get())}
            </button>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_paths_use_the_step_number() {
        assert_eq!(step_path(WizardStep::Phone), "/api/onboarding/4");
        assert_eq!(skip_path(WizardStep::DryRun), "/api/onboarding/skip/5");
    }

    #[test]
    fn skip_for_now_shows_on_install_until_it_is_met() {
        let empty = WizardState::default();
        assert!(can_skip_now(WizardStep::Install, &empty));
        assert!(!can_skip_now(WizardStep::Repository, &empty));
        assert!(can_skip_now(WizardStep::Phone, &empty));
    }

    #[test]
    fn the_last_step_says_finish_setup() {
        assert_eq!(continue_label(WizardStep::DryRun), "Finish setup");
        assert_eq!(continue_label(WizardStep::Budget), "Continue");
    }

    #[test]
    fn a_cap_shows_whole_cents() {
        assert_eq!(dollars_text(5_000_000), "5.00");
        assert_eq!(dollars_text(1_500_000), "1.50");
        assert_eq!(dollars_text(40_000), "0.04");
    }
}
