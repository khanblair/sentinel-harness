//! Settings pages: General (repository and wake lock) and Brain. The other pages live in
//! `views/settings.rs` and in `components/` (Agents and tools, Budget, Connections, Devices).

use std::collections::BTreeMap;

use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::{CheckRow, CheckStatus, RepoCheckRequest, RepoCheckResponse};
use serde::Deserialize;

use crate::api;
use crate::components::icon::Icon;
use crate::components::reset_section::ResetSection;
use crate::components::status_badge::shape_markup;
use crate::components::toggle::Toggle;
use crate::components::top_bar::settle;
use crate::components::tour_menu::TourMenu;
use crate::state::{Modal, ToastKind, Ui, use_ui};
use crate::views::brain_settings::BrainSettings;
use crate::views::settings::Field;

const WAKE_LOCK_KEY: &str = "sentinel.wake_lock";

/// A folder the user picked, with the daemon's checks for it.
#[derive(Debug, Clone)]
struct PickedFolder {
    path: String,
    check: RepoCheckResponse,
}

/// The answer of `POST /api/fs/pick`. `path` is empty when the user cancels the dialog.
#[derive(Deserialize)]
struct PickReply {
    path: Option<String>,
}

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

#[component]
fn RepositoryField() -> impl IntoView {
    let ui = use_ui();
    let picked = RwSignal::new(None::<PickedFolder>);
    let folder = move || {
        ui.snapshot
            .get()
            .repo
            .map_or_else(|| "No folder chosen".to_owned(), |repo| repo.path)
    };
    view! {
        <Field label="Repository" hint="The folder agents work in. One tree, no copies.">
            <span class="row wrap" style="justify-content:flex-end">
                <span class="k">{folder}</span>
                <button class="btn" on:click=move |_| pick_folder(ui, picked)>
                    <Icon name="folder" size=15 />
                    " Choose folder"
                </button>
            </span>
        </Field>
        {move || picked.get().map(|found| check_panel(ui, picked, found))}
    }
}

fn check_panel(
    ui: Ui,
    picked: RwSignal<Option<PickedFolder>>,
    found: PickedFolder,
) -> impl IntoView {
    let usable = found.check.usable;
    let path = found.path.clone();
    let checks = found
        .check
        .rows
        .into_iter()
        .map(|row| check_row(ui, picked, path.clone(), row))
        .collect_view();
    view! {
        <div class="psec">
            <h5>{format!("Checks for {}", found.path)}</h5>
            {checks}
            <button class="btn pri" disabled=!usable on:click=move |_| use_folder(ui, picked, path.clone())>
                "Use this folder"
            </button>
        </div>
    }
}

// Only the starter-backlog row carries a fix (`folders.rs`), so the fix button calls that route.
fn check_row(
    ui: Ui,
    picked: RwSignal<Option<PickedFolder>>,
    path: String,
    row: CheckRow,
) -> impl IntoView {
    let (label, color, shape) = check_look(row.status);
    let style = format!("color:var(--{color})");
    let fix = row.fix.map(|fix_label| {
        view! {
            <button class="btn sm" on:click=move |_| fix_folder(ui, picked, path.clone())>{fix_label}</button>
        }
    });
    view! {
        <div class="tcard">
            <div class="row sp wrap">
                <b>{row.name}</b>
                <span class="stb" style=style>
                    <svg class="shp" viewBox="0 0 12 12" aria-hidden="true" inner_html=shape_markup(shape, color)></svg>
                    <span>{label}</span>
                </span>
            </div>
            <div class="meta">{row.detail}</div>
            {fix}
        </div>
    }
}

/// The label, color, and shape for one folder check.
pub fn check_look(status: CheckStatus) -> (&'static str, &'static str, &'static str) {
    match status {
        CheckStatus::Ok => ("Ready", "ok", "dot"),
        CheckStatus::NeedsAction => ("Needs action", "warn", "ring"),
        CheckStatus::Invalid => ("Not usable", "danger", "oct"),
    }
}

fn pick_folder(ui: Ui, picked: RwSignal<Option<PickedFolder>>) {
    spawn_local(async move {
        match api::post_empty::<PickReply>("/api/fs/pick").await {
            Ok(PickReply { path: Some(path) }) => check_folder(ui, picked, path).await,
            // Cancelling the dialog is not an error, so nothing changes.
            Ok(PickReply { path: None }) => {}
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

async fn check_folder(ui: Ui, picked: RwSignal<Option<PickedFolder>>, path: String) {
    let request = RepoCheckRequest { path: path.clone() };
    match api::post::<_, RepoCheckResponse>("/api/repo/check", &request).await {
        Ok(check) => picked.set(Some(PickedFolder { path, check })),
        Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
    }
}

fn fix_folder(ui: Ui, picked: RwSignal<Option<PickedFolder>>, path: String) {
    spawn_local(async move {
        let request = RepoCheckRequest { path: path.clone() };
        match api::post::<_, serde_json::Value>("/api/repo/starter-backlog", &request).await {
            Ok(_) => {
                ui.show_toast("Starter backlog created.", ToastKind::Ok);
                check_folder(ui, picked, path).await;
            }
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

fn use_folder(ui: Ui, picked: RwSignal<Option<PickedFolder>>, path: String) {
    spawn_local(async move {
        let request = RepoCheckRequest { path };
        let result = api::send::<_, RepoCheckResponse>("PUT", "/api/repo", &request).await;
        if result.is_ok() {
            picked.set(None);
            ui.show_toast("Folder set.", ToastKind::Ok);
        }
        settle(ui, result);
    });
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
    fn a_ready_check_is_ok_and_an_invalid_one_is_not_usable() {
        assert_eq!(check_look(CheckStatus::Ok).0, "Ready");
        assert_eq!(check_look(CheckStatus::Invalid).0, "Not usable");
        assert_eq!(check_look(CheckStatus::NeedsAction).1, "warn");
    }

    #[test]
    fn a_stored_setting_wins_over_the_fallback() {
        let mut settings = BTreeMap::new();
        settings.insert("care_schedule".to_owned(), "Weekly".to_owned());
        assert_eq!(setting_or(&settings, "care_schedule", "Off"), "Weekly");
        assert_eq!(setting_or(&settings, "care_budget", "Not set"), "Not set");
    }
}
