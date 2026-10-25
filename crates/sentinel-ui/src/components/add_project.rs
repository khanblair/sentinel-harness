//! `AddProjectButton` and the project dialogs: the New project flow, the folder browser for a host
//! with no folder dialog, and the Remove dialog. The daemon keeps the project list. The page
//! asks for it, and a switch of project reloads the page (`docs/ui-rules.md`, Layout).

use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::{AddProject, FolderEntry, ProjectInfo};
use serde::Deserialize;

use crate::api::{self, ApiFailure, Then};
use crate::components::icon::Icon;
use crate::components::project_group::stored_id;
use crate::components::sidebar::new_chat;
use crate::state::{ToastKind, Ui, refresh, use_ui};
use crate::views::folder_picker::list_url;

/// How often the project list reloads while the sidebar is open.
const POLL_EVERY: Duration = Duration::from_secs(8);

/// The dialog the project overlay shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectDialog {
    /// The folder browser, for a host with no folder dialog.
    Add,
    /// Asks before a project leaves the list. The folder itself stays.
    Remove { id: String, name: String },
}

/// `POST /api/fs/pick`. `cancelled` means the owner closed the dialog. `available: false` means
/// no dialog could open on the computer.
#[derive(Deserialize)]
pub struct PickReply {
    path: Option<String>,
    #[serde(default)]
    cancelled: bool,
    #[serde(default = "yes")]
    available: bool,
}

fn yes() -> bool {
    true
}

/// What the page does with the answer of a folder dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickOutcome {
    Add(String),
    Nothing,
    ShowBrowser,
}

/// Only a missing dialog opens the in-app folder box. A cancel does nothing.
pub fn pick_outcome(reply: &PickReply) -> PickOutcome {
    match &reply.path {
        Some(path) => PickOutcome::Add(path.clone()),
        None if reply.cancelled || reply.available => PickOutcome::Nothing,
        None => PickOutcome::ShowBrowser,
    }
}

/// Asks the computer for a folder. A failed call counts as no dialog, so the browser opens.
pub async fn ask_dialog() -> PickOutcome {
    match api::post_empty::<PickReply>("/api/fs/pick").await {
        Ok(reply) => pick_outcome(&reply),
        Err(_) => PickOutcome::ShowBrowser,
    }
}

/// The sidebar's New project button. A phone cannot add folders, so it gets a note instead.
#[component]
pub fn AddProjectButton() -> impl IntoView {
    let ui = use_ui();
    view! {
        <Show
            when=move || !ui.on_phone()
            fallback=|| view! { <div class="note" style="padding:1em">"Add projects from the computer."</div> }
        >
            <div class="sb-new">
                <button class="btn" style="width:100%" aria-label="New project" on:click=move |_| start_add(ui)>
                    <Icon name="plus" size=15 />
                    <span>"New project"</span>
                </button>
            </div>
        </Show>
    }
}

/// The overlay for the project dialogs. It draws over the page like the modal host does.
#[component]
pub fn ProjectDialogHost() -> impl IntoView {
    let ui = use_ui();
    move || {
        ui.project_dialog.get().map(|dialog| {
            view! {
                <div class="ov" id="ov-project" on:click=move |_| ui.project_dialog.set(None)>
                    <div class="modal sm" role="dialog" on:click=move |event| event.stop_propagation()>
                        {dialog_body(dialog)}
                    </div>
                </div>
            }
        })
    }
}

fn dialog_body(dialog: ProjectDialog) -> AnyView {
    match dialog {
        ProjectDialog::Add => view! { <AddProjectDialog /> }.into_any(),
        ProjectDialog::Remove { id, name } => {
            view! { <RemoveProjectDialog id=id name=name /> }.into_any()
        }
    }
}

/// What the folder box lists: the folders in view, or the daemon's message when a call fails.
#[derive(Clone, Copy)]
struct BrowserList {
    entries: RwSignal<Vec<FolderEntry>>,
    problem: RwSignal<Option<String>>,
}

