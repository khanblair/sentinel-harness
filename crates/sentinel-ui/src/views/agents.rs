//! The Agents panel: detected worker CLIs, and which agent serves each tier
//! (`docs/ui-rules.md`, Views; rule D28: no tier dropdowns in the tool list).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::RoutingUpdate;
use sentinel_core::{Routing, Tier, ToolId, ToolInfo, ToolStatus};

use crate::api;
use crate::components::tool_row::ToolRow;
use crate::components::top_bar::settle;
use crate::state::{Modal, Ui, use_ui};

/// The tiers in the routing section, easy first.
pub const TIERS: [Tier; 3] = [Tier::Cheap, Tier::Mid, Tier::Smart];

/// The words shown next to a tool. `ToolStatus` has no "disabled" state: that is the switch.
pub fn tool_status_label(status: ToolStatus) -> &'static str {
    match status {
        ToolStatus::Ready => "found",
        ToolStatus::NeedsLogin => "needs login",
        ToolStatus::NeedsCommand => "needs a command",
        ToolStatus::Missing => "missing",
        ToolStatus::Failed => "failed",
    }
}

/// Color token and shape name for a tool status. Ready is a dot, a fix needed is a ring.
pub fn tool_status_look(status: ToolStatus) -> (&'static str, &'static str) {
    match status {
        ToolStatus::Ready => ("ok", "dot"),
        ToolStatus::NeedsLogin | ToolStatus::NeedsCommand => ("warn", "ring"),
        ToolStatus::Missing | ToolStatus::Failed => ("danger", "oct"),
    }
}

/// The instruction the Fix button shows, or nothing when the tool needs no fix.
pub fn fix_instruction(tool: &ToolInfo) -> Option<String> {
    match tool.status {
        ToolStatus::NeedsLogin => Some(format!(
            "Sign in to {} in a terminal, then press Rescan.",
            tool.name
        )),
        ToolStatus::NeedsCommand => Some(format!(
            "{} needs a command template. Add it in Settings, then press Rescan.",
            tool.name
        )),
        ToolStatus::Ready | ToolStatus::Missing | ToolStatus::Failed => None,
    }
}

/// The tool summary line, such as `2 of 4 tools ready`.
pub fn ready_summary(tools: &[ToolInfo]) -> String {
    let ready = tools.iter().filter(|tool| tool.status.is_ready()).count();
    format!("{ready} of {} tools ready", tools.len())
}

/// The routing with one tier changed. `None` clears the tier.
pub fn with_tier(routing: &Routing, tier: Tier, agent: Option<ToolId>) -> Routing {
    let mut next = routing.clone();
    match tier {
        Tier::Cheap => next.cheap = agent,
        Tier::Mid => next.mid = agent,
        Tier::Smart => next.smart = agent,
    }
    next
}

/// Tools that can be picked for a tier: the ready ones, in list order.
pub fn routing_choices(tools: &[ToolInfo]) -> Vec<ToolId> {
    tools
        .iter()
        .filter(|tool| tool.status.is_ready())
        .map(|tool| tool.id.clone())
        .collect()
}

fn rescan(ui: Ui) {
    spawn_local(async move {
        let result = api::post_empty::<Vec<ToolInfo>>("/api/tools/scan").await;
        settle(ui, result);
    });
}

fn save_routing(ui: Ui, tier: Tier, value: String) {
    let agent = (!value.is_empty()).then(|| ToolId::new(value));
    let routing = ui
        .snapshot
        .with_untracked(|s| with_tier(&s.routing, tier, agent));
    let body = RoutingUpdate { routing };
    spawn_local(async move {
        let result = api::send::<_, serde_json::Value>("PUT", "/api/routing", &body).await;
        settle(ui, result);
    });
}

