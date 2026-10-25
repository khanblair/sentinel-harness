//! The agent chip: shows which agent the open chat uses, and opens the menu that changes it. The
//! daemon keeps the agent per chat, so a change is one `PUT /api/chats/:id`.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::UpdateChat;
use sentinel_core::{ChatId, Snapshot, ToolId, ToolInfo, ToolStatus};

use crate::api::{self, ApiFailure};
use crate::components::icon::Icon;
use crate::components::popover::toggle_popover;
use crate::state::{Align, PopoverKind, ToastKind, Ui, refresh, use_ui};
use crate::views::composer_send::start_retry;
use crate::views::stream_model::last_owner_text;

pub const BUSY_TIP: &str = "Stop the run first.";

/// One row of the agent menu. `agent` is `None` for the Auto-route row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuRow {
    pub agent: Option<ToolId>,
    pub label: String,
    /// A dim note under the name, such as `paused`.
    pub mark: Option<&'static str>,
    pub ticked: bool,
    pub choosable: bool,
}

/// The name of a tool, or its id when the tool is not listed.
pub fn tool_name(tools: &[ToolInfo], id: &ToolId) -> String {
    tools
        .iter()
        .find(|tool| tool.id == *id)
        .map_or_else(|| id.as_str().to_owned(), |tool| tool.name.clone())
}

/// `Agent: Auto` for auto-route, else `Agent: ` and the agent's name.
pub fn chip_label(chat_agent: Option<&ToolId>, tools: &[ToolInfo]) -> String {
    match chat_agent {
        None => "Agent: Auto".to_owned(),
        Some(id) => format!("Agent: {}", tool_name(tools, id)),
    }
}

/// The agent a chat uses. `None` is auto-route, and also an unknown chat.
pub fn chat_agent(snapshot: &Snapshot, chat: Option<&ChatId>) -> Option<ToolId> {
    let chat = chat?;
    snapshot
        .chats
        .iter()
        .find(|info| info.id == *chat)
        .and_then(|info| info.agent.clone())
}

/// True when `chat` holds the live run. Its agent cannot change until the run ends.
pub fn chat_has_live_run(snapshot: &Snapshot, chat: Option<&ChatId>) -> bool {
    snapshot
        .run
        .as_ref()
        .is_some_and(|run| Some(&run.chat) == chat)
}

/// The menu rows: Auto-route first, then every tool. A tool that is off or not ready shows why
/// and cannot be picked.
pub fn menu_rows(tools: &[ToolInfo], current: Option<&ToolId>) -> Vec<MenuRow> {
    let auto = MenuRow {
        agent: None,
        label: "Auto-route".to_owned(),
        mark: None,
        ticked: current.is_none(),
        choosable: true,
    };
    let mut rows = vec![auto];
    rows.extend(tools.iter().map(|tool| tool_row(tool, current)));
    rows
}

fn tool_row(tool: &ToolInfo, current: Option<&ToolId>) -> MenuRow {
    MenuRow {
        agent: Some(tool.id.clone()),
        label: tool.name.clone(),
        mark: row_mark(tool),
        ticked: current == Some(&tool.id),
        choosable: tool.enabled && tool.status.is_ready(),
    }
}

/// The dim note for a tool that cannot take work. `None` when it is on and ready.
pub fn row_mark(tool: &ToolInfo) -> Option<&'static str> {
    if !tool.enabled {
        Some("paused")
    } else if tool.status == ToolStatus::NeedsLogin {
        Some("not signed in")
    } else if !tool.status.is_ready() {
        Some("not ready")
    } else {
        None
    }
}

/// What `/agent` asks for. The daemon checks that a named agent exists and is ready.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentCommand {
    /// The word is empty.
    Missing,
    /// Route by tier.
    Auto,
    /// One agent.
    Named(ToolId),
}

pub fn parse_agent_command(arg: &str) -> AgentCommand {
    match arg.trim() {
        "" => AgentCommand::Missing,
        "auto" => AgentCommand::Auto,
        name => AgentCommand::Named(ToolId::new(name)),
    }
}

/// The text of a refusal. The daemon's own message is shown, not the status line.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "only a status failure carries the daemon's message; any other failure shows its own text"
)]
pub fn refusal_text(failure: &ApiFailure) -> String {
    match failure {
        ApiFailure::Status { message, .. } => message.clone(),
        other => other.to_string(),
    }
}

/// The value the daemon takes for an agent: its id, or `auto`.
fn agent_value(agent: Option<&ToolId>) -> String {
    agent.map_or_else(|| "auto".to_owned(), |id| id.as_str().to_owned())
}

/// Sets the open chat's agent. With `retry`, the text is sent again as a new run after the change
/// is saved. A refusal shows the daemon's message and leaves the chat as it was.
pub fn choose_chat_agent(ui: Ui, agent: Option<&ToolId>, retry: Option<String>) {
    let Some(chat) = ui.open_chat.get_untracked() else {
        ui.show_toast("Open a chat first.", ToastKind::Warn);
        return;
    };
    let update = UpdateChat {
        agent: Some(agent_value(agent)),
        ..UpdateChat::default()
    };
    spawn_local(async move {
        let saved = api::update_chat(&chat, &update).await;
        match saved {
            Ok(_) => finish_choice(ui, chat, retry).await,
            Err(failure) => ui.show_toast(refusal_text(&failure), ToastKind::Warn),
        }
    });
}

async fn finish_choice(ui: Ui, chat: ChatId, retry: Option<String>) {
    if let Some(text) = retry
        && let Err(failure) = start_retry(Some(chat), text).await
    {
        ui.show_toast(failure.to_string(), ToastKind::Danger);
    }
    refresh(ui);
}

