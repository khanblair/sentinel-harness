//! Empty slots in Settings, Integrations. The always-on host card is built by its own work item,
//! so its slot still renders nothing and the page keeps its layout.

use leptos::prelude::*;

use crate::components::telegram_card::TelegramCard;

#[component]
pub fn TelegramSettingsSlot() -> impl IntoView {
    view! { <TelegramCard /> }
}

#[component]
pub fn HostCardSlot() -> impl IntoView {}
