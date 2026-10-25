//! Settings pages: General (repository and wake lock) and Brain. The other pages live in
//! `views/settings.rs` and in `components/` (Agents and tools, Budget, Connections, Devices).

use std::collections::BTreeMap;

use crate::api;
use crate::components::add_project::start_add;
use crate::components::icon::Icon;
use crate::components::reset_section::ResetSection;
use crate::components::toggle::Toggle;
use crate::components::tour_menu::TourMenu;
use crate::state::{Modal, ToastKind, Ui, use_ui};
use crate::views::brain_settings::BrainSettings;
use crate::views::settings::Field;
use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::*;
use leptos::task::spawn_local;

const WAKE_LOCK_KEY: &str = "sentinel.wake_lock";

#[component]
pub fn GeneralPage() -> impl IntoView {
    let ui = use_ui();
    view! {
        <h2>"General"</h2>
        <p class="lead">"Where Sentinel works and how it starts."</p>
        <RepositoryField />
        <WakeLockField />
        <QuitField />
        <Field label="Product tour" hint="Seven stops through the sidebar, panels, and permissions.">
            <TourMenu />
        </Field>
        <div class="row wrap" style="margin-top:1em">
            <button class="btn" on:click=move |_| ui.modal.set(Some(Modal::Guide))>
                "How to use Sentinel"
            </button>
        </div>
        <ResetSection />
    }
}

/// The projects in the list, and the folder of the active project. Adding a project runs the same
/// flow as the sidebar's New project button.
#[component]
fn RepositoryField() -> impl IntoView {
    let ui = use_ui();
    let count = move || {
        let total = ui.projects.with(Vec::len);
        format!("{total} in the list")
    };
    let folder = move || {
        ui.snapshot
            .get()
            .repo
            .map_or_else(|| "No folder chosen".to_owned(), |repo| repo.path)
    };
    view! {
        <Field label="Projects" hint="Each folder is a project with its own chats.">
            <span class="row wrap" style="justify-content:flex-end">
                <span class="k">{count}</span>
                <button class="btn" on:click=move |_| add_from_settings(ui)>
                    <Icon name="folder" size=15 />
                    " Add a project"
                </button>
            </span>
        </Field>
        <div class="meta">{folder}</div>
    }
}

/// Closes Settings, so the project dialog is not hidden under it. A phone cannot add folders.
fn add_from_settings(ui: Ui) {
    if ui.on_phone() {
        ui.show_toast("Add projects from the computer.", ToastKind::Info);
        return;
    }
    ui.modal.set(None);
    start_add(ui);
}

#[component]
fn WakeLockField() -> impl IntoView {
    let on = RwSignal::new(load_wake_lock());
    view! {
        <Field label="Wake lock" hint="Keeps the screen on while runs are live. Stored on this device.">
            <Toggle checked=on on_change=Callback::new(move |next: bool| set_wake_lock(on, next)) />
        </Field>
    }
}

/// Stops the daemon. The app has no terminal, so this is how the owner switches Sentinel off.
#[component]
fn QuitField() -> impl IntoView {
    let ui = use_ui();
    let phone = move || ui.on_phone();
    view! {
        <Field label="Quit Sentinel" hint="Stops the daemon on this computer. Runs, routines, and Telegram stop with it.">
            <button class="btn dng" disabled=phone title=move || if phone() { "Needs the computer" } else { "" }
                on:click=move |_| quit(ui)>
                "Quit"
            </button>
        </Field>
    }
}

fn quit(ui: Ui) {
    spawn_local(async move {
        let result = api::post_empty::<serde_json::Value>("/api/quit").await;
        if let Err(failure) = result {
            ui.show_toast(failure.to_string(), ToastKind::Danger);
        }
    });
}

fn load_wake_lock() -> bool {
    LocalStorage::get::<bool>(WAKE_LOCK_KEY).unwrap_or(false)
}

fn set_wake_lock(on: RwSignal<bool>, next: bool) {
    on.set(next);
    // A blocked store keeps the setting for this visit only.
    let _ = LocalStorage::set(WAKE_LOCK_KEY, next);
}

#[component]
pub fn BrainPage() -> impl IntoView {
    view! { <BrainSettings /> }
}

/// A stored setting, or the fallback when the daemon has not sent that key.
pub fn setting_or(settings: &BTreeMap<String, String>, key: &str, fallback: &str) -> String {
    settings
        .get(key)
        .cloned()
        .unwrap_or_else(|| fallback.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_setting_wins_over_the_fallback() {
        let mut settings = BTreeMap::new();
        settings.insert("care_schedule".to_owned(), "Weekly".to_owned());
        assert_eq!(setting_or(&settings, "care_schedule", "Off"), "Weekly");
        assert_eq!(setting_or(&settings, "care_budget", "Not set"), "Not set");
    }
}
