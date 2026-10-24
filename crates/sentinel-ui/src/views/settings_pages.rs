//! Settings pages: General (repository and wake lock) and Brain. The other pages live in
//! `views/settings.rs` and in `components/` (Agents and tools, Budget, Connections, Devices).

use std::collections::BTreeMap;

use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::BrainMode;
use sentinel_core::api::{
    CheckRow, CheckStatus, RepoCheckRequest, RepoCheckResponse, SetBrainMode,
};
use serde::Deserialize;

use crate::api;
use crate::components::icon::Icon;
use crate::components::status_badge::shape_markup;
use crate::components::toggle::Toggle;
use crate::components::top_bar::settle;
use crate::layout::Layout;
use crate::state::{Modal, ToastKind, Ui, use_ui};
use crate::views::settings::Field;

const WAKE_LOCK_KEY: &str = "sentinel.wake_lock";

/// The three Brain modes, with the words the prototype uses.
const BRAIN_MODES: [(BrainMode, &str, &str); 3] = [
    (BrainMode::Off, "Off", "No notes read or written."),
    (
        BrainMode::ReadOnly,
        "Read only",
        "Agents read notes. Nothing is written. Best for live runs.",
    ),
    (
        BrainMode::Learn,
        "Learn",
        "Agents read notes. Each run writes a journal and lesson candidates.",
    ),
];

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
        <div class="row wrap" style="margin-top:1em">
            <button class="btn" on:click=move |_| ui.modal.set(Some(Modal::Guide))>
                "How to use Sentinel"
            </button>
        </div>
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
    let ui = use_ui();
    view! {
        <h2>"Brain"</h2>
        <p class="lead">"A folder of plain markdown notes with links between them. Agents read it as memory, every run can write to it, and you can open the same folder in Obsidian."</p>
        <div class="psec">
            <h5>"Mode"</h5>
            <div class="radios">
                {BRAIN_MODES
                    .into_iter()
                    .map(|(mode, label, hint)| brain_mode_button(ui, mode, label, hint))
                    .collect_view()}
            </div>
        </div>
        <Field label="Vault folder" hint="Plain markdown files. Open it in Obsidian to browse or edit. Edits show up here.">
            <span class="k">{move || setting_or(&ui.snapshot.get().settings, "brain_vault", ".sentinel/brain/")}</span>
        </Field>
        <h2 style="margin-top:1.3em;font-size:1.1em">"Scheduled care"</h2>
        <p class="lead">"Runs on a schedule while Sentinel is on. It refreshes notes from the repository, merges duplicates, summarizes old runs, weakens notes nobody uses, and proposes improvements."</p>
        <Field label="Schedule" hint="How often care runs. Off means it never runs by itself.">
            <span class="k">{move || setting_or(&ui.snapshot.get().settings, "care_schedule", "Off")}</span>
        </Field>
        <Field label="Budget per care run" hint="Separate from run budgets. A care run stops at this amount.">
            <span class="k">{move || setting_or(&ui.snapshot.get().settings, "care_budget", "Not set")}</span>
        </Field>
        <Field label="Apply without asking" hint="Only to notes Sentinel wrote. A note you wrote is never changed without your OK.">
            <span class="k">{move || setting_or(&ui.snapshot.get().settings, "care_auto", "Off")}</span>
        </Field>
    }
}

/// A stored setting, or the fallback when the daemon has not sent that key.
pub fn setting_or(settings: &BTreeMap<String, String>, key: &str, fallback: &str) -> String {
    settings
        .get(key)
        .cloned()
        .unwrap_or_else(|| fallback.to_owned())
}

fn brain_mode_button(
    ui: Ui,
    mode: BrainMode,
    label: &'static str,
    hint: &'static str,
) -> impl IntoView {
    let pressed = move || (ui.snapshot.get().brain_mode == mode).to_string();
    let locked = move || phone_blocks(ui, mode);
    view! {
        <button aria-pressed=pressed disabled=locked on:click=move |_| set_brain_mode(ui, mode)>
            <span><Icon name="brain" size=17 /></span>
            <div>
                <b>{label}</b>
                <div class="d">{hint}</div>
            </div>
        </button>
    }
}

/// On a phone, the Brain can only move toward Off. The daemon refuses the rest too.
fn phone_blocks(ui: Ui, target: BrainMode) -> bool {
    let current = ui.snapshot.get().brain_mode;
    ui.layout.get() == Layout::Phone && !current.phone_may_set(target)
}

fn set_brain_mode(ui: Ui, mode: BrainMode) {
    spawn_local(async move {
        let result =
            api::post::<_, serde_json::Value>("/api/brain/mode", &SetBrainMode { mode }).await;
        settle(ui, result);
    });
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
