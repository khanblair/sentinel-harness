//! The dry run step of the setup wizard (`docs/product-scope.md`, F8 step 5). It says what the
//! dry run does, which agents it uses, and what it costs. It never touches the repository. The
//! stages and the result come from `GET /api/dry-run`, which the page reads every second while a
//! run goes on.

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::ToolInfo;
use sentinel_core::dry_run::{DryRunStageRow, DryRunStatus};

use crate::api::{self, ApiFailure};
use crate::components::dry_run_stages::DryRunStages;
use crate::components::icon::Icon;
use crate::dry_run::{outcome_text, result_text};
use crate::state::{ToastKind, Ui, use_ui};
use crate::views::budget_step::ready_tools;
use crate::views::onboarding::use_wizard;

pub const DRY_RUN_UNAVAILABLE: &str = "The dry run is not available in this build yet.";

/// How often the page reads the status while the step is open.
const POLL_MS: u32 = 1_000;

/// The agents the dry run uses: the first two ready ones, or a plain phrase when none are ready.
pub fn agents_line(tools: &[ToolInfo]) -> String {
    let names: Vec<String> = ready_tools(tools)
        .into_iter()
        .take(2)
        .map(|tool| tool.name)
        .collect();
    if names.is_empty() {
        "Your turned-on agents".to_string()
    } else {
        names.join(" and ")
    }
}

/// The toast text for a failed start. A 404 means this build has no dry run yet. Other failures
/// show the daemon's own sentence.
pub fn dry_run_message(error: &ApiFailure) -> String {
    match error {
        ApiFailure::Status { code: 404, .. } => DRY_RUN_UNAVAILABLE.to_string(),
        ApiFailure::Status { message, .. } => message.clone(),
        ApiFailure::Network(_) | ApiFailure::Decode(_) => error.to_string(),
    }
}

/// The stage rows the page shows. Before the first answer there are none, so every stage is
/// shown as not started.
fn stage_rows(current: Option<&DryRunStatus>) -> Vec<DryRunStageRow> {
    current.map_or_else(Vec::new, |status| status.stages.clone())
}

fn is_running(current: Option<&DryRunStatus>) -> bool {
    current.is_some_and(|status| status.running)
}

/// The verified and total counts, once the daemon has counted them.
fn counts(current: Option<&DryRunStatus>) -> Option<(u32, u32)> {
    current.and_then(|status| status.verified.zip(status.total))
}

/// Reads the status. An error or a 404 keeps the page as it is: the static stages.
fn load_status(status: RwSignal<Option<DryRunStatus>>) {
    spawn_local(async move {
        if let Ok(fetched) = api::get::<DryRunStatus>("/api/dry-run").await {
            status.set(Some(fetched));
        }
    });
}

/// Reads the status every second while a run goes on. The loop ends with the step.
fn poll_while_running(status: RwSignal<Option<DryRunStatus>>, alive: RwSignal<bool>) {
    spawn_local(async move {
        loop {
            TimeoutFuture::new(POLL_MS).await;
            if alive.try_get_untracked() != Some(true) {
                break;
            }
            if status.with_untracked(|current| is_running(current.as_ref())) {
                load_status(status);
            }
        }
    });
}

fn start_dry_run(ui: Ui, status: RwSignal<Option<DryRunStatus>>) {
    spawn_local(async move {
        match api::post_empty::<serde_json::Value>("/api/dry-run").await {
            Ok(_) => mark_started(ui, status),
            Err(error) => ui.show_toast(dry_run_message(&error), ToastKind::Danger),
        }
    });
}

fn mark_started(ui: Ui, status: RwSignal<Option<DryRunStatus>>) {
    status.set(Some(DryRunStatus {
        running: true,
        stages: Vec::new(),
        verified: None,
        total: None,
    }));
    ui.show_toast("The dry run started.", ToastKind::Info);
}

#[component]
fn StartRow(ui: Ui, status: RwSignal<Option<DryRunStatus>>) -> impl IntoView {
    view! {
        <div class="row wrap" style="margin-top:1em">
            <button
                class="btn pri"
                disabled=move || status.with(|current| is_running(current.as_ref()))
                on:click=move |_| start_dry_run(ui, status)
            >
                <Icon name="play" size=14 />
                " Start the dry run"
            </button>
            <span class="sub">"Not now? Press Skip for now. You can run it later from Settings."</span>
        </div>
    }
}

#[component]
fn ResultCard(ui: Ui, status: RwSignal<Option<DryRunStatus>>) -> impl IntoView {
    view! {
        <div class="card" style="margin-top:.8em">
            <div class="row">
                <Icon name="check" size=16 />
                <b>{move || status.with(|current| counts(current.as_ref()).map(|(v, t)| result_text(v, t)).unwrap_or_default())}</b>
            </div>
            <p style="margin:.5em 0 0">"Your repository was not touched."</p>
            <div class="row wrap" style="margin-top:1em">
                <button class="btn" on:click=move |_| start_dry_run(ui, status)>"Start again"</button>
            </div>
        </div>
    }
}

