//! The pending attachments of the open chat, and the type of the next message: the chips row above
//! the footer, and the reload that fills it. The daemon keeps the chips per chat, so switching
//! chats reloads them.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::Intent;

use crate::api;
use crate::components::attachment_chip::{AttachmentChip, Chip};
use crate::components::intent_chip::chip_look;
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

/// The row shows when something is pending: an attachment, or the type of the next message.
fn chips_hidden(ui: Ui) -> bool {
    ui.attachments.with(Vec::is_empty) && ui.next_intent.with(Option::is_none)
}

/// The pending attachments of the open chat, one chip each, and the type set for the next message.
#[component]
pub fn ChipsRow() -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="chips" id="attachChips" hidden=move || chips_hidden(ui)>
            {move || ui.next_intent.get().map(|intent| view! { <NextIntentChip intent=intent /> })}
            <For each=move || ui.attachments.get() key=|attachment| attachment.id.clone()
                children=|attachment| view! { <AttachmentChip attachment=attachment /> } />
        </div>
    }
}

/// The type the next message will carry. The x clears it.
#[component]
fn NextIntentChip(intent: Intent) -> impl IntoView {
    let ui = use_ui();
    let (_, icon) = chip_look(intent);
    let name = format!("Next message: {}", intent.label());
    let on_remove = Callback::new(move |()| ui.next_intent.set(None));
    view! { <Chip icon=icon name=name on_remove=on_remove /> }
}
