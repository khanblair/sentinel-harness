//! Menu bodies for the one popover. Each entry calls into `Ui` or the daemon API, then closes
//! the menu.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::{PermissionsUpdate, SetBrainMode};
use sentinel_core::{BrainMode, PermissionMode, Snapshot, ToolId};

use crate::api;
use crate::components::agent_stack::{avatar_color, avatar_letter, ready_agents};
use crate::components::icon::Icon;
use crate::components::panel_host::panel_icon;
use crate::components::top_bar::{post_action, spend_label};
use crate::layout::PanelId;
use crate::state::{Modal, PopoverKind, Target, ToastKind, Ui};

/// One popover body per kind.
pub fn body(kind: PopoverKind, ui: Ui) -> AnyView {
    match kind {
        PopoverKind::Title => title_menu(ui).into_any(),
        PopoverKind::Agents => agents_menu(ui).into_any(),
        PopoverKind::Permission => permission_menu(ui).into_any(),
        PopoverKind::Brain => brain_menu(ui).into_any(),
        PopoverKind::Target => target_menu(ui).into_any(),
        PopoverKind::More => more_menu(ui).into_any(),
        PopoverKind::Panel => panel_menu(ui).into_any(),
        PopoverKind::Attach => attach_menu(ui).into_any(),
    }
}

/// One entry: an icon, a label, an optional description, and a tick for the current choice.
#[component]
pub fn MenuItem(
    icon: &'static str,
    label: &'static str,
    #[prop(default = "")] desc: &'static str,
    #[prop(default = false)] danger: bool,
    #[prop(default = false)] ticked: bool,
    #[prop(into)] on_pick: Callback<()>,
) -> impl IntoView {
    let class = if danger { "mi dng" } else { "mi" };
    view! {
        <button class=class on:click=move |_| on_pick.run(())>
            <Icon name=icon size=16 />
            <span>
                <span>{label}{ticked.then(tick_mark)}</span>
                {(!desc.is_empty()).then(|| view! { <div class="d">{desc}</div> })}
            </span>
        </button>
    }
}

fn tick_mark() -> impl IntoView {
    view! { <span class="tk"><Icon name="check" size=15 /></span> }
}

fn close(ui: Ui) {
    ui.popover.set(None);
}

fn pick_panel(ui: Ui, panel: PanelId) {
    close(ui);
    ui.open_panel(panel);
}

/// One agent in the Agents menu or the Target menu.
struct AgentRow {
    id: ToolId,
    name: String,
    detail: String,
    letter: char,
    color: &'static str,
}

fn agent_rows(snapshot: &Snapshot) -> Vec<AgentRow> {
    ready_agents(snapshot)
        .into_iter()
        .map(|tool| AgentRow {
            id: tool.id.clone(),
            name: tool.name.clone(),
            detail: format!(
                "{} tasks · {}",
                tool.tier.ui_label(),
                agent_spend(snapshot, &tool.id)
            ),
            letter: avatar_letter(&tool.id),
            color: avatar_color(&tool.id),
        })
        .collect()
}

fn agent_spend(snapshot: &Snapshot, agent: &ToolId) -> String {
    snapshot
        .per_agent_cost
        .iter()
        .find(|entry| entry.agent == *agent)
        .map_or_else(
            || spend_label(sentinel_core::Cost::ZERO),
            |entry| spend_label(entry.cost),
        )
}

fn title_menu(ui: Ui) -> impl IntoView {
    let live = ui.snapshot.with(crate::components::sidebar::is_live);
    view! {
        {live.then(|| view! {
            <MenuItem icon="stop" label="Stop run" desc="Checkpoints are kept" danger=true
                on_pick=move |()| { close(ui); post_action(ui, "/api/stop".to_owned()) } />
            <div class="sep"></div>
        })}
        <MenuItem icon="download" label="Export report"
            on_pick=move |()| pick_panel(ui, PanelId::Report) />
    }
}

fn agents_menu(ui: Ui) -> impl IntoView {
    let rows = ui.snapshot.with(agent_rows);
    view! {
        <div class="mi" style="cursor:default">
            <span>
                <b style="font-weight:600">"Agents in this run"</b>
                <div class="d">"Switch what the stream shows, or step in."</div>
            </span>
        </div>
        <MenuItem icon="agent" label="All agents" desc="Show every event"
            on_pick=move |()| { close(ui); ui.focus.set(None) } />
        <div class="sep"></div>
        {rows.into_iter().map(|row| agent_row(ui, row)).collect_view()}
        <div class="sep"></div>
        <MenuItem icon="gear" label="Manage agents" desc="Detected CLIs and tier routing"
            on_pick=move |()| settings(ui, "agents") />
    }
}

