//! The Agents and tools page, the Budget page, and the routing choices they share. The cap can only
//! go down, so the lower-cap check here is the only path to a new cap.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::{LowerCap, RoutingUpdate, ToolToggle};
use sentinel_core::{Cost, Routing, Snapshot, Tier, ToolId, ToolInfo, ToolStatus};

use crate::api;
use crate::components::status_badge::shape_markup;
use crate::components::toggle::Toggle;
use crate::components::top_bar::settle;
use crate::state::{ToastKind, Ui, use_ui};
use crate::views::install::ScanButton;
use crate::views::settings::Field;

/// The three routing rows of Settings, Agents and tools: the tier, its label, and its hint.
const TIER_ROWS: [(Tier, &str, &str); 3] = [
    (Tier::Cheap, "Easy tasks", "Tried first. Keeps cost low."),
    (Tier::Mid, "Normal tasks", "Used when an easy task fails."),
    (Tier::Smart, "Hard tasks", "Hard work, and the last resort."),
];

/// True when `text` is a dollar amount above zero and strictly below the current cap. The daemon
/// refuses a raise too (`POST /api/cap/lower`), so this check only keeps the form honest.
pub fn lower_cap_valid(text: &str, current: Cost) -> bool {
    match Cost::parse_usd(text) {
        Ok(next) => next.micro_usd() > 0 && next.micro_usd() < current.micro_usd(),
        Err(_) => false,
    }
}

/// The routing with one tier changed. The other two tiers keep their agents.
pub fn routing_with(routing: &Routing, tier: Tier, tool: Option<ToolId>) -> Routing {
    let mut next = routing.clone();
    match tier {
        Tier::Cheap => next.cheap = tool,
        Tier::Mid => next.mid = tool,
        Tier::Smart => next.smart = tool,
    }
    next
}

/// Tools that are found and ready. Only a ready tool can take work, so only these can be picked.
pub fn ready_tools(tools: &[ToolInfo]) -> Vec<&ToolInfo> {
    tools
        .iter()
        .filter(|tool| matches!(tool.status, ToolStatus::Ready))
        .collect()
}

#[component]
pub fn BudgetPage() -> impl IntoView {
    let ui = use_ui();
    let cap = move || ui.snapshot.get().cost.cap.to_string();
    view! {
        <h2>"Budget"</h2>
        <p class="lead">"Spend limits."</p>
        <Field label="Budget cap" hint="The cap can only be lowered.">
            <span class="k">{cap}</span>
        </Field>
        <LowerCapField />
        <Field label="Stop starting new tasks at" hint="Fixed rule.">
            <span class="k">"70%"</span>
        </Field>
        <Field label="Kill all workers at" hint="Fixed rule.">
            <span class="k">"95%"</span>
        </Field>
        <Field label="Per-task cost ceiling" hint="A task over its ceiling is killed, then escalated or blocked.">
            <select disabled aria-label="Per-task cost ceiling">
                <option value="1.5">"1.5x estimate"</option>
                <option value="2" selected=true>"2x estimate"</option>
                <option value="3">"3x estimate"</option>
            </select>
        </Field>
        <RoutingFields />
    }
}

/// The form that lowers the cap. Only a smaller amount is sent.
#[component]
fn LowerCapField() -> impl IntoView {
    let ui = use_ui();
    let draft = RwSignal::new(String::new());
    view! {
        <Field label="Lower the cap" hint="The cap can only go down. There is no way to raise it during a run.">
            <span class="row">
                <input
                    class="field"
                    type="number"
                    step="0.1"
                    min="0.1"
                    aria-label="New cap in dollars"
                    style="max-width:8em"
                    prop:value=move || draft.get()
                    on:input=move |ev| draft.set(event_target_value(&ev))
                />
                <button class="btn" on:click=move |_| apply_cap(ui, draft.get_untracked())>"Apply"</button>
            </span>
        </Field>
    }
}

fn apply_cap(ui: Ui, text: String) {
    let cap = ui.snapshot.get_untracked().cost.cap;
    if !lower_cap_valid(&text, cap) {
        ui.show_toast(
            "The cap can only be lowered. Enter a smaller amount.",
            ToastKind::Warn,
        );
        return;
    }
    spawn_local(async move {
        let result =
            api::post::<_, serde_json::Value>("/api/cap/lower", &LowerCap { cap: text }).await;
        if result.is_ok() {
            ui.show_toast("Cap lowered.", ToastKind::Ok);
        }
        settle(ui, result);
    });
}