/// The folder browser. It starts in the home folder. Its "Use this folder" button adds the
/// folder as a project.
#[component]
fn AddProjectDialog() -> impl IntoView {
    let ui = use_ui();
    let trail = RwSignal::new(Vec::<FolderEntry>::new());
    let list = BrowserList {
        entries: RwSignal::new(Vec::new()),
        problem: RwSignal::new(None),
    };
    Effect::new(move |_| load_folders(&trail.get(), list));
    view! {
        <div class="modal-hd">
            <Icon name="folder" size=18 />
            <h3>"Add a project"</h3>
        </div>
        <div class="modal-body" style="padding:0">
            <div class="fd-crumb">
                <button
                    class="btn sm"
                    disabled=move || trail.with(Vec::is_empty)
                    on:click=move |_| trail.update(|opened| { opened.pop(); })
                >
                    "Up"
                </button>
                <span class="mono">{move || trail_label(&trail.get())}</span>
            </div>
            <div class="fd-list" role="listbox">
                {move || {
                    box_note(list.problem.get(), list.entries.with(Vec::len))
                        .map(|text| view! { <div class="note">{text}</div> })
                }}
                {move || list.entries.get().into_iter().map(|entry| folder_option(trail, entry)).collect_view()}
            </div>
        </div>
        <div class="modal-ft">
            <button class="btn" on:click=move |_| ui.project_dialog.set(None)>"Cancel"</button>
            <button
                class="btn pri"
                disabled=move || trail.with(Vec::is_empty)
                on:click=move |_| spawn_local(add_path(ui, trail_path(&trail.get_untracked())))
            >
                "Use this folder"
            </button>
        </div>
    }
}

fn folder_option(trail: RwSignal<Vec<FolderEntry>>, entry: FolderEntry) -> impl IntoView {
    let next = entry.clone();
    view! {
        <button class="fd-row" role="option" on:click=move |_| trail.update(|opened| opened.push(next.clone()))>
            <Icon name="folder" size=17 />
            <span>{entry.name}</span>
        </button>
    }
}

/// Lists the folder the trail ends in. A failed call shows its text in the box.
fn load_folders(trail: &[FolderEntry], list: BrowserList) {
    let path = trail_path(trail);
    spawn_local(async move {
        match api::get::<Vec<FolderEntry>>(&list_url(&path)).await {
            Ok(folders) => {
                list.entries.set(folders);
                list.problem.set(None);
            }
            Err(failure) => {
                list.entries.set(Vec::new());
                list.problem.set(Some(failure_text(&failure)));
            }
        }
    });
}

/// The folder the trail ends in, as the daemon reads it. An empty trail is the home folder,
/// which the daemon reads from an empty path.
pub fn trail_path(trail: &[FolderEntry]) -> String {
    trail
        .last()
        .map_or_else(String::new, |folder| folder.path.clone())
}

/// The location the box shows. The home folder shows as `~`.
pub fn trail_label(trail: &[FolderEntry]) -> String {
    if trail.is_empty() {
        "~".to_owned()
    } else {
        trail_path(trail)
    }
}

/// The message the box shows above its list: a failure, or a note when the folder is empty.
pub fn box_note(problem: Option<String>, count: usize) -> Option<String> {
    match problem {
        Some(text) => Some(text),
        None if count == 0 => Some("No subfolders here.".to_owned()),
        None => None,
    }
}

/// Asks before the project leaves the list. The folder is never touched.
#[component]
fn RemoveProjectDialog(id: String, name: String) -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="modal-hd">
            <span style="color:var(--danger)"><Icon name="trash" size=18 /></span>
            <h3>"Remove this project?"</h3>
        </div>
        <div class="modal-body" style="padding:1em 1.1em">
            <p style="margin:0;line-height:1.6">
                {format!("{name} leaves the list. Its folder and files stay where they are.")}
            </p>
        </div>
        <div class="modal-ft">
            <button class="btn" on:click=move |_| ui.project_dialog.set(None)>"Cancel"</button>
            <button class="btn dng" on:click=move |_| remove_project(ui, id.clone())>
                "Remove from the list"
            </button>
        </div>
    }
}