/// The chip in the composer footer. It is disabled while the open chat has a live run.
#[component]
pub fn AgentChip() -> impl IntoView {
    let ui = use_ui();
    let label = move || {
        let open = ui.open_chat.get();
        ui.snapshot
            .with(|s| chip_label(chat_agent(s, open.as_ref()).as_ref(), &s.tools))
    };
    let busy = move || {
        let open = ui.open_chat.get();
        ui.snapshot.with(|s| chat_has_live_run(s, open.as_ref()))
    };
    let tip = move || if busy() { BUSY_TIP } else { "" };
    view! {
        <button id="agentChip" class="cbtn" aria-label="Agent of this chat"
            disabled=busy title=tip
            on:click=move |_| toggle_popover(ui, "agentChip", Align::Right, PopoverKind::ChatAgent)>
            <Icon name="agent" size=15 />
            <span>{label}</span>
            <Icon name="down" size=12 />
        </button>
    }
}

/// The menu body. The chip opens it with `resend` off. A stopped-agent card opens it with
/// `resend` on, so the last message goes again once the agent is set.
#[component]
pub fn AgentMenu(resend: bool) -> impl IntoView {
    let ui = use_ui();
    let rows = move || {
        let open = ui.open_chat.get();
        ui.snapshot
            .with(|s| menu_rows(&s.tools, chat_agent(s, open.as_ref()).as_ref()))
    };
    view! {
        {move || rows().into_iter().map(|row| agent_row(ui, row, resend)).collect_view()}
    }
}

fn agent_row(ui: Ui, row: MenuRow, resend: bool) -> impl IntoView {
    let choice = row.agent.clone();
    let style = if row.choosable { "" } else { "opacity:.4" };
    view! {
        <button class="mi" disabled={!row.choosable} style=style
            on:click=move |_| pick_row(ui, choice.as_ref(), resend)>
            <span>
                <span>{row.label}{row.ticked.then(|| view! { <span class="tk"><Icon name="check" size=15 /></span> })}</span>
                {row.mark.map(|mark| view! { <div class="d">{mark}</div> })}
            </span>
        </button>
    }
}

fn pick_row(ui: Ui, agent: Option<&ToolId>, resend: bool) {
    ui.popover.set(None);
    let retry = if resend {
        ui.snapshot.with_untracked(|s| last_owner_text(&s.events))
    } else {
        None
    };
    choose_chat_agent(ui, agent, retry);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(id: &str, name: &str, status: ToolStatus, enabled: bool) -> ToolInfo {
        ToolInfo {
            id: ToolId::new(id),
            name: name.to_owned(),
            path: String::new(),
            version: String::new(),
            adapter: String::new(),
            status,
            tier: sentinel_core::Tier::Smart,
            enabled,
        }
    }

    #[test]
    fn auto_chat_reads_agent_auto() {
        assert_eq!(chip_label(None, &[]), "Agent: Auto");
    }

    #[test]
    fn named_chat_reads_the_agent_name() {
        let tools = vec![tool("claude", "Claude", ToolStatus::Ready, true)];
        assert_eq!(
            chip_label(Some(&ToolId::new("claude")), &tools),
            "Agent: Claude"
        );
    }

    #[test]
    fn unknown_agent_reads_its_id() {
        assert_eq!(chip_label(Some(&ToolId::new("codex")), &[]), "Agent: codex");
    }

    #[test]
    fn auto_route_is_the_first_row_and_ticked_when_nothing_is_chosen() {
        let rows = menu_rows(&[], None);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].agent, None);
        assert!(rows[0].ticked);
        assert!(rows[0].choosable);
    }

    #[test]
    fn the_current_agent_is_ticked_and_the_others_are_not() {
        let tools = vec![
            tool("claude", "Claude", ToolStatus::Ready, true),
            tool("codex", "Codex", ToolStatus::Ready, true),
        ];
        let rows = menu_rows(&tools, Some(&ToolId::new("codex")));
        assert!(!rows[0].ticked);
        assert!(!rows[1].ticked);
        assert!(rows[2].ticked);
    }

    #[test]
    fn a_paused_tool_is_marked_and_cannot_be_picked() {
        let tools = vec![tool("claude", "Claude", ToolStatus::Ready, false)];
        let rows = menu_rows(&tools, None);
        assert_eq!(rows[1].mark, Some("paused"));
        assert!(!rows[1].choosable);
    }

    #[test]
    fn a_tool_that_needs_login_is_marked_not_signed_in() {
        let tools = vec![tool("claude", "Claude", ToolStatus::NeedsLogin, true)];
        let rows = menu_rows(&tools, None);
        assert_eq!(rows[1].mark, Some("not signed in"));
        assert!(!rows[1].choosable);
    }

    #[test]
    fn a_ready_tool_has_no_mark_and_can_be_picked() {
        let tools = vec![tool("claude", "Claude", ToolStatus::Ready, true)];
        let rows = menu_rows(&tools, None);
        assert_eq!(rows[1].mark, None);
        assert!(rows[1].choosable);
    }

    #[test]
    fn an_empty_word_is_not_an_agent_choice() {
        assert_eq!(parse_agent_command("   "), AgentCommand::Missing);
    }

    #[test]
    fn auto_means_auto_route() {
        assert_eq!(parse_agent_command("auto"), AgentCommand::Auto);
    }

    #[test]
    fn a_name_means_that_agent() {
        assert_eq!(
            parse_agent_command("claude"),
            AgentCommand::Named(ToolId::new("claude"))
        );
    }

    #[test]
    fn a_refusal_shows_the_daemon_message_only() {
        let failure = ApiFailure::Status {
            code: 400,
            message: "Claude is not ready.".to_owned(),
        };
        assert_eq!(refusal_text(&failure), "Claude is not ready.");
    }
}