#[component]
fn FailedCard(ui: Ui, status: RwSignal<Option<DryRunStatus>>) -> impl IntoView {
    view! {
        <div class="card" style="margin-top:.8em">
            <b>{move || status.with(|current| current.as_ref().and_then(outcome_text).unwrap_or_default())}</b>
            <div class="row wrap" style="margin-top:1em">
                <button class="btn" on:click=move |_| start_dry_run(ui, status)>"Start again"</button>
            </div>
        </div>
    }
}

/// The start button while no outcome shows. Once the run ends, the result card or the failure
/// card takes its place. Memos keep the button from being rebuilt on every poll.
#[component]
fn DryRunActions(ui: Ui, status: RwSignal<Option<DryRunStatus>>) -> impl IntoView {
    let finished = Memo::new(move |_| {
        status.with(|current| current.as_ref().and_then(outcome_text).is_some())
    });
    let counted = Memo::new(move |_| status.with(|current| counts(current.as_ref()).is_some()));
    view! {
        {move || (!finished.get()).then(|| view! { <StartRow ui=ui status=status /> })}
        {move || {
            finished
                .get()
                .then(|| {
                    if counted.get() {
                        view! { <ResultCard ui=ui status=status /> }.into_any()
                    } else {
                        view! { <FailedCard ui=ui status=status /> }.into_any()
                    }
                })
        }}
    }
}

#[component]
pub fn DryRunStep() -> impl IntoView {
    let ui = use_ui();
    let wizard = use_wizard();
    let status = RwSignal::new(None::<DryRunStatus>);
    let alive = RwSignal::new(true);
    on_cleanup(move || {
        alive.try_set(false);
    });
    load_status(status);
    poll_while_running(status, alive);
    Effect::new(move |_| {
        if status.with(|current| counts(current.as_ref()).is_some()) {
            wizard.dry_run_done.set(true);
        }
    });
    view! {
        <h2>"Dry run on a sample project"</h2>
        <p class="lead">
            "A short rehearsal before you spend real budget. It shows that your agents start, that file locks work, that tests run, and that Sentinel reports honestly."
        </p>
        <div class="card" style="margin-bottom:.8em">
            <b>"What it uses"</b>
            <ul style="margin:.4em 0 0 1.1em;padding:0;line-height:1.7">
                <li>"A tiny sample project that ships with Sentinel. " <b>"Your repository is not touched."</b></li>
                <li>{move || format!("{}, working at the same time.", ui.snapshot.with(|s| agents_line(&s.tools)))}</li>
                <li>
                    "About " <b>"$0.04"</b> " of your "
                    {move || ui.snapshot.with(|s| s.cost.cap.to_string())}
                    " budget."
                </li>
                <li>"About 15 seconds."</li>
            </ul>
        </div>
        {move || view! { <DryRunStages rows=status.with(|current| stage_rows(current.as_ref())) /> }}
        <DryRunActions ui=ui status=status />
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{Tier, ToolId, ToolStatus};

    fn tool(name: &str, status: ToolStatus) -> ToolInfo {
        ToolInfo {
            id: ToolId::new(name),
            name: name.to_string(),
            path: String::new(),
            version: String::new(),
            adapter: name.to_string(),
            status,
            tier: Tier::Mid,
            enabled: true,
        }
    }

    #[test]
    fn the_dry_run_names_the_first_two_ready_agents() {
        let tools = [
            tool("claude", ToolStatus::Ready),
            tool("gemini", ToolStatus::NeedsLogin),
            tool("codex", ToolStatus::Ready),
            tool("opencode", ToolStatus::Ready),
        ];
        assert_eq!(agents_line(&tools), "claude and codex");
    }

    #[test]
    fn with_no_ready_agents_the_dry_run_names_no_tool() {
        assert_eq!(agents_line(&[]), "Your turned-on agents");
    }

    #[test]
    fn a_404_says_the_dry_run_is_not_in_this_build() {
        let error = ApiFailure::Status {
            code: 404,
            message: "not found".to_string(),
        };
        assert_eq!(dry_run_message(&error), DRY_RUN_UNAVAILABLE);
    }

    #[test]
    fn other_failures_keep_the_daemon_message() {
        let error = ApiFailure::Status {
            code: 500,
            message: "disk full".to_string(),
        };
        assert!(dry_run_message(&error).contains("disk full"));
    }

    #[test]
    fn a_conflict_shows_the_daemon_sentence_without_the_status_code() {
        let error = ApiFailure::Status {
            code: 409,
            message: "A dry run is already going".to_string(),
        };
        assert_eq!(dry_run_message(&error), "A dry run is already going");
    }
}
