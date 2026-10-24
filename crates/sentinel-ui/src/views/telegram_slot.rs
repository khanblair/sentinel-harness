//! Empty slots in Settings, Integrations. The Telegram card and the always-on host card are built
//! by their own work items. Until then these render nothing, so the page keeps its layout.

use leptos::prelude::*;

#[component]
pub fn TelegramSettingsSlot() -> impl IntoView {}

#[component]
pub fn HostCardSlot() -> impl IntoView {}
