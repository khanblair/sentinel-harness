//! `ConfirmDelete`: asks before a chat, its messages and its uploads are removed. The repository
//! and its commits are not touched (`docs/ui-rules.md`, Top Bar).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::ChatId;

use crate::api;
use crate::components::icon::Icon;
use crate::state::{ToastKind, Ui, refresh, use_ui};

#[component]
pub fn ConfirmDelete(chat: ChatId) -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="modal-hd">
            <span style="color:var(--danger)"><Icon name="trash" size=18 /></span>
            <h3>"Delete this chat?"</h3>
        </div>
        <div class="modal-body" style="padding:1em 1.1em">
            <p style="margin:0;line-height:1.6">
                "This removes its messages and uploaded files. Your repository and its commits are not touched."
            </p>
        </div>
        <div class="modal-ft">
            <button class="btn" on:click=move |_| ui.modal.set(None)>"Cancel"</button>
            <button class="btn dng" on:click=move |_| delete_chat(ui, chat.clone())>"Delete chat"</button>
        </div>
    }
}

/// Closes the dialog, deletes the chat, and reloads. The reload opens another chat when the
/// deleted one was open.
fn delete_chat(ui: Ui, chat: ChatId) {
    ui.modal.set(None);
    spawn_local(async move {
        match api::delete_chat(&chat).await {
            Ok(_) => refresh(ui),
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}
