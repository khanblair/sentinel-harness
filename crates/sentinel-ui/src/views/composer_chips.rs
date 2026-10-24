//! The pending attachments of the open chat: the chips row above the footer, and the reload that
//! fills it. The daemon keeps the chips per chat, so switching chats reloads them.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::components::attachment_chip::AttachmentChip;
use crate::state::{ToastKind, Ui, use_ui};

/// Loads the pending attachments of the open chat. A reply for a chat the user has left is
/// dropped. With no chat open, nothing is pending.
pub fn reload_attachments(ui: Ui) {
    let Some(chat) = ui.open_chat.get_untracked() else {
        ui.attachments.set(Vec::new());
        return;
    };
    spawn_local(async move {
        match api::pending_attachments(&chat).await {
            Ok(list) if ui.open_chat.get_untracked().as_ref() == Some(&chat) => {
                ui.attachments.set(list);
            }
            Ok(_) => {}
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

/// The pending attachments of the open chat, one chip each.
#[component]
pub fn ChipsRow() -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="chips" id="attachChips" hidden=move || ui.attachments.with(Vec::is_empty)>
            <For each=move || ui.attachments.get() key=|attachment| attachment.id.clone()
                children=|attachment| view! { <AttachmentChip attachment=attachment /> } />
        </div>
    }
}