fn agent_row(ui: Ui, row: AgentRow) -> impl IntoView {
    let focus_id = row.id.clone();
    let kill_id = row.id;
    view! {
        <div class="mi" style="cursor:default;align-items:center">
            <span class="av-a" style=format!("background:{}", row.color)>{row.letter}</span>
            <span style="flex:1;min-width:0">
                <b style="font-weight:500">{row.name}</b>
                <div class="d">{row.detail}</div>
            </span>
            <button class="ibtn" aria-label="Focus stream" data-tip="Focus stream"
                on:click=move |_| { close(ui); ui.focus.set(Some(focus_id.clone())) }>
                <Icon name="eye" size=15 />
            </button>
            <button class="ibtn" aria-label="Open terminal" data-tip="Terminal"
                on:click=move |_| pick_panel(ui, PanelId::Terminal)>
                <Icon name="terminal" size=15 />
            </button>
            <button class="ibtn" aria-label="Kill agent" data-tip="Kill" style="color:var(--danger)"
                on:click=move |_| { close(ui); post_action(ui, format!("/api/agents/{}/kill", kill_id.as_str())) }>
                <Icon name="stop" size=14 />
            </button>
        </div>
    }
}

const PERMISSION_CHOICES: [(PermissionMode, &str, &str); 4] = [
    (
        PermissionMode::PlanOnly,
        "eye",
        "Agents read and plan. Nothing is written until you approve the plan.",
    ),
    (
        PermissionMode::AskEachWave,
        "shield",
        "You approve every wave and every scope extension before agents write.",
    ),
    (
        PermissionMode::AutoInsideReserved,
        "lock",
        "Writes to files a task has reserved run without asking. Asking for more files needs your OK.",
    ),
    (
        PermissionMode::Bypass,
        "alert",
        "No approval cards. Free files are granted on request. Locks and the test-edit guard still apply.",
    ),
];

fn permission_menu(ui: Ui) -> impl IntoView {
    let current = ui.snapshot.with(|snapshot| snapshot.permission_mode);
    view! {
        {PERMISSION_CHOICES
            .into_iter()
            .map(|(mode, icon, desc)| view! {
                <MenuItem icon=icon label=mode.label() desc=desc ticked={current == mode}
                    on_pick=move |()| pick_permission(ui, mode) />
            })
            .collect_view()}
        <div class="sep"></div>
        <MenuItem icon="shield" label="Per-agent permissions"
            on_pick=move |()| pick_panel(ui, PanelId::Permissions) />
        <div class="d" style="padding:.3em .65em .4em;color:var(--muted);font-size:.85em">
            "Agents can only write files their task has reserved."
        </div>
    }
}

fn pick_permission(ui: Ui, mode: PermissionMode) {
    close(ui);
    if mode == PermissionMode::Bypass {
        ui.modal.set(Some(Modal::BypassConfirm));
        return;
    }
    let update = PermissionsUpdate {
        mode: Some(mode),
        ..PermissionsUpdate::default()
    };
    spawn_local(async move {
        let result = api::send::<_, serde_json::Value>("PUT", "/api/permissions", &update).await;
        crate::components::top_bar::settle(ui, result);
    });
}

const BRAIN_CHOICES: [(BrainMode, &str, &str); 3] = [
    (BrainMode::Off, "off", "No notes read or written."),
    (
        BrainMode::ReadOnly,
        "read-only",
        "Agents read notes. Nothing is written. Best for live runs.",
    ),
    (
        BrainMode::Learn,
        "learn",
        "Agents read notes. Each run writes a journal and lesson candidates.",
    ),
];

fn brain_menu(ui: Ui) -> impl IntoView {
    let current = ui.snapshot.with(|snapshot| snapshot.brain_mode);
    view! {
        {BRAIN_CHOICES
            .into_iter()
            .map(|(mode, label, desc)| view! {
                <MenuItem icon="brain" label=label desc=desc ticked={current == mode}
                    on_pick=move |()| pick_brain(ui, mode) />
            })
            .collect_view()}
        <div class="sep"></div>
        <MenuItem icon="brain" label="Open the Brain" on_pick=move |()| pick_panel(ui, PanelId::Brain) />
        <MenuItem icon="clock" label="Brain care schedule" on_pick=move |()| settings(ui, "brain") />
    }
}

