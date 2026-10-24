//! `PanelHost`: the right panel. Its tab strip lists the open panels, and its body shows the
//! active one (`docs/ui-rules.md`, Layout).

use leptos::prelude::*;

use crate::components::icon::Icon;
use crate::components::popover::toggle_popover;
use crate::layout::PanelId;
use crate::state::{Align, PopoverKind, Ui, use_ui};
use crate::views::{
    agents::AgentsPanel, board::BoardPanel, brain::BrainPanel, changes::ChangesPanel,
    cost::CostPanel, files::FilesPanel, permissions::PermissionsPanel, remote::RemotePanel,
    report::ReportPanel, reserved::ReservedPanel, telegram::TelegramPanel,
    terminal::TerminalPanel,
};

/// The icon of each panel's tab.
pub fn panel_icon(panel: PanelId) -> &'static str {
    match panel {
        PanelId::Board => "tasks",
        PanelId::Terminal => "terminal",
        PanelId::Changes => "diff",
        PanelId::Reserved => "lock",
        PanelId::Brain => "brain",
        PanelId::Remote => "phone",
        PanelId::Files => "folder",
        PanelId::Cost => "dollar",
        PanelId::Report => "list",
        PanelId::Agents => "agent",
        PanelId::Permissions => "shield",
        PanelId::Telegram => "send",
    }
}

#[component]
pub fn PanelHost() -> impl IntoView {
    let ui = use_ui();
    view! {
        <aside class="rp" id="rp" aria-label="Panels">
            <div class="rp-hd">
                <div class="ptabs" role="tablist">
                    <For
                        each=move || ui.panel_tabs.get()
                        key=|panel| *panel
                        children=move |panel| panel_tab(ui, panel)
                    />
                </div>
                <button class="ibtn" id="panelPlus" data-tip="Open a panel" aria-label="Open a panel"
                    on:click=move |_| toggle_popover(ui, "panelPlus", Align::Right, PopoverKind::Panel)>
                    <Icon name="plus" size=16 />
                </button>
                <button class="ibtn" data-tip="Expand panel" aria-label="Expand panel"
                    on:click=move |_| ui.panel_wide.update(|wide| *wide = !*wide)>
                    <Icon name="expand" size=15 />
                </button>
                <button class="ibtn" data-tip="Close panels" aria-label="Close panels"
                    on:click=move |_| ui.panel_open.set(false)>
                    <Icon name="x" size=16 />
                </button>
            </div>
            <div class="rp-body">
                {move || ui.active_panel.get().map(panel_body)}
            </div>
        </aside>
    }
}

fn panel_tab(ui: Ui, panel: PanelId) -> impl IntoView {
    let selected = move || ui.active_panel.get() == Some(panel);
    view! {
        <div class="ptab" role="tab" tabindex="0" aria-selected=selected
            on:click=move |_| ui.active_panel.set(Some(panel))>
            <Icon name=panel_icon(panel) size=15 />
            <span>{panel.label()}</span>
            <span class="x" role="button" aria-label=format!("Close {}", panel.label())
                on:click=move |event| {
                    event.stop_propagation();
                    ui.close_panel_tab(panel);
                }>
                <Icon name="x" size=12 />
            </span>
        </div>
    }
}

fn panel_body(panel: PanelId) -> AnyView {
    match panel {
        PanelId::Board => view! { <BoardPanel /> }.into_any(),
        PanelId::Terminal => view! { <TerminalPanel /> }.into_any(),
        PanelId::Changes => view! { <ChangesPanel /> }.into_any(),
        PanelId::Reserved => view! { <ReservedPanel /> }.into_any(),
        PanelId::Brain => view! { <BrainPanel /> }.into_any(),
        PanelId::Remote => view! { <RemotePanel /> }.into_any(),
        PanelId::Files => view! { <FilesPanel /> }.into_any(),
        PanelId::Cost => view! { <CostPanel /> }.into_any(),
        PanelId::Report => view! { <ReportPanel /> }.into_any(),
        PanelId::Agents => view! { <AgentsPanel /> }.into_any(),
        PanelId::Permissions => view! { <PermissionsPanel /> }.into_any(),
        PanelId::Telegram => view! { <TelegramPanel /> }.into_any(),
    }
}
