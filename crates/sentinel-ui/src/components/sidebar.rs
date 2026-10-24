//! `Sidebar`: the chats grouped by project, the New chat button, search, and the footer
//! (`docs/ui-rules.md`, Layout).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::{ChatId, HarnessState, Snapshot};

use crate::api;
use crate::chats::{ARCHIVED_GROUP, ChatRow, ProjectGroup, RowIcon, group_chats};
use crate::components::icon::Icon;
use crate::prefs::{Resolved, Theme};
use crate::state::{self, Connection, Modal, ToastKind, Ui, use_ui};
use crate::theme;

/// True while the harness runs or is paused.
pub fn is_live(snapshot: &Snapshot) -> bool {
    matches!(
        snapshot.harness,
        HarnessState::Running | HarnessState::Paused
    )
}

#[component]
pub fn Sidebar() -> impl IntoView {
    let search_open = RwSignal::new(false);
    let query = RwSignal::new(String::new());
    view! {
        <aside class="sb" id="sb" aria-label="Chats">
            <SidebarHeader search_open=search_open />
            <NewChatButton />
            <SearchRow search_open=search_open query=query />
            <ChatList query=query />
            <SidebarFooter />
        </aside>
    }
}

#[component]
fn SidebarHeader(search_open: RwSignal<bool>) -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="sb-hd">
            <button class="ibtn" aria-label="Toggle sidebar" data-tip="Hide sidebar"
                on:click=move |_| ui.sidebar_open.update(|open| *open = !*open)>
                <Icon name="sidebar" size=18 />
            </button>
            <button class="ibtn" aria-label="Back" data-tip="Back" disabled=true>
                <Icon name="back" size=18 />
            </button>
            <button class="ibtn" aria-label="Forward" data-tip="Forward" disabled=true>
                <Icon name="fwd" size=18 />
            </button>
            <span class="sp"></span>
            <span class="segi">
                <button class="ibtn" aria-label="Find a chat" data-tip="Find a chat"
                    aria-pressed=move || search_open.get()
                    on:click=move |_| search_open.update(|open| *open = !*open)>
                    <Icon name="search" size=16 />
                </button>
                <button class="ibtn on" aria-label="Workspace">
                    <Icon name="code" size=16 />
                </button>
            </span>
        </div>
    }
}

#[component]
fn NewChatButton() -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="sb-new">
            <button class="btn pri" style="width:100%" aria-label="New chat"
                disabled=move || ui.connection.get() != Connection::Live
                on:click=move |_| new_chat(ui)>
                <Icon name="plus" size=15 />
                <span>"New chat"</span>
            </button>
        </div>
    }
}

