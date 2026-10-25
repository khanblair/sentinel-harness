//! The install check and the agents step of the setup wizard (`docs/product-scope.md`, F8 and
//! F18). The page cannot run installs: it copies the commands and the user runs them.

use std::collections::HashSet;

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::{DoctorReport, ToolToggle};
use sentinel_core::{ToolInfo, ToolStatus};

use crate::api;
use crate::components::agent_test_button::{AgentTestButton, AgentTestLine, TestSlot};
use crate::components::icon::Icon;
use crate::components::install_checklist::{InstallChecklist, all_found, missing_commands};
use crate::components::install_log::{InstallLog, LogLine, copy_text, install_lines};
use crate::components::onboarding_stepper::WizardStep;
use crate::components::repo_checks::CheckShape;
use crate::state::{ToastKind, Ui, refresh, use_ui};
use crate::views::onboarding::{Wizard, use_wizard};

/// Agents that must be ready before a full backlog runs, and from how many adapters.
const MIN_READY_AGENTS: usize = 3;
const MIN_ADAPTERS: usize = 2;

/// True when at least 3 enabled, ready tools come from 2 or more adapters.
pub fn enough_agents(tools: &[ToolInfo]) -> bool {
    let ready: Vec<&ToolInfo> = tools
        .iter()
        .filter(|tool| tool.enabled && tool.status.is_ready())
        .collect();
    let adapters: HashSet<&str> = ready.iter().map(|tool| tool.adapter.as_str()).collect();
    ready.len() >= MIN_READY_AGENTS && adapters.len() >= MIN_ADAPTERS
}

/// The text of a tool's status, as the list shows it.
pub fn tool_status_label(status: ToolStatus) -> &'static str {
    match status {
        ToolStatus::Ready => "Found",
        ToolStatus::NeedsLogin => "Needs login",
        ToolStatus::NeedsCommand => "Needs command",
        ToolStatus::Missing => "Missing",
        ToolStatus::Failed => "Failed",
    }
}

/// The color class of a tool's status: found is green, missing is red, the rest need action.
pub fn tool_status_class(status: ToolStatus) -> &'static str {
    match status {
        ToolStatus::Ready => "ok",
        ToolStatus::Missing => "danger",
        ToolStatus::NeedsLogin | ToolStatus::NeedsCommand | ToolStatus::Failed => "warn",
    }
}

/// The three tabs of the install step. They change only the `sentinel doctor` command shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsTab {
    Mac,
    Linux,
    Windows,
}

impl OsTab {
    pub const ALL: [OsTab; 3] = [OsTab::Mac, OsTab::Linux, OsTab::Windows];

    pub fn label(self) -> &'static str {
        match self {
            OsTab::Mac => "macOS",
            OsTab::Linux => "Linux",
            OsTab::Windows => "Windows",
        }
    }

    pub fn doctor_command(self) -> &'static str {
        match self {
            OsTab::Mac | OsTab::Linux => "./target/release/sentinel doctor",
            OsTab::Windows => ".\\target\\release\\sentinel.exe doctor",
        }
    }
}

/// The by-hand build steps, ending with the doctor command for the chosen tab.
pub fn build_commands(os: OsTab) -> Vec<String> {
    [
        "git clone <your-sentinel-repo-url> sentinel && cd sentinel",
        "rustup target add wasm32-unknown-unknown",
        "cargo install trunk cargo-nextest cargo-deny cargo-machete",
        "(cd crates/sentinel-ui && trunk build --release)",
        "cargo build --release -p sentinel-cli",
    ]
    .into_iter()
    .map(str::to_string)
    .chain(std::iter::once(os.doctor_command().to_string()))
    .collect()
}

/// Runs the install check and stores its rows. A failure is a toast; the old rows stay.
pub fn check_install(wizard: &Wizard) {
    let wizard = *wizard;
    spawn_local(async move {
        match api::post_empty::<DoctorReport>("/api/doctor").await {
            Ok(report) => wizard.doctor.set(Some(report)),
            Err(error) => wizard.ui.show_toast(error.to_string(), ToastKind::Danger),
        }
    });
}

