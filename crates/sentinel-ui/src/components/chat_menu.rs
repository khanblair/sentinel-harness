//! `ChatMenu`: the menu of one chat, used by the chat row and the top bar title
//! (`docs/ui-rules.md`, Top Bar). Each entry acts, then the menu closes.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::UpdateChat;
use sentinel_core::{ChatId, Snapshot};

use crate::api;
use crate::chats::{MenuAction, MenuEntry, RenameState, menu_items, title_menu_items};
use crate::components::icon::Icon;
use crate::components::sidebar::is_live;
use crate::components::top_bar::{post_action, settle};
use crate::layout::PanelId;
use crate::state::{Modal, Ui, use_ui};

const MORE_PREFIX: &str = "chat-more-";
const ROW_PREFIX: &str = "chat-row-";

/// The DOM id of a chat row's more button. The menu is placed under it.
pub fn more_anchor(chat: &ChatId) -> String {
    format!("{MORE_PREFIX}{}", chat.as_str())
}

/// The DOM id of a chat row. A long press opens the menu under it.
pub fn row_anchor(chat: &ChatId) -> String {
    format!("{ROW_PREFIX}{}", chat.as_str())
}

/// The chat an anchor id belongs to, so the one chat menu body knows which chat to show.
pub fn chat_from_anchor(anchor: &str) -> Option<ChatId> {
    anchor
        .strip_prefix(MORE_PREFIX)
        .or_else(|| anchor.strip_prefix(ROW_PREFIX))
        .filter(|id| !id.is_empty())
        .map(ChatId::new)
}

/// Whether this chat holds the run that is live now.
fn holds_live_run(snapshot: &Snapshot, chat: &ChatId) -> bool {
    is_live(snapshot) && snapshot.run.as_ref().is_some_and(|run| run.chat == *chat)
}

/// The entries for a chat. A chat the snapshot does not hold has none.
fn entries_for(snapshot: &Snapshot, chat: &ChatId, title: bool, phone: bool) -> Vec<MenuEntry> {
    let Some(info) = snapshot.chats.iter().find(|info| info.id == *chat) else {
        return Vec::new();
    };
    let live = holds_live_run(snapshot, chat);
    if title {
        title_menu_items(info, live, phone)
    } else {
        menu_items(info, live, phone)
    }
}

#[component]
pub fn ChatMenu(chat: ChatId, title: bool) -> impl IntoView {
    let ui = use_ui();
    view! {
        {move || {
            let phone = ui.on_phone();
            let entries = ui.snapshot.with(|snapshot| entries_for(snapshot, &chat, title, phone));
            entries
                .into_iter()
                .map(|entry| entry_view(ui, chat.clone(), entry))
                .collect_view()
        }}
    }
}

fn entry_view(ui: Ui, chat: ChatId, entry: MenuEntry) -> AnyView {
    match entry {
        MenuEntry::Separator => view! { <div class="sep"></div> }.into_any(),
        MenuEntry::Item(item) => {
            let class = if item.danger { "mi dng" } else { "mi" };
            let action = item.action;
            view! {
                <button class=class disabled=item.disabled on:click=move |_| run_action(ui, &chat, action)>
                    <Icon name=item.icon size=16 />
                    <span>
                        <span>{item.label}</span>
                        {item.note.map(|note| view! { <div class="d">{note}</div> })}
                    </span>
                </button>
            }
            .into_any()
        }
    }
}

/// Closes the menu, then does the entry's action.
fn run_action(ui: Ui, chat: &ChatId, action: MenuAction) {
    ui.popover.set(None);
    match action {
        MenuAction::Rename => start_rename(ui, chat),
        MenuAction::TogglePin => toggle_pinned(ui, chat),
        MenuAction::ToggleArchive => toggle_archived(ui, chat),
        MenuAction::Delete => ui
            .modal
            .set(Some(Modal::ConfirmDelete { chat: chat.clone() })),
        MenuAction::ExportReport => ui.open_panel(PanelId::Report),
        MenuAction::StopRun => post_action(ui, "/api/stop".to_owned()),
    }
}

/// Opens the sidebar and puts the chat's row into rename mode.
fn start_rename(ui: Ui, chat: &ChatId) {
    let title = ui.snapshot.with_untracked(|snapshot| {
        snapshot
            .chats
            .iter()
            .find(|info| info.id == *chat)
            .map(|info| info.title.clone())
    });
    ui.sidebar_open.set(true);
    ui.renaming.set(Some(RenameState {
        chat: chat.clone(),
        title: title.unwrap_or_default(),
    }));
}

fn toggle_pinned(ui: Ui, chat: &ChatId) {
    let pinned = ui.snapshot.with_untracked(|snapshot| {
        snapshot
            .chats
            .iter()
            .any(|info| info.id == *chat && info.pinned)
    });
    let update = UpdateChat {
        pinned: Some(!pinned),
        ..UpdateChat::default()
    };
    send_update(ui, chat.clone(), update);
}

fn toggle_archived(ui: Ui, chat: &ChatId) {
    let archived = ui.snapshot.with_untracked(|snapshot| {
        snapshot
            .chats
            .iter()
            .any(|info| info.id == *chat && info.archived)
    });
    let update = UpdateChat {
        archived: Some(!archived),
        ..UpdateChat::default()
    };
    send_update(ui, chat.clone(), update);
}

fn send_update(ui: Ui, chat: ChatId, update: UpdateChat) {
    spawn_local(async move {
        settle(ui, api::update_chat(&chat, &update).await);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_anchor_names_its_chat_for_both_the_row_and_its_more_button() {
        let chat = ChatId::new("c7");
        assert_eq!(chat_from_anchor(&more_anchor(&chat)), Some(chat.clone()));
        assert_eq!(chat_from_anchor(&row_anchor(&chat)), Some(chat));
    }

    #[test]
    fn an_anchor_that_is_not_a_chat_names_no_chat() {
        assert_eq!(chat_from_anchor("titleBtn"), None);
        assert_eq!(chat_from_anchor("chat-more-"), None);
    }
}