#[component]
fn RoutingRow(tier: Tier) -> impl IntoView {
    let ui = use_ui();
    let choices = move || ui.snapshot.with(|s| routing_choices(&s.tools));
    let current = move || {
        ui.snapshot
            .with(|s| s.routing.for_tier(tier).map(ToString::to_string))
            .unwrap_or_default()
    };
    view! {
        <div class="fld">
            <div class="l">
                <b>{tier.ui_label()}</b>
            </div>
            <select class="field" aria-label=format!("Agent for {} tasks", tier.ui_label())
                prop:value=current
                on:change=move |event| save_routing(ui, tier, event_target_value(&event))>
                <option value="">"None"</option>
                {move || choices().into_iter().map(|id| {
                    let value = id.to_string();
                    let label = value.clone();
                    view! { <option value=value>{label}</option> }
                }).collect_view()}
            </select>
        </div>
    }
}

#[component]
pub fn AgentsPanel() -> impl IntoView {
    let ui = use_ui();
    let tools = move || ui.snapshot.with(|s| s.tools.clone());
    view! {
        <div class="row sp" style="margin-bottom:.8em">
            <span class="sub">{move || ready_summary(&tools())}</span>
            <button class="btn sm" on:click=move |_| rescan(ui)>"Rescan"</button>
        </div>
        {move || tools().into_iter().map(|tool| view! { <ToolRow tool=tool /> }).collect_view()}
        <div class="row" style="margin-top:.8em">
            <button class="btn" on:click=move |_| ui.modal.set(Some(Modal::Settings { page: "tools".to_owned() }))>
                "Manage in settings"
            </button>
        </div>
        <div class="psec" style="margin-top:1em">
            <h5>"Which agent does what"</h5>
            {TIERS.into_iter().map(|tier| view! { <RoutingRow tier=tier /> }).collect_view()}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::ToolStatus;

    fn tool(status: ToolStatus) -> ToolInfo {
        ToolInfo {
            id: ToolId::new("claude"),
            name: "Claude".to_owned(),
            path: String::new(),
            version: "unknown".to_owned(),
            adapter: "claude".to_owned(),
            status,
            tier: Tier::Cheap,
            enabled: true,
        }
    }

    #[test]
    fn status_labels_match_the_four_states() {
        assert_eq!(tool_status_label(ToolStatus::Ready), "found");
        assert_eq!(tool_status_label(ToolStatus::NeedsLogin), "needs login");
        assert_eq!(
            tool_status_label(ToolStatus::NeedsCommand),
            "needs a command"
        );
        assert_eq!(tool_status_label(ToolStatus::Missing), "missing");
    }

    #[test]
    fn only_tools_that_need_the_user_get_a_fix() {
        assert!(fix_instruction(&tool(ToolStatus::NeedsLogin)).is_some());
        assert!(fix_instruction(&tool(ToolStatus::NeedsCommand)).is_some());
        assert_eq!(fix_instruction(&tool(ToolStatus::Missing)), None);
        assert_eq!(fix_instruction(&tool(ToolStatus::Ready)), None);
    }

    #[test]
    fn summary_counts_ready_tools() {
        let tools = vec![tool(ToolStatus::Ready), tool(ToolStatus::Missing)];
        assert_eq!(ready_summary(&tools), "1 of 2 tools ready");
    }

    #[test]
    fn with_tier_changes_only_that_tier() {
        let routing = Routing {
            cheap: Some(ToolId::new("a")),
            mid: None,
            smart: None,
        };
        let next = with_tier(&routing, Tier::Smart, Some(ToolId::new("b")));
        assert_eq!(next.cheap, Some(ToolId::new("a")));
        assert_eq!(next.smart, Some(ToolId::new("b")));
        assert_eq!(with_tier(&next, Tier::Cheap, None).cheap, None);
    }

    #[test]
    fn only_ready_tools_are_routing_choices() {
        let tools = vec![tool(ToolStatus::Ready), tool(ToolStatus::NeedsLogin)];
        assert_eq!(routing_choices(&tools), vec![ToolId::new("claude")]);
    }
}
