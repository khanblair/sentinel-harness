//! The dry run step of the setup wizard (`docs/product-scope.md`, F8 step 5). It says what the
//! dry run does, which agents it uses, and what it costs. It never touches the repository. In
//! this build the daemon does not run it yet, so the step shows the stages and says so.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::ToolInfo;

use crate::api::{self, ApiFailure};
use crate::components::dry_run_stages::DryRunStages;
use crate::components::icon::Icon;
use crate::state::{ToastKind, Ui, use_ui};
use crate::views::budget_step::ready_tools;

pub const DRY_RUN_UNAVAILABLE: &str = "The dry run is not available in this build yet.";

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

/// The toast text for a failed start. A 404 means this build has no dry run yet.
pub fn dry_run_message(error: &ApiFailure) -> String {
    match error {
        ApiFailure::Status { code: 404, .. } => DRY_RUN_UNAVAILABLE.to_string(),
        ApiFailure::Status { .. } | ApiFailure::Network(_) | ApiFailure::Decode(_) => {
            error.to_string()
        }
    }
}

fn start_dry_run(ui: Ui) {
    spawn_local(async move {
        match api::post_empty::<serde_json::Value>("/api/dry-run").await {
            Ok(_) => ui.show_toast("The dry run started.", ToastKind::Info),
            Err(error) => ui.show_toast(dry_run_message(&error), ToastKind::Danger),
        }
    });
}

#[component]
pub fn DryRunStep() -> impl IntoView {
    let ui = use_ui();
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
        <DryRunStages />
        <div class="row wrap" style="margin-top:1em">
            <button class="btn pri" on:click=move |_| start_dry_run(ui)>
                <Icon name="play" size=14 />
                " Start the dry run"
            </button>
            <span class="sub">"Not now? Press Skip for now. You can run it later from Settings."</span>
        </div>
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
}
