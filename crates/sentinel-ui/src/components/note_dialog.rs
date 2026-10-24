//! `NoteDialog`: a context note that goes into every agent prompt of the chat
//! (`docs/product-scope.md`, F23).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::AddAttachment;
use sentinel_core::{AttachmentKind, ChatId};

use crate::api;
use crate::components::icon::Icon;
use crate::state::{ToastKind, Ui, use_ui};
use crate::views::composer::reload_attachments;

#[component]
pub fn NoteDialog() -> impl IntoView {
    let ui = use_ui();
    let text = RwSignal::new(String::new());
    view! {
        <div class="modal-hd">
            <Icon name="edit" size=18 />
            <h3>"Add a context note"</h3>
        </div>
        <div class="modal-body" style="padding:1em 1.1em">
            <textarea class="field" rows="5" aria-label="Context note"
                prop:value=move || text.get()
                on:input=move |ev| text.set(event_target_value(&ev))></textarea>
            <p class="sub" style="margin:.5em 0 0">"Goes into every agent prompt."</p>
        </div>
        <div class="modal-ft">
            <button class="btn" on:click=move |_| ui.modal.set(None)>"Cancel"</button>
            <button class="btn pri" on:click=move |_| add_note(ui, &text.get_untracked())>"Add note"</button>
        </div>
    }
}

/// Adds the note to the open chat. An empty note is refused with a toast and the dialog stays.
fn add_note(ui: Ui, typed: &str) {
    let text = typed.trim().to_owned();
    if text.is_empty() {
        ui.show_toast("Write a note first", ToastKind::Warn);
        return;
    }
    let Some(chat) = ui.open_chat.get_untracked() else {
        ui.show_toast("Open a chat first.", ToastKind::Warn);
        return;
    };
    spawn_local(async move { send_note(ui, &chat, text).await });
}

async fn send_note(ui: Ui, chat: &ChatId, text: String) {
    let body = AddAttachment {
        kind: AttachmentKind::Note,
        path: None,
        text: Some(text),
        name: None,
        content_base64: None,
    };
    match api::add_attachment(chat, &body).await {
        Ok(_) => {
            ui.modal.set(None);
            reload_attachments(ui);
        }
        Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
    }
}
