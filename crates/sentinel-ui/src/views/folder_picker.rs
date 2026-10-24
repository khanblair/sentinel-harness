//! The repository step: choose the project folder, check it, and fix what the check finds
//! (`docs/product-scope.md`, F8 step 1). The folder dialog runs on the computer. Where there is
//! no dialog (a phone, a headless host) the same step shows a folder browser.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::{FolderEntry, RepoCheckRequest, RepoCheckResponse};
use serde::Deserialize;

use crate::api;
use crate::components::icon::Icon;
use crate::components::repo_checks::{FixAction, RepoChecks, fix_action};
use crate::state::ToastKind;
use crate::views::onboarding::{Wizard, post_or_toast, use_wizard};

/// `POST /api/fs/pick`. A `null` path means the user closed the dialog.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PickedFolder {
    pub path: Option<String>,
}

/// The last part of a folder path, which is the repository's name.
pub fn fs_name(path: &str) -> &str {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
}

/// The folder above `path`. The browser never goes above the home folder.
pub fn parent_path(path: &str) -> String {
    match path.trim_end_matches('/').rsplit_once('/') {
        Some(("", _)) => "/".to_string(),
        Some((parent, _)) => parent.to_string(),
        None => "~".to_string(),
    }
}

/// Percent-encodes a path for a query string. Slashes and tildes stay readable.
pub fn encode_query(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                char::from(byte).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

pub fn list_url(path: &str) -> String {
    format!("/api/fs/list?path={}", encode_query(path))
}

/// Runs the repository check for a folder and keeps its rows.
pub fn check_repo(wizard: &Wizard, path: String) {
    let wizard = *wizard;
    spawn_local(async move {
        let body = RepoCheckRequest { path };
        match api::post::<_, RepoCheckResponse>("/api/repo/check", &body).await {
            Ok(response) => wizard.repo.set(Some(response)),
            Err(error) => wizard.ui.show_toast(error.to_string(), ToastKind::Danger),
        }
    });
}

/// A fix button. The backlog row creates the starter file first; then the check runs again.
fn run_fix(wizard: &Wizard, name: String) {
    let wizard = *wizard;
    let Some(path) = wizard.repo_path() else {
        return;
    };
    spawn_local(async move {
        if fix_action(&name) == FixAction::CreateBacklog
            && !post_or_toast(wizard.ui, "/api/repo/starter-backlog").await
        {
            return;
        }
        check_repo(&wizard, path);
    });
}

/// Opens the folder dialog on the computer. When it cannot open (no dialog here), the browser
/// opens instead.
fn choose_with_dialog(wizard: &Wizard, browsing: RwSignal<bool>) {
    let wizard = *wizard;
    spawn_local(async move {
        match api::post_empty::<PickedFolder>("/api/fs/pick").await {
            Ok(PickedFolder { path: Some(path) }) => {
                browsing.set(false);
                check_repo(&wizard, path);
            }
            Ok(PickedFolder { path: None }) => {}
            Err(_) => browsing.set(true),
        }
    });
}

#[component]
pub fn RepositoryStep() -> impl IntoView {
    let wizard = use_wizard();
    let browsing = RwSignal::new(false);
    Effect::new(move |_| {
        if wizard.repo.with(Option::is_some) {
            return;
        }
        if let Some(path) = wizard
            .ui
            .snapshot
            .with(|s| s.repo.as_ref().map(|r| r.path.clone()))
        {
            check_repo(&wizard, path);
        }
    });
    view! {
        <h2>"Choose the repository"</h2>
        <p class="lead">
            "Pick the project folder your agents will work in. Sentinel uses this one folder and makes no copies of it."
        </p>
        <button class="btn pri" on:click=move |_| choose_with_dialog(&wizard, browsing)>
            <Icon name="folder" size=16 />
            " Choose a folder on this computer"
        </button>
        <Show when=move || browsing.get() fallback=|| ()>
            <FolderBrowser browsing=browsing />
        </Show>
        <RepoCard />
    }
}

/// The folder browser for a phone or a headless host. It lists folders under the home folder.
#[component]
fn FolderBrowser(browsing: RwSignal<bool>) -> impl IntoView {
    let wizard = use_wizard();
    let path = RwSignal::new("~".to_string());
    let entries = RwSignal::new(Vec::<FolderEntry>::new());
    Effect::new(move |_| load_folders(&wizard, path.get(), entries));
    view! {
        <div class="card" style="margin-top:1em">
            <div class="fd-crumb">
                <button class="btn sm" on:click=move |_| path.set(parent_path(&path.get_untracked()))>
                    "Up"
                </button>
                <span class="mono">{move || path.get()}</span>
            </div>
            <div class="fd-list" role="listbox">
                {move || {
                    entries
                        .get()
                        .into_iter()
                        .map(|entry| {
                            let next = entry.path.clone();
                            view! {
                                <button class="fd-row" role="option" on:click=move |_| path.set(next.clone())>
                                    <Icon name="folder" size=17 />
                                    <span>{entry.name}</span>
                                </button>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <BrowserFooter path=path browsing=browsing />
        </div>
    }
}

/// The browser footer: the folder it is on, and the button that chooses it.
#[component]
fn BrowserFooter(path: RwSignal<String>, browsing: RwSignal<bool>) -> impl IntoView {
    let wizard = use_wizard();
    view! {
        <div class="fd-ft">
            <span class="sub">{move || path.get()}</span>
            <button
                class="btn pri"
                on:click=move |_| {
                    browsing.set(false);
                    check_repo(&wizard, path.get_untracked());
                }
            >
                {move || format!("Choose {}", fs_name(&path.get()))}
            </button>
        </div>
    }
}

fn load_folders(wizard: &Wizard, path: String, entries: RwSignal<Vec<FolderEntry>>) {
    let ui = wizard.ui;
    spawn_local(async move {
        match api::get::<Vec<FolderEntry>>(&list_url(&path)).await {
            Ok(list) => entries.set(list),
            Err(error) => ui.show_toast(error.to_string(), ToastKind::Danger),
        }
    });
}

/// The chosen folder and its check rows.
#[component]
fn RepoCard() -> impl IntoView {
    let wizard = use_wizard();
    let on_fix = Callback::new(move |name: String| run_fix(&wizard, name));
    move || {
        wizard.repo.get().map(|repo| {
            view! {
                <div class="card" style="margin-top:1em">
                    <div class="row sp wrap" style="margin-bottom:.3em">
                        <b>{fs_name(&repo.path).to_string()}</b>
                        <span class="k">{repo.path.clone()}</span>
                    </div>
                    <RepoChecks rows=repo.rows on_fix=on_fix />
                </div>
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_repository_name_is_the_last_folder() {
        assert_eq!(fs_name("~/code/fee-ledger-api"), "fee-ledger-api");
        assert_eq!(fs_name("~/code/wallet-app/"), "wallet-app");
    }

    #[test]
    fn the_parent_of_a_folder_is_the_folder_above_it() {
        assert_eq!(parent_path("~/code/wallet-app"), "~/code");
        assert_eq!(parent_path("~/code"), "~");
        assert_eq!(parent_path("~"), "~");
    }

    #[test]
    fn spaces_are_encoded_and_slashes_are_kept() {
        assert_eq!(encode_query("~/my code"), "~/my%20code");
        assert_eq!(list_url("~/code"), "/api/fs/list?path=~/code");
    }
}