/// Fetches the project list. A list that no longer holds the active project means the project
/// was removed elsewhere, so the page reloads on the primary project.
pub fn load_projects(ui: Ui) {
    spawn_local(async move {
        let Ok(list) = api::get::<Vec<ProjectInfo>>("/api/projects").await else {
            return;
        };
        if active_is_missing(api::project().as_deref(), &list) {
            switch_project(None, &Then::Nothing);
            return;
        }
        if ui.projects.with_untracked(|current| *current != list) {
            ui.projects.set(list);
        }
    });
}

/// Loads the list now, then every few seconds while the sidebar is mounted.
pub fn poll_projects(ui: Ui) {
    load_projects(ui);
    if let Ok(timer) = set_interval_with_handle(move || load_projects(ui), POLL_EVERY) {
        on_cleanup(move || timer.clear());
    }
}

/// True when the active project id is set and is not in the list.
fn active_is_missing(active: Option<&str>, list: &[ProjectInfo]) -> bool {
    active.is_some_and(|id| !list.iter().any(|project| project.id == id))
}

/// Makes `id` the active project and reloads the page, so every panel starts fresh for it. `None`
/// is the primary project. `then` says what the page does once it has loaded.
pub fn switch_project(id: Option<&str>, then: &Then) {
    api::set_project(id);
    api::save_wish(then);
    if let Some(window) = web_sys::window() {
        let _ = window.location().reload();
    }
}

/// Carries out the wish a project switch left behind. It runs once, on the first snapshot.
pub fn carry_out_wish(ui: Ui) {
    match api::take_wish() {
        Some(Then::OpenChat(chat)) => {
            ui.open(chat);
            refresh(ui);
        }
        Some(Then::NewChat) => new_chat(ui),
        Some(Then::Nothing) | None => {}
    }
}

/// The New project flow. The folder dialog of the computer answers first. The folder browser
/// opens only when no dialog can open.
pub fn start_add(ui: Ui) {
    spawn_local(async move {
        match ask_dialog().await {
            PickOutcome::Add(path) => add_path(ui, path).await,
            PickOutcome::ShowBrowser => ui.project_dialog.set(Some(ProjectDialog::Add)),
            PickOutcome::Nothing => {}
        }
    });
}

/// Adds a folder as a project and switches to it. A refusal stays on screen as a toast.
pub async fn add_path(ui: Ui, path: String) {
    match api::post::<_, ProjectInfo>("/api/projects", &AddProject { path }).await {
        Ok(project) => {
            ui.project_dialog.set(None);
            ui.show_toast(format!("Added {}", project.name), ToastKind::Ok);
            let target = stored_id(&project.id, project.primary);
            switch_project(target.as_deref(), &Then::Nothing);
        }
        Err(failure) => ui.show_toast(failure_text(&failure), ToastKind::Danger),
    }
}

/// Removes a project from the list. Removing the active one switches to the primary project.
fn remove_project(ui: Ui, id: String) {
    ui.project_dialog.set(None);
    let was_active = api::project().as_deref() == Some(id.as_str());
    spawn_local(async move {
        let path = format!("/api/projects/{id}");
        match api::send::<_, serde_json::Value>("DELETE", &path, &serde_json::json!({})).await {
            Ok(_) if was_active => switch_project(None, &Then::Nothing),
            Ok(_) => load_projects(ui),
            Err(failure) => ui.show_toast(failure_text(&failure), ToastKind::Danger),
        }
    });
}

