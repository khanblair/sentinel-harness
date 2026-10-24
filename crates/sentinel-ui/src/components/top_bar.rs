//! `TopBar`: the run title, agent stack, cost chip, run controls, and panel icons
//! (`docs/ui-rules.md`, Top Bar).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::{Cost, HarnessState};

use crate::api::{self, ApiFailure};
use crate::chats::chat_title;
use crate::components::agent_stack::AgentStack;
use crate::components::icon::Icon;
use crate::components::popover::toggle_popover;
use crate::components::sidebar::is_live;
use crate::layout::{Layout, PanelId};
use crate::state::{self, Align, PopoverKind, ToastKind, Ui, use_ui};

/// Panel icon and tooltip for each top bar panel, in `PanelId::TOP_BAR` order.
const TOP_BAR_ICONS: [(&str, &str); 6] = [
    ("tasks", "Tasks board"),
    ("terminal", "Agent terminal"),
    ("diff", "Changes"),
    ("lock", "Reserved files"),
    ("brain", "Brain"),
    ("phone", "Phone and remote"),
];

/// A cost with ` est` after it when any part of the figure was guessed.
pub fn spend_label(cost: Cost) -> String {
    if cost.is_estimated() {
        format!("{cost} est")
    } else {
        cost.to_string()
    }
}

/// The cost chip turns warn at 70 percent of the cap and danger at 95 percent.
pub fn cost_chip_class(percent: u8) -> &'static str {
    if percent >= 95 {
        "d"
    } else if percent >= 70 {
        "w"
    } else {
        ""
    }
}

/// Sends a POST with an empty body and then refreshes the snapshot or shows the error.
pub fn post_action(ui: Ui, path: String) {
    spawn_local(async move {
        settle(ui, api::post_empty::<serde_json::Value>(&path).await);
    });
}

/// After a change went to the daemon: refresh on success, show the failure otherwise.
pub fn settle<T>(ui: Ui, result: Result<T, ApiFailure>) {
    match result {
        Ok(_) => state::refresh(ui),
        Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
    }
}

#[component]
pub fn TopBar() -> impl IntoView {
    let ui = use_ui();
    view! {
        <header class="hd-c">
            {move || {
                let collapsed = !ui.sidebar_open.get() || ui.layout.get() == Layout::Phone;
                collapsed.then(|| view! {
                    <button class="ibtn" aria-label="Open sidebar"
                        on:click=move |_| toggle_sidebar(ui)>
                        <Icon name="sidebar" size=18 />
                    </button>
                })
            }}
            <button class="title-btn" id="titleBtn" aria-label="Chat menu"
                on:click=move |_| toggle_popover(ui, "titleBtn", Align::Left, PopoverKind::Title)>
                <span>{move || {
                    let open = ui.open_chat.get();
                    ui.snapshot.with(|snapshot| chat_title(snapshot, open.as_ref()))
                }}</span>
                <Icon name="down" size=14 />
            </button>
            <AgentStack />
            <span class="hd-sp"></span>
            <CostChip />
            <RunControls />
            <PanelIcons />
        </header>
    }
}

fn toggle_sidebar(ui: Ui) {
    let opening = !ui.sidebar_open.get_untracked();
    ui.sidebar_open.set(opening);
    if opening && ui.layout.get_untracked() == Layout::Phone {
        ui.panel_open.set(false);
    }
}

#[component]
fn CostChip() -> impl IntoView {
    let ui = use_ui();
    let percent = move || {
        let cost = ui.snapshot.with(|snapshot| snapshot.cost);
        cost.total.percent_of(cost.cap)
    };
    view! {
        <button class=move || format!("costchip {}", cost_chip_class(percent())) aria-label="Cost"
            on:click=move |_| ui.open_panel(PanelId::Cost)>
            <b>{move || ui.snapshot.with(|snapshot| spend_label(snapshot.cost.total))}</b>
            <span class="bar">
                <i style=move || format!("width:{}%", percent().min(100))></i>
            </span>
            <span class="cap-t">{move || ui.snapshot.with(|snapshot| spend_label(snapshot.cost.cap))}</span>
        </button>
    }
}

#[component]
fn RunControls() -> impl IntoView {
    let ui = use_ui();
    let live = move || ui.snapshot.with(is_live);
    let paused = move || {
        ui.snapshot
            .with(|snapshot| snapshot.harness == HarnessState::Paused)
    };
    view! {
        <Show when=live fallback=|| ()>
            <button class="ibtn" aria-label=move || if paused() { "Resume" } else { "Pause" }
                disabled=move || ui.connection.get() != state::Connection::Live
                on:click=move |_| post_action(ui, if paused() { "/api/resume".to_owned() } else { "/api/pause".to_owned() })>
                {move || if paused() {
                    view! { <Icon name="play" size=16 /> }.into_any()
                } else {
                    view! { <Icon name="pause" size=16 /> }.into_any()
                }}
            </button>
            <button class="ibtn stopbtn" aria-label="Stop run"
                disabled=move || ui.connection.get() != state::Connection::Live
                on:click=move |_| post_action(ui, "/api/stop".to_owned())>
                <Icon name="stop" size=15 />
            </button>
        </Show>
    }
}

#[component]
fn PanelIcons() -> impl IntoView {
    let ui = use_ui();
    let more = move |_| toggle_popover(ui, "moreBtn", Align::Right, PopoverKind::More);
    view! {
        <span class="hd-icons">
            {PanelId::TOP_BAR
                .into_iter()
                .zip(TOP_BAR_ICONS)
                .map(|(panel, (icon, tip))| view! {
                    <button class="ibtn" data-tip=tip aria-label=tip
                        aria-pressed=move || ui.panel_open.get() && ui.active_panel.get() == Some(panel)
                        on:click=move |_| ui.open_panel(panel)>
                        <Icon name=icon size=17 />
                    </button>
                })
                .collect_view()}
            <button class="ibtn" id="moreBtn" data-tip="More" aria-label="More" on:click=more>
                <Icon name="more" size=17 />
            </button>
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{ChatId, Snapshot};

    #[test]
    fn cost_chip_turns_warn_at_seventy_percent_and_danger_at_ninety_five() {
        assert_eq!(cost_chip_class(69), "");
        assert_eq!(cost_chip_class(70), "w");
        assert_eq!(cost_chip_class(94), "w");
        assert_eq!(cost_chip_class(95), "d");
    }

    #[test]
    fn estimated_spend_is_marked_est() {
        assert_eq!(spend_label(Cost::from_micro_usd(40_000, true)), "$0.04 est");
        assert_eq!(spend_label(Cost::from_micro_usd(40_000, false)), "$0.04");
    }

    #[test]
    fn the_title_falls_back_to_new_chat_when_no_chat_is_open() {
        assert_eq!(chat_title(&Snapshot::empty(), None), "New chat");
    }

    #[test]
    fn the_title_is_the_open_chat_title() {
        let snapshot = Snapshot {
            chats: vec![sentinel_core::ChatInfo {
                id: ChatId::new("c1"),
                project: "api".to_owned(),
                title: "Fees".to_owned(),
                pinned: false,
                archived: false,
                created_at: time::OffsetDateTime::UNIX_EPOCH,
                updated_at: time::OffsetDateTime::UNIX_EPOCH,
                run: sentinel_core::ChatRunState::Idle,
                verified: 0,
                total: 0,
                needs_you: false,
            }],
            ..Snapshot::empty()
        };
        assert_eq!(chat_title(&snapshot, Some(&ChatId::new("c1"))), "Fees");
    }
}