#[component]
pub fn InstallStep() -> impl IntoView {
    let wizard = use_wizard();
    let os = RwSignal::new(OsTab::Mac);
    let lines = RwSignal::new(Vec::<LogLine>::new());
    Effect::new(move |_| {
        if wizard.doctor.with(Option::is_none) {
            check_install(&wizard);
        }
    });
    view! {
        <h2>"Install Sentinel"</h2>
        <p class="lead">
            "Sentinel is one program with the interface built in. First check what this computer already has, then install what is missing."
        </p>
        <OsTabs os=os />
        {move || {
            wizard
                .doctor
                .get()
                .map(|report| view! { <InstallChecklist rows=report.rows /> })
        }}
        <InstallActions lines=lines />
        <InstallLog lines=lines />
        <BuildByHand os=os />
    }
}

#[component]
fn OsTabs(os: RwSignal<OsTab>) -> impl IntoView {
    view! {
        <div class="os">
            {OsTab::ALL
                .into_iter()
                .map(|tab| {
                    view! {
                        <button
                            aria-pressed=move || (os.get() == tab).to_string()
                            on:click=move |_| os.set(tab)
                        >
                            {tab.label()}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}

/// The install and check-again buttons. Install copies the missing commands into the log.
#[component]
fn InstallActions(lines: RwSignal<Vec<LogLine>>) -> impl IntoView {
    let wizard = use_wizard();
    let installed = move || {
        wizard
            .doctor
            .with(|report| report.as_ref().is_some_and(|r| all_found(&r.rows)))
    };
    let install = move |_| {
        let commands = wizard
            .doctor
            .with(|report| report.as_ref().map(|r| missing_commands(&r.rows)))
            .unwrap_or_default();
        lines.set(install_lines(&commands));
        copy_text(wizard.ui, commands.join("\n"));
    };
    view! {
        <div class="row" style="margin:1em 0">
            <button class="btn pri" disabled=installed on:click=install>
                {move || if installed() { "Everything is installed" } else { "Install what is missing" }}
            </button>
            <button class="btn" on:click=move |_| check_install(&wizard)>
                <Icon name="refresh" size=13 />
                " Check again"
            </button>
        </div>
    }
}

#[component]
fn BuildByHand(os: RwSignal<OsTab>) -> impl IntoView {
    let wizard = use_wizard();
    view! {
        <details style="margin-top:1em">
            <summary>"Build from source by hand"</summary>
            {move || {
                build_commands(os.get())
                    .into_iter()
                    .map(|command| {
                        let copy = command.clone();
                        view! {
                            <div class="cmd">
                                <span>{command}</span>
                                <button
                                    class="ibtn"
                                    aria-label="Copy command"
                                    on:click=move |_| copy_text(wizard.ui, copy.clone())
                                >
                                    <Icon name="copy" size=14 />
                                </button>
                            </div>
                        }
                    })
                    .collect_view()
            }}
        </details>
    }
}

#[component]
pub fn AgentsStep() -> impl IntoView {
    let wizard = use_wizard();
    let ui = wizard.ui;
    let scanning = RwSignal::new(false);
    Effect::new(move |_| run_scan(ui, scanning, false));
    view! {
        <h2>"Agents on this PC"</h2>
        <p class="lead">
            "Sentinel looked for coding agents on this computer. Turn on the ones you want to use. To run a full backlog you need at least 3 ready agents, from at least 2 different tools."
        </p>
        <AgentCount scanning=scanning />
        <div class="card">
            {move || {
                ui.snapshot
                    .with(|s| s.tools.clone())
                    .into_iter()
                    .map(|tool| view! { <AgentToggleRow tool=tool /> })
                    .collect_view()
            }}
        </div>
        <Show
            when=move || !wizard.state().ready.contains(&WizardStep::Agents)
            fallback=|| ()
        >
            <button
                class="btn sm"
                style="margin-top:.8em"
                on:click=move |_| wizard.agents_override.set(true)
            >
                "Continue anyway"
            </button>
        </Show>
    }
}

/// The count line and the scan button.
#[component]
fn AgentCount(scanning: RwSignal<bool>) -> impl IntoView {
    let ui = use_ui();
    let ready = move || {
        ui.snapshot.with(|s| {
            s.tools
                .iter()
                .filter(|tool| tool.enabled && tool.status.is_ready())
                .count()
        })
    };
    view! {
        <div class="card" style="margin-bottom:.8em">
            <div class="row sp">
                <span class="row">
                    {move || {
                        let enough = ui.snapshot.with(|s| enough_agents(&s.tools));
                        view! { <Icon name=if enough { "check" } else { "alert" } size=15 /> }
                    }}
                    <span>{move || format!("{} turned on and ready", ready())}</span>
                </span>
                <ScanButton ui=ui scanning=scanning idle="Scan again" />
            </div>
        </div>
    }
}

/// The scan button. It reads Scanning, with the refresh icon, until the call ends.
#[component]
pub fn ScanButton(ui: Ui, scanning: RwSignal<bool>, idle: &'static str) -> impl IntoView {
    view! {
        <button class="btn sm" disabled=move || scanning.get() on:click=move |_| scan_tools(ui, scanning)>
            <Icon name="refresh" size=13 />
            {move || format!(" {}", if scanning.get() { "Scanning" } else { idle })}
        </button>
    }
}

/// The toast text after a scan: how many agents are ready and how many are missing.
pub fn scan_summary(tools: &[ToolInfo]) -> String {
    if tools.is_empty() {
        return "Scanned. No agents found.".to_owned();
    }
    let ready = tools.iter().filter(|tool| tool.status.is_ready()).count();
    let missing = tools
        .iter()
        .filter(|tool| tool.status == ToolStatus::Missing)
        .count();
    let word = if ready == 1 { "agent" } else { "agents" };
    format!("Scanned. {ready} {word} ready, {missing} missing.")
}

/// The scan button's action: a user scan announces the counts, the mount scan stays quiet.
/// The snapshot reloads after a good answer, and a failure shows as a Danger toast.
fn run_scan(ui: Ui, scanning: RwSignal<bool>, announce: bool) {
    scanning.set(true);
    spawn_local(async move {
        let result = api::post_empty::<Vec<ToolInfo>>("/api/tools/scan").await;
        scanning.set(false);
        finish_scan(ui, result, announce);
    });
}

fn finish_scan(ui: Ui, result: Result<Vec<ToolInfo>, api::ApiFailure>, announce: bool) {
    let tools = match result {
        Ok(tools) => tools,
        Err(error) => {
            ui.show_toast(error.to_string(), ToastKind::Danger);
            return;
        }
    };
    refresh(ui);
    if announce {
        ui.show_toast(scan_summary(&tools), ToastKind::Info);
    }
}

/// Searches for agents and shows the counts. The wizard, Settings, and the budget page share it.
pub fn scan_tools(ui: Ui, scanning: RwSignal<bool>) {
    run_scan(ui, scanning, true);
}

/// One agent: its status shape, name, path, and the switch that turns it on or off.
#[component]
fn AgentToggleRow(tool: ToolInfo) -> impl IntoView {
    let ui = use_ui();
    let class = tool_status_class(tool.status);
    let path = if tool.path.is_empty() {
        "not found".to_string()
    } else {
        tool.path.clone()
    };
    let toggle_id = tool.id.as_str().to_string();
    let next = !tool.enabled;
    let on_toggle = move |_| toggle_tool(ui, toggle_id.clone(), next);
    let test = TestSlot {
        running: RwSignal::new(false),
        outcome: RwSignal::new(None),
    };
    let status = tool.status;
    view! {
        <div class="chk">
            <CheckShape class=class />
            <div class="n">
                <b style="font-weight:500">{tool.name.clone()}</b>
                <small class="mono">{path}</small>
            </div>
            <span class="sub">{tool_status_label(tool.status)}</span>
            <AgentTestButton tool=tool.clone() test=test />
            <button
                class="tog"
                role="switch"
                aria-checked=tool.enabled.to_string()
                aria-label=format!("Use {}", tool.name)
                disabled=!tool.status.is_ready()
                on:click=on_toggle
            ></button>
        </div>
        <AgentTestLine test=test status=status />
    }
}

fn toggle_tool(ui: Ui, id: String, enabled: bool) {
    spawn_local(async move {
        let path = format!("/api/tools/{id}");
        let body = ToolToggle { enabled };
        match api::send::<_, serde_json::Value>("PUT", &path, &body).await {
            Ok(_) => refresh(ui),
            Err(error) => ui.show_toast(error.to_string(), ToastKind::Danger),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{Tier, ToolId, ToolInfo, ToolStatus};

    fn tool(adapter: &str, status: ToolStatus, enabled: bool) -> ToolInfo {
        ToolInfo {
            id: ToolId::new(format!("{adapter}-{enabled}-{status:?}")),
            name: adapter.to_string(),
            path: String::new(),
            version: String::new(),
            adapter: adapter.to_string(),
            status,
            tier: Tier::Mid,
            enabled,
        }
    }

    #[test]
    fn three_ready_agents_from_one_adapter_are_not_enough() {
        let tools = [
            tool("claude", ToolStatus::Ready, true),
            tool("claude", ToolStatus::Ready, true),
            tool("claude", ToolStatus::Ready, true),
        ];
        assert!(!enough_agents(&tools));
    }

    #[test]
    fn three_ready_agents_from_two_adapters_are_enough() {
        let tools = [
            tool("claude", ToolStatus::Ready, true),
            tool("claude", ToolStatus::Ready, true),
            tool("codex", ToolStatus::Ready, true),
        ];
        assert!(enough_agents(&tools));
    }

    #[test]
    fn a_disabled_or_unready_agent_does_not_count() {
        let tools = [
            tool("claude", ToolStatus::Ready, true),
            tool("codex", ToolStatus::Ready, false),
            tool("gemini", ToolStatus::NeedsLogin, true),
        ];
        assert!(!enough_agents(&tools));
    }

    #[test]
    fn a_missing_tool_has_a_danger_color() {
        assert_eq!(tool_status_class(ToolStatus::Missing), "danger");
        assert_eq!(tool_status_label(ToolStatus::NeedsCommand), "Needs command");
    }

    #[test]
    fn a_scan_with_no_agents_says_none_were_found() {
        assert_eq!(scan_summary(&[]), "Scanned. No agents found.");
    }

    #[test]
    fn a_scan_counts_ready_and_missing_agents() {
        let mut tools = vec![
            tool("claude", ToolStatus::Ready, true),
            tool("codex", ToolStatus::Ready, true),
            tool("gemini", ToolStatus::Ready, true),
            tool("aider", ToolStatus::Missing, false),
            tool("goose", ToolStatus::NeedsLogin, false),
        ];
        assert_eq!(scan_summary(&tools), "Scanned. 3 agents ready, 1 missing.");
        tools.truncate(1);
        assert_eq!(scan_summary(&tools), "Scanned. 1 agent ready, 0 missing.");
    }

    #[test]
    fn build_steps_end_with_the_doctor_command_for_the_tab() {
        let commands = build_commands(OsTab::Windows);
        assert_eq!(
            commands.last().map(String::as_str),
            Some(".\\target\\release\\sentinel.exe doctor")
        );
        assert_eq!(commands.len(), 6);
    }
}