/// Makes an empty chat, opens it, and reloads. A failure shows as a toast.
fn new_chat(ui: Ui) {
    spawn_local(async move {
        match api::create_chat(None).await {
            Ok(chat) => {
                ui.open(chat.id);
                state::refresh(ui);
            }
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

#[component]
fn SearchRow(search_open: RwSignal<bool>, query: RwSignal<String>) -> impl IntoView {
    view! {
        <div class="sb-search" hidden=move || !search_open.get()>
            <input placeholder="Find a chat" aria-label="Find a chat"
                prop:value=move || query.get()
                on:input=move |ev| query.set(event_target_value(&ev)) />
        </div>
    }
}

#[component]
fn ChatList(query: RwSignal<String>) -> impl IntoView {
    let ui = use_ui();
    let closed_groups = RwSignal::new(vec![ARCHIVED_GROUP.to_owned()]);
    view! {
        <div class="sb-scroll">
            {move || {
                let needle = query.get();
                let groups = ui.snapshot.with(|snapshot| group_chats(&snapshot.chats, &needle));
                if groups.is_empty() {
                    let text = ui.snapshot.with(list_empty_text);
                    return view! { <div class="note" style="padding:1em">{text}</div> }.into_any();
                }
                let searching = !needle.trim().is_empty();
                groups
                    .into_iter()
                    .map(|group| group_view(group, closed_groups, searching))
                    .collect_view()
                    .into_any()
            }}
        </div>
    }
}

/// The text shown when the list has no row to show.
fn list_empty_text(snapshot: &Snapshot) -> &'static str {
    if snapshot.chats.is_empty() {
        "No chats yet."
    } else {
        "No chats match."
    }
}

/// A project header and its chats. While a search runs, every group is open, so a match shows.
fn group_view(
    group: ProjectGroup,
    closed: RwSignal<Vec<String>>,
    searching: bool,
) -> impl IntoView {
    let ui = use_ui();
    let ProjectGroup { project, chats } = group;
    let is_closed = !searching && closed.with(|list| list.contains(&project));
    let toggle_name = project.clone();
    view! {
        <div class="grp" class:closed=is_closed
            on:click=move |_| toggle_group(closed, &toggle_name)>
            {project}
            <Icon name="down" size=12 />
        </div>
        {(!is_closed).then(|| chats.into_iter().map(|row| chat_row(row, ui)).collect_view())}
    }
}

fn toggle_group(closed: RwSignal<Vec<String>>, group: &str) {
    closed.update(|list| {
        if let Some(index) = list.iter().position(|name| name == group) {
            list.remove(index);
        } else {
            list.push(group.to_owned());
        }
    });
}

fn chat_row(row: ChatRow, ui: Ui) -> impl IntoView {
    let id = row.id.clone();
    let open_id = row.id.clone();
    let active = move || ui.open_chat.with(|open| open.as_ref() == Some(&open_id));
    let icon = status_icon(row.icon);
    view! {
        <button class="run" class:active=active on:click=move |_| show_chat(ui, id.clone())>
            <span class="t">{row.title}</span>
            <span class="meta">
                {row.needs_you.then(|| view! { <span class="dot" title="Needs you"></span> })}
                <span>{row.count}</span>
                {icon}
            </span>
        </button>
    }
}

/// Opens a chat in the stream and reloads its events.
fn show_chat(ui: Ui, id: ChatId) {
    ui.open(id);
    state::refresh(ui);
}

fn status_icon(icon: RowIcon) -> AnyView {
    match icon {
        RowIcon::Spinner => view! { <span class="spin" title="Running"></span> }.into_any(),
        RowIcon::Paused => view! {
            <span style="color:var(--warn)" title="Paused"><Icon name="pause" size=14 /></span>
        }
        .into_any(),
        RowIcon::Done => view! {
            <span style="color:var(--ok)" title="Finished"><Icon name="check" size=14 /></span>
        }
        .into_any(),
        RowIcon::Blocked => view! {
            <span style="color:var(--danger)" title="Has blocked tasks"><Icon name="alert" size=14 /></span>
        }
        .into_any(),
        RowIcon::Planned => view! {
            <span title="Plan waiting for approval"><Icon name="clock" size=14 /></span>
        }
        .into_any(),
        RowIcon::Stopped => view! {
            <span title="Stopped"><Icon name="stop" size=13 /></span>
        }
        .into_any(),
        RowIcon::None => ().into_any(),
    }
}

#[component]
fn SidebarFooter() -> impl IntoView {
    let ui = use_ui();
    let dark = move || ui.prefs.get().resolve(theme::system_is_dark()) == Resolved::Dark;
    view! {
        <div class="sb-ft">
            <div class="me">
                <span class="av">"Y"</span>
                <span class="nm">"You · local daemon"</span>
                <button class="ibtn" data-tip=move || if dark() { "Light theme" } else { "Dark theme" }
                    aria-label=move || if dark() { "Switch to light theme" } else { "Switch to dark theme" }
                    on:click=move |_| toggle_theme(ui)>
                    {move || if dark() {
                        view! { <Icon name="sun" size=16 /> }.into_any()
                    } else {
                        view! { <Icon name="moon" size=16 /> }.into_any()
                    }}
                </button>
                <button class="ibtn" data-tip="Settings" aria-label="Settings"
                    on:click=move |_| ui.modal.set(Some(Modal::Settings { page: "general".to_owned() }))>
                    <Icon name="gear" size=16 />
                </button>
            </div>
        </div>
    }
}

fn toggle_theme(ui: Ui) {
    let dark = ui.prefs.get_untracked().resolve(theme::system_is_dark()) == Resolved::Dark;
    ui.prefs.update(|prefs| {
        prefs.theme = if dark { Theme::Light } else { Theme::Dark };
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{RunId, RunInfo};
    use time::OffsetDateTime;

    #[test]
    fn an_idle_harness_is_not_live() {
        assert!(!is_live(&Snapshot::empty()));
    }

    #[test]
    fn a_running_or_paused_harness_is_live() {
        let running = Snapshot {
            harness: HarnessState::Running,
            ..Snapshot::empty()
        };
        let paused = Snapshot {
            harness: HarnessState::Paused,
            run: Some(RunInfo {
                id: RunId::new("r1"),
                chat: ChatId::new("c1"),
                backlog: "backlog.md".to_owned(),
                wave: 1,
                started_at: OffsetDateTime::UNIX_EPOCH,
            }),
            ..Snapshot::empty()
        };
        assert!(is_live(&running));
        assert!(is_live(&paused));
    }

    #[test]
    fn an_empty_list_says_whether_chats_exist() {
        assert_eq!(list_empty_text(&Snapshot::empty()), "No chats yet.");
        let with_chat = Snapshot {
            chats: vec![sentinel_core::ChatInfo {
                id: ChatId::new("c1"),
                project: "api".to_owned(),
                title: "Fees".to_owned(),
                pinned: false,
                archived: false,
                created_at: OffsetDateTime::UNIX_EPOCH,
                updated_at: OffsetDateTime::UNIX_EPOCH,
                run: sentinel_core::ChatRunState::Idle,
                verified: 0,
                total: 0,
                needs_you: false,
            }],
            ..Snapshot::empty()
        };
        assert_eq!(list_empty_text(&with_chat), "No chats match.");
    }
}
