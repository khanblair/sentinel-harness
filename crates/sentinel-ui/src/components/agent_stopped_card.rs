//! The card for an agent that stopped. The chat's run ends with it, and no other agent takes over.
//! Retry sends the last message again. Switch agent picks another agent, then sends it.

use leptos::prelude::*;
use sentinel_core::ToolId;

use crate::components::agent_chip::tool_name;
use crate::components::icon::Icon;
use crate::components::popover::toggle_popover;
use crate::state::{Align, PopoverKind, use_ui};
use crate::views::composer_send::resend_text;

pub fn stop_title(name: &str) -> String {
    format!("{name} stopped")
}

/// The id of the Switch agent button. The popover is anchored there, so a click on the button
/// is not read as a click outside the menu.
pub fn switch_anchor(tool: &ToolId) -> String {
    format!("switch-{}", tool.as_str())
}

#[component]
pub fn AgentStoppedCard(
    tool: ToolId,
    #[prop(into)] reason: String,
    retry: Option<String>,
) -> impl IntoView {
    let ui = use_ui();
    let name = ui.snapshot.with_untracked(|s| tool_name(&s.tools, &tool));
    view! {
        <div class="card dangc">
            <h4 style="color:var(--danger)"><Icon name="stop" size=16 />" "{stop_title(&name)}</h4>
            <div>{reason}</div>
            {retry.map(|text| view! { <StopActions tool=tool text=text /> })}
        </div>
    }
}

/// The two buttons. They exist only when the chat has a message to send again.
#[component]
fn StopActions(tool: ToolId, text: String) -> impl IntoView {
    let ui = use_ui();
    let anchor = switch_anchor(&tool);
    let open_anchor = anchor.clone();
    view! {
        <div class="row" style="margin-top:.6em">
            <button class="btn" on:click=move |_| resend_text(ui, text.clone())>"Retry"</button>
            <button class="btn" id=anchor
                on:click=move |_| toggle_popover(ui, &open_anchor, Align::Left, PopoverKind::SwitchAgent)>
                "Switch agent"
            </button>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_names_the_agent_that_stopped() {
        assert_eq!(stop_title("Claude Code"), "Claude Code stopped");
    }

    #[test]
    fn each_agent_has_its_own_switch_anchor() {
        assert_eq!(switch_anchor(&ToolId::new("claude")), "switch-claude");
        assert_ne!(
            switch_anchor(&ToolId::new("claude")),
            switch_anchor(&ToolId::new("codex"))
        );
    }
}