fn pick_brain(ui: Ui, mode: BrainMode) {
    close(ui);
    let request = SetBrainMode { mode };
    spawn_local(async move {
        let result = api::post::<_, serde_json::Value>("/api/brain/mode", &request).await;
        crate::components::top_bar::settle(ui, result);
    });
}

fn target_menu(ui: Ui) -> impl IntoView {
    let rows = ui.snapshot.with(agent_rows);
    view! {
        <MenuItem icon="shield" label="Sentinel"
            desc="Routes by tier: cheap first, escalate on failure."
            ticked={ui.target.get() == Target::Sentinel}
            on_pick=move |()| pick_target(ui, Target::Sentinel) />
        <div class="sep"></div>
        {rows
            .into_iter()
            .map(|row| target_row(ui, row))
            .collect_view()}
        <div class="sep"></div>
        <MenuItem icon="sliders" label="Task routing"
            desc="Choose which agent handles easy, normal, and hard tasks"
            on_pick=move |()| settings(ui, "agents") />
    }
}

fn target_row(ui: Ui, row: AgentRow) -> impl IntoView {
    let target = Target::Agent(row.id.clone());
    let ticked = ui.target.get() == target;
    view! {
        <button class="mi" on:click=move |_| pick_target(ui, target.clone())>
            <span class="av-a" style=format!("background:{}", row.color)>{row.letter}</span>
            <span>
                <span>{row.name}{ticked.then(tick_mark)}</span>
                <div class="d">{row.detail}</div>
            </span>
        </button>
    }
}

fn pick_target(ui: Ui, target: Target) {
    close(ui);
    ui.target.set(target);
}

fn more_menu(ui: Ui) -> impl IntoView {
    view! {
        <MenuItem icon="dollar" label="Cost" on_pick=move |()| pick_panel(ui, PanelId::Cost) />
        <MenuItem icon="list" label="Report" on_pick=move |()| pick_panel(ui, PanelId::Report) />
        <MenuItem icon="brain" label="Brain" on_pick=move |()| pick_panel(ui, PanelId::Brain) />
        <MenuItem icon="send" label="Telegram" on_pick=move |()| pick_panel(ui, PanelId::Telegram) />
        <MenuItem icon="folder" label="Files" on_pick=move |()| pick_panel(ui, PanelId::Files) />
        <MenuItem icon="agent" label="Agents" on_pick=move |()| pick_panel(ui, PanelId::Agents) />
        <MenuItem icon="shield" label="Permissions" on_pick=move |()| pick_panel(ui, PanelId::Permissions) />
        <div class="sep"></div>
        <MenuItem icon="book" label="How to use Sentinel"
            on_pick=move |()| { close(ui); ui.modal.set(Some(Modal::Guide)) } />
        <MenuItem icon="search" label="Command palette" desc="Ctrl or Cmd + K"
            on_pick=move |()| { close(ui); ui.modal.set(Some(Modal::Palette)) } />
        <MenuItem icon="gear" label="Settings" on_pick=move |()| settings(ui, "general") />
    }
}

fn panel_menu(ui: Ui) -> impl IntoView {
    PanelId::ALL
        .into_iter()
        .map(|panel| {
            let ticked = ui.panel_tabs.with(|tabs| tabs.contains(&panel));
            view! {
                <MenuItem icon=panel_icon(panel) label=panel.label() ticked=ticked
                    on_pick=move |()| pick_panel(ui, panel) />
            }
        })
        .collect_view()
}

fn attach_menu(ui: Ui) -> impl IntoView {
    view! {
        <MenuItem icon="file" label="Attach a backlog file"
            on_pick=move |()| unavailable(ui, "Attaching a file is not available yet.") />
        <MenuItem icon="book" label="Add a context note" desc="Goes into every agent prompt"
            on_pick=move |()| unavailable(ui, "Context notes are not available yet.") />
    }
}

fn unavailable(ui: Ui, text: &str) {
    close(ui);
    ui.show_toast(text, ToastKind::Info);
}

fn settings(ui: Ui, page: &str) {
    close(ui);
    ui.modal.set(Some(Modal::Settings {
        page: page.to_owned(),
    }));
}