/// The text for a failed project call. A refusal shows the daemon's own message.
pub fn failure_text(failure: &ApiFailure) -> String {
    match failure {
        ApiFailure::Status { message, .. } => message.clone(),
        ApiFailure::Network(_) | ApiFailure::Decode(_) => failure.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::ChatInfo;

    fn project(id: &str) -> ProjectInfo {
        ProjectInfo {
            id: id.to_owned(),
            name: id.to_owned(),
            path: format!("/work/{id}"),
            primary: false,
            live: false,
            chats: Vec::<ChatInfo>::new(),
        }
    }

    #[test]
    fn no_active_project_is_never_missing() {
        assert!(!active_is_missing(None, &[project("p1")]));
    }

    #[test]
    fn an_active_project_in_the_list_is_not_missing() {
        assert!(!active_is_missing(Some("p1"), &[project("p1")]));
    }

    #[test]
    fn an_active_project_left_out_of_the_list_is_missing() {
        assert!(active_is_missing(Some("gone"), &[project("p1")]));
    }

    #[test]
    fn a_refusal_shows_only_the_daemon_message() {
        let refusal = ApiFailure::Status {
            code: 409,
            message: "already in the list".to_owned(),
        };
        assert_eq!(failure_text(&refusal), "already in the list");
    }

    #[test]
    fn a_network_failure_keeps_its_description() {
        let failure = ApiFailure::Network("timeout".to_owned());
        assert!(failure_text(&failure).starts_with("Could not reach the daemon."));
    }

    fn reply(json: &str) -> PickReply {
        serde_json::from_str(json).unwrap_or_else(|error| panic!("bad reply: {error}"))
    }

    #[test]
    fn a_chosen_folder_is_added() {
        let answer = reply(r#"{"path":"/work/app","cancelled":false,"available":true}"#);
        assert_eq!(
            pick_outcome(&answer),
            PickOutcome::Add("/work/app".to_owned())
        );
    }

    #[test]
    fn a_cancelled_dialog_does_nothing_at_all() {
        let answer = reply(r#"{"path":null,"cancelled":true,"available":true}"#);
        assert_eq!(pick_outcome(&answer), PickOutcome::Nothing);
    }

    #[test]
    fn a_missing_dialog_opens_the_folder_box() {
        let answer = reply(r#"{"path":null,"cancelled":false,"available":false}"#);
        assert_eq!(pick_outcome(&answer), PickOutcome::ShowBrowser);
    }

    #[test]
    fn an_answer_without_the_new_fields_counts_as_an_available_dialog() {
        let answer = reply(r#"{"path":null}"#);
        assert_eq!(pick_outcome(&answer), PickOutcome::Nothing);
    }

    fn folder(name: &str) -> FolderEntry {
        FolderEntry {
            name: name.to_owned(),
            path: format!("/Users/me/{name}"),
        }
    }

    #[test]
    fn an_empty_trail_is_the_home_folder() {
        assert_eq!(trail_path(&[]), "");
        assert_eq!(trail_label(&[]), "~");
    }

    #[test]
    fn the_trail_shows_the_folder_it_ends_in() {
        let trail = [folder("code"), folder("app")];
        assert_eq!(trail_path(&trail), "/Users/me/app");
        assert_eq!(trail_label(&trail), "/Users/me/app");
    }

    #[test]
    fn up_removes_the_last_folder_and_stops_at_home() {
        let mut trail = vec![folder("code")];
        trail.pop();
        assert_eq!(trail_path(&trail), "");
        assert_eq!(trail_label(&trail), "~");
    }

    #[test]
    fn a_failure_message_shows_in_the_box_first() {
        let note = box_note(Some("Not in the home folder".to_owned()), 0);
        assert_eq!(note.as_deref(), Some("Not in the home folder"));
    }

    #[test]
    fn an_empty_folder_says_so_instead_of_an_empty_box() {
        assert_eq!(box_note(None, 0).as_deref(), Some("No subfolders here."));
        assert_eq!(box_note(None, 2), None);
    }
}