/// The three tier selects. Each lists the ready tools and saves the whole routing on a change.
#[component]
pub fn RoutingFields() -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="psec">
            <h5>"Which agent does what"</h5>
            {TIER_ROWS
                .into_iter()
                .map(|(tier, label, hint)| tier_field(ui, tier, label, hint))
                .collect_view()}
        </div>
    }
}

fn tier_field(ui: Ui, tier: Tier, label: &'static str, hint: &'static str) -> impl IntoView {
    view! {
        <Field label=label hint=hint>
            <select
                aria-label=label
                on:change=move |ev| set_route(ui, tier, event_target_value(&ev))
            >
                {move || tool_options(&ui.snapshot.get(), tier)}
            </select>
        </Field>
    }
}

/// One option per ready tool. The chosen agent for the tier is marked on its option, so the
/// select shows it even when the options are drawn after the select.
fn tool_options(snapshot: &Snapshot, tier: Tier) -> AnyView {
    let chosen = snapshot.routing.for_tier(tier);
    ready_tools(&snapshot.tools)
        .into_iter()
        .map(|tool| {
            let selected = chosen == Some(&tool.id);
            view! {
                <option value=tool.id.as_str().to_owned() selected=selected>
                    {tool.name.clone()}
                </option>
            }
        })
        .collect_view()
        .into_any()
}

fn set_route(ui: Ui, tier: Tier, tool: String) {
    let routing = routing_with(
        &ui.snapshot.get_untracked().routing,
        tier,
        Some(ToolId::new(tool)),
    );
    spawn_local(async move {
        let result =
            api::send::<_, serde_json::Value>("PUT", "/api/routing", &RoutingUpdate { routing })
                .await;
        settle(ui, result);
    });
}

#[component]
pub fn AgentsPage() -> impl IntoView {
    let ui = use_ui();
    let scanning = RwSignal::new(false);
    view! {
        <h2>"Agents and tools"</h2>
        <p class="lead">"Sentinel found these CLIs on this PC. Enable at least 3 agents across 2 tools to run a full backlog."</p>
        <div class="row sp" style="margin-bottom:.4em">
            <span class="sub">{move || enabled_summary(&ui.snapshot.get().tools)}</span>
            <ScanButton ui=ui scanning=scanning idle="Rescan" />
        </div>
        {move || ui.snapshot.get().tools.into_iter().map(|tool| tool_row(ui, tool)).collect_view()}
        <RoutingFields />
    }
}

/// The line above the tool list. Only a tool that is enabled and ready can take work.
pub fn enabled_summary(tools: &[ToolInfo]) -> String {
    let count = tools
        .iter()
        .filter(|tool| tool.enabled && matches!(tool.status, ToolStatus::Ready))
        .count();
    format!("{count} enabled and ready")
}

fn tool_row(ui: Ui, tool: ToolInfo) -> impl IntoView {
    let (label, color, shape) = tool_status_look(tool.status);
    let id = tool.id.as_str().to_owned();
    let path_line = tool_path_line(&tool);
    let style = format!("color:var(--{color})");
    view! {
        <div class="toolrow">
            <Toggle checked=tool.enabled on_change=Callback::new(move |on: bool| toggle_tool(ui, id.clone(), on)) />
            <div class="n">
                <b style="font-weight:500">{tool.name}</b>
                <small>{path_line}</small>
            </div>
            <span class="stb" style=style>
                <svg class="shp" viewBox="0 0 12 12" aria-hidden="true" inner_html=shape_markup(shape, color)></svg>
                <span>{label}</span>
            </span>
        </div>
    }
}

/// The words, color, and shape for a tool's status.
pub fn tool_status_look(status: ToolStatus) -> (&'static str, &'static str, &'static str) {
    match status {
        ToolStatus::Ready => ("Found", "ok", "dot"),
        ToolStatus::NeedsLogin => ("Found, needs login", "warn", "ring"),
        ToolStatus::NeedsCommand => ("Found, needs a command", "warn", "ring"),
        ToolStatus::Missing => ("Missing", "danger", "oct"),
        ToolStatus::Failed => ("Failed", "danger", "oct"),
    }
}

