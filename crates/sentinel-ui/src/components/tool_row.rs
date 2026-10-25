//! `ToolRow`: one detected worker CLI with its status, path, version, enable switch, and a fix
//! action when the tool needs the user (`docs/ui-registry.md`, Tools).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::ToolToggle;
use sentinel_core::{ToolInfo, ToolStatus};

use crate::api;
use crate::components::agent_test_button::{AgentTestButton, AgentTestLine};
use crate::components::status_badge::shape_markup;
use crate::components::top_bar::settle;
use crate::state::{ToastKind, Ui, use_ui};
use crate::views::agents::{fix_instruction, tool_status_label, tool_status_look};

fn status_line(status: ToolStatus) -> impl IntoView {
    let (color, shape) = tool_status_look(status);
    let label = tool_status_label(status);
    view! {
        <span class="stb" style=format!("color:var(--{color})")>
            <svg class="shp" viewBox="0 0 12 12" aria-hidden="true" inner_html=shape_markup(shape, color)></svg>
            <span>{label}</span>
        </span>
    }
}

/// The top line of a row: the name, then the status and the Test button.
#[component]
fn ToolHead(tool: ToolInfo) -> impl IntoView {
    let name = tool.name.clone();
    let status = status_line(tool.status);
    view! {
        <div class="row sp">
            <span class="row"><b>{name}</b></span>
            <span class="row" style="gap:.5em">
                {status}
                <AgentTestButton id=tool.id.clone() status=tool.status />
            </span>
        </div>
    }
}

fn toggle_tool(ui: Ui, tool: &ToolInfo) {
    let path = format!("/api/tools/{}", tool.id);
    let body = ToolToggle {
        enabled: !tool.enabled,
    };
    spawn_local(async move {
        let result = api::send::<_, serde_json::Value>("PUT", &path, &body).await;
        settle(ui, result);
    });
}

#[component]
pub fn ToolRow(tool: ToolInfo) -> impl IntoView {
    let ui = use_ui();
    let fix = fix_instruction(&tool);
    let path = if tool.path.is_empty() {
        "not found on this PC".to_owned()
    } else {
        tool.path.clone()
    };
    let version =
        (tool.version != "unknown" && !tool.version.is_empty()).then(|| tool.version.clone());
    let enabled = tool.enabled;
    let name = tool.name.clone();
    let head = tool.clone();
    let line_id = tool.id.clone();
    let status = tool.status;
    view! {
        <div class="tcard">
            <ToolHead tool=head />
            <div class="meta">
                <span class="mono">{path}</span>
                {version.map(|text| view! { <span>{text}</span> })}
            </div>
            <div class="row wrap" style="gap:.5em;margin-top:.5em">
                <button class="tog" role="switch" aria-checked=enabled.to_string()
                    aria-label=format!("Use {name}")
                    on:click=move |_| toggle_tool(ui, &tool)></button>
                {fix.map(|text| view! {
                    <button class="btn sm" on:click=move |_| ui.show_toast(text.clone(), ToastKind::Warn)>
                        "Fix"
                    </button>
                })}
            </div>
            <AgentTestLine id=line_id status=status />
        </div>
    }
}
