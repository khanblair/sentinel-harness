//! The send-to chip: shows where the next message goes, and opens the target menu.

use leptos::prelude::*;
use sentinel_core::ToolInfo;

use crate::components::icon::Icon;
use crate::components::popover::toggle_popover;
use crate::state::{Align, PopoverKind, Target, use_ui};

/// "Sentinel" routes by tier. An agent shows its name and its tier.
pub fn target_label(target: &Target, tools: &[ToolInfo]) -> String {
    match target {
        Target::Sentinel => "Sentinel · auto-route".to_owned(),
        Target::Agent(id) => match tools.iter().find(|tool| tool.id == *id) {
            Some(tool) => format!("{} · {}", tool.name, tool.tier.ui_label()),
            None => id.as_str().to_owned(),
        },
    }
}

#[component]
pub fn TargetChip() -> impl IntoView {
    let ui = use_ui();
    view! {
        <button id="targetChip" class="cbtn" aria-label="Send to"
            on:click=move |_| toggle_popover(ui, "targetChip", Align::Right, PopoverKind::Target)>
            <Icon name="agent" size=15 />
            <span>{move || ui.snapshot.with(|s| target_label(&ui.target.get(), &s.tools))}</span>
            <Icon name="down" size=12 />
        </button>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{Tier, ToolId, ToolStatus};

    fn tool(id: &str, name: &str) -> ToolInfo {
        ToolInfo {
            id: ToolId::new(id),
            name: name.to_owned(),
            path: String::new(),
            version: String::new(),
            adapter: String::new(),
            status: ToolStatus::Ready,
            tier: Tier::Smart,
            enabled: true,
        }
    }

    #[test]
    fn sentinel_label_says_it_routes_by_tier() {
        assert_eq!(
            target_label(&Target::Sentinel, &[]),
            "Sentinel · auto-route"
        );
    }

    #[test]
    fn agent_label_names_the_agent_and_its_tier() {
        let tools = vec![tool("claude", "Claude Code")];
        assert_eq!(
            target_label(&Target::Agent(ToolId::new("claude")), &tools),
            "Claude Code · hard"
        );
    }

    #[test]
    fn unknown_agent_shows_its_id() {
        assert_eq!(
            target_label(&Target::Agent(ToolId::new("codex")), &[]),
            "codex"
        );
    }
}