/// The path, then the version when the tool reported one.
pub fn tool_path_line(tool: &ToolInfo) -> String {
    let path = if tool.path.is_empty() {
        "not found on this PC"
    } else {
        tool.path.as_str()
    };
    if tool.version.is_empty() || tool.version == "unknown" {
        path.to_owned()
    } else {
        format!("{path} · {}", tool.version)
    }
}

fn toggle_tool(ui: Ui, id: String, enabled: bool) {
    spawn_local(async move {
        let path = format!("/api/tools/{id}");
        let result = api::send::<_, serde_json::Value>("PUT", &path, &ToolToggle { enabled }).await;
        settle(ui, result);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_dollars() -> Cost {
        Cost::from_micro_usd(2_000_000, false)
    }

    #[test]
    fn a_lower_cap_is_accepted() {
        assert!(lower_cap_valid("1.50", two_dollars()));
    }

    #[test]
    fn a_cap_equal_to_or_above_the_current_one_is_refused() {
        assert!(!lower_cap_valid("2.00", two_dollars()));
        assert!(!lower_cap_valid("9.00", two_dollars()));
    }

    #[test]
    fn changing_one_tier_keeps_the_other_two_agents() {
        let routing = Routing {
            cheap: Some(ToolId::new("opencode")),
            mid: Some(ToolId::new("codex")),
            smart: Some(ToolId::new("claude")),
        };
        let next = routing_with(&routing, Tier::Mid, Some(ToolId::new("gemini")));
        assert_eq!(next.cheap, routing.cheap);
        assert_eq!(next.mid, Some(ToolId::new("gemini")));
        assert_eq!(next.smart, routing.smart);
    }

    #[test]
    fn only_ready_tools_can_be_picked_for_a_tier() {
        let tool = |id: &str, status: ToolStatus| ToolInfo {
            id: ToolId::new(id),
            name: id.to_owned(),
            path: String::new(),
            version: String::new(),
            adapter: "generic".to_owned(),
            status,
            tier: Tier::Cheap,
            enabled: true,
        };
        let tools = [
            tool("claude", ToolStatus::Ready),
            tool("gemini", ToolStatus::Missing),
        ];
        let picked: Vec<&ToolId> = ready_tools(&tools).iter().map(|t| &t.id).collect();
        assert_eq!(picked, [&ToolId::new("claude")]);
    }

    #[test]
    fn zero_and_text_that_is_not_money_are_refused() {
        assert!(!lower_cap_valid("0", two_dollars()));
        assert!(!lower_cap_valid("abc", two_dollars()));
        assert!(!lower_cap_valid("", two_dollars()));
    }

    #[test]
    fn only_enabled_ready_tools_count_as_working_agents() {
        let tools = [
            tool("claude", true, ToolStatus::Ready, "/bin/claude"),
            tool("codex", false, ToolStatus::Ready, "/bin/codex"),
            tool("gemini", true, ToolStatus::Missing, ""),
        ];
        assert_eq!(enabled_summary(&tools), "1 enabled and ready");
    }

    #[test]
    fn a_missing_tool_says_it_was_not_found() {
        let missing = tool("gemini", true, ToolStatus::Missing, "");
        assert_eq!(tool_path_line(&missing), "not found on this PC");
    }

    #[test]
    fn a_version_is_shown_only_when_the_tool_reported_one() {
        let mut found = tool("claude", true, ToolStatus::Ready, "/bin/claude");
        found.version = "unknown".to_owned();
        assert_eq!(tool_path_line(&found), "/bin/claude");
        found.version = "1.2".to_owned();
        assert_eq!(tool_path_line(&found), "/bin/claude · 1.2");
    }

    fn tool(id: &str, enabled: bool, status: ToolStatus, path: &str) -> ToolInfo {
        ToolInfo {
            id: sentinel_core::ToolId::new(id),
            name: id.to_owned(),
            path: path.to_owned(),
            version: String::new(),
            adapter: "generic".to_owned(),
            status,
            tier: sentinel_core::Tier::Cheap,
            enabled,
        }
    }
}
