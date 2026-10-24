//! `AttachmentChip`: one pending attachment of the open chat, and the `Chip` markup it uses
//! (`docs/ui-rules.md`, Composer). A chip too large to send is drawn in the warning style.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::{Attachment, AttachmentId, ChatId};

use crate::api;
use crate::chats::{chip_icon, chip_label, too_large};
use crate::components::icon::Icon;
use crate::state::{ToastKind, Ui, use_ui};
use crate::views::composer::reload_attachments;

/// The tooltip of a chip that is too large to send.
pub const TOO_LARGE_TIP: &str = "Too large. The limit is 1 MB.";

/// One chip. `on_remove` adds the x button. A warning chip carries the size tooltip.
#[component]
pub fn Chip(
    icon: &'static str,
    name: String,
    #[prop(default = false)] warn: bool,
    #[prop(optional)] on_remove: Option<Callback<()>>,
) -> impl IntoView {
    let class = if warn { "chip att warnc" } else { "chip att" };
    let title = warn.then_some(TOO_LARGE_TIP);
    let remove_label = format!("Remove {name}");
    view! {
        <span class=class title=title>
            <Icon name=icon size=13 />
            <span class="cn">{name}</span>
            {on_remove.map(|remove| view! {
                <button class="ibtn" aria-label=remove_label on:click=move |_| remove.run(())>
                    <Icon name="x" size=12 />
                </button>
            })}
        </span>
    }
}

/// A chip for a pending attachment. The x button removes it from the chat on the daemon.
#[component]
pub fn AttachmentChip(attachment: Attachment) -> impl IntoView {
    let ui = use_ui();
    let icon = chip_icon(attachment.kind);
    let warn = too_large(attachment.size);
    let name = chip_label(&attachment);
    let Attachment { chat, id, .. } = attachment;
    let on_remove = Callback::new(move |()| remove_chip(ui, chat.clone(), id.clone()));
    view! { <Chip icon=icon name=name warn=warn on_remove=on_remove /> }
}

fn remove_chip(ui: Ui, chat: ChatId, id: AttachmentId) {
    spawn_local(async move {
        match api::remove_attachment(&chat, &id).await {
            Ok(_) => reload_attachments(ui),
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}
