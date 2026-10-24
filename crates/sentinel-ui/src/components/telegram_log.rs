//! The Activity list of the Telegram panel: one line per Telegram event in the snapshot, newest
//! last. The `.term` box of the prototype's activity log.

use leptos::prelude::*;

use crate::components::icon::Icon;
use crate::state::use_ui;
use crate::telegram::activity_lines;

#[component]
pub fn TelegramLog() -> impl IntoView {
    let ui = use_ui();
    let lines = move || {
        ui.snapshot
            .with(|snapshot| activity_lines(&snapshot.events))
    };
    view! {
        <Show
            when=move || !lines().is_empty()
            fallback=|| view! {
                <div class="note"><Icon name="info" size=14 /><span>"Nothing yet."</span></div>
            }
        >
            <div class="term" style="min-height:0">{move || lines().join("\n")}</div>
        </Show>
    }
}
