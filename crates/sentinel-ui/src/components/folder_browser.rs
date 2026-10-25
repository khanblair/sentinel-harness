//! `FolderBrowser`: the body of "Add a folder". It walks the project one folder at a time with
//! `GET /api/repo/tree`, and attaches the folder the owner stops in. The modal frame lives in
//! `repo_file_picker.rs`.

use std::fmt::Write as _;

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::{RepoTree, TreeFolder};

use crate::api;
use crate::chats::PickerKind;
use crate::components::icon::Icon;
use crate::components::repo_file_picker::confirm;
use crate::state::{Ui, use_ui};

/// What the newest folder request returned.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Listing {
    Loading,
    Ready(RepoTree),
    Failed(String),
}

/// The state of one browser. `current` is the folder on screen, `wanted` the folder the newest
/// request asked for. A reply for any other folder is stale and is dropped.
#[derive(Debug, Clone, Copy)]
struct Browser {
    current: RwSignal<String>,
    wanted: RwSignal<String>,
    listing: RwSignal<Listing>,
}

#[component]
pub fn FolderBrowser() -> impl IntoView {
    let ui = use_ui();
    let browser = Browser {
        current: RwSignal::new(String::new()),
        wanted: RwSignal::new(String::new()),
        listing: RwSignal::new(Listing::Loading),
    };
    open_folder(browser, String::new());
    view! {
        <div class="modal-body" style="padding:1em 1.1em">
            <Crumbs browser=browser />
            <div class="row sp" style="margin:.6em 0">
                <span class="mono sub">{move || path_text(&browser.current.get())}</span>
                <UpButton browser=browser />
            </div>
            <FolderList browser=browser />
        </div>
        <div class="modal-ft">
            <button class="btn" on:click=move |_| ui.modal.set(None)>"Cancel"</button>
            <UseButton browser=browser />
        </div>
    }
}

/// Asks for one folder. The reply is shown only if the owner still wants that folder.
fn open_folder(browser: Browser, target: String) {
    browser.wanted.set(target.clone());
    browser.listing.set(Listing::Loading);
    spawn_local(async move {
        let outcome = api::get::<RepoTree>(&tree_url(&target)).await;
        if browser.wanted.try_get_untracked().as_deref() != Some(target.as_str()) {
            return;
        }
        match outcome {
            Ok(tree) => {
                let _ = browser.current.try_set(tree.path.clone());
                let _ = browser.listing.try_set(Listing::Ready(tree));
            }
            Err(failure) => {
                let _ = browser
                    .listing
                    .try_set(Listing::Failed(failure.to_string()));
            }
        }
    });
}

#[component]
fn Crumbs(browser: Browser) -> impl IntoView {
    let ui = use_ui();
    move || {
        let project = ui
            .snapshot
            .with(|snapshot| snapshot.repo.as_ref().map(|repo| repo.name.clone()))
            .unwrap_or_else(|| "Project".to_owned());
        let path = browser.current.get();
        view! {
            <div class="fd-crumb">
                {crumb(browser, project, String::new())}
                {breadcrumbs(&path)
                    .into_iter()
                    .map(|(label, target)| view! {
                        <Icon name="right" size=12 />
                        {crumb(browser, label, target)}
                    })
                    .collect_view()}
            </div>
        }
    }
}

fn crumb(browser: Browser, label: String, target: String) -> impl IntoView {
    view! {
        <button class="btn sm" style="border:0"
            on:click=move |_| open_folder(browser, target.clone())>{label}</button>
    }
}

#[component]
fn UpButton(browser: Browser) -> impl IntoView {
    move || {
        browser.listing.with(parent_of).map(|parent| {
            view! {
                <button class="btn sm" on:click=move |_| open_folder(browser, parent.clone())>
                    "Up"
                </button>
            }
        })
    }
}

#[component]
fn FolderList(browser: Browser) -> impl IntoView {
    view! {
        <div class="fd-list">
            {move || browser.listing.with(|listing| listing_rows(browser, listing))}
        </div>
    }
}

/// A note while the folders load or fail, a note when the folder is empty, else the rows.
fn listing_rows(browser: Browser, listing: &Listing) -> AnyView {
    match listing {
        Listing::Loading => note("Loading the folders.".to_owned()),
        Listing::Failed(text) => note(text.clone()),
        Listing::Ready(tree) if tree.folders.is_empty() => {
            note("No folders inside this one.".to_owned())
        }
        Listing::Ready(tree) => tree
            .folders
            .iter()
            .map(|folder| folder_row(browser, folder))
            .collect_view()
            .into_any(),
    }
}

fn note(text: String) -> AnyView {
    view! { <div class="note">{text}</div> }.into_any()
}

/// One folder. It is a button, so Enter and Space open it from the keyboard.
fn folder_row(browser: Browser, folder: &TreeFolder) -> impl IntoView {
    let target = folder.path.clone();
    view! {
        <button class="mi" on:click=move |_| open_folder(browser, target.clone())>
            <Icon name="folder" size=17 />
            <span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">
                {folder.name.clone()}
            </span>
            <span class="d">{file_count_text(folder.files)}</span>
        </button>
    }
}

#[component]
fn UseButton(browser: Browser) -> impl IntoView {
    let ui = use_ui();
    let ready = move || {
        browser
            .listing
            .with(|listing| matches!(listing, Listing::Ready(_)))
    };
    view! {
        <button class="btn pri" disabled=move || !ready()
            on:click=move |_| attach(ui, &browser.current.get_untracked())>
            {move || use_label(&browser.current.get())}
        </button>
    }
}

/// Attaches the folder through the same path as the other picker kinds. That also replaces the
/// folder chip the composer already holds.
fn attach(ui: Ui, path: &str) {
    confirm(ui, PickerKind::Folder, vec![attach_path(path)], &[]);
}

/// The request path of one folder. The path is a query value, so it is percent-encoded.
fn tree_url(path: &str) -> String {
    format!("/api/repo/tree?path={}", encode_query_value(path))
}

fn encode_query_value(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~/".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            // Writing to a String cannot fail.
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

/// The folder a row of the listing opens, or `None` at the project root.
fn parent_of(listing: &Listing) -> Option<String> {
    match listing {
        Listing::Ready(tree) => tree.parent.clone(),
        Listing::Loading | Listing::Failed(_) => None,
    }
}

/// The folders above `path`, one crumb each. The label is the folder name and the path is the
/// folder the crumb opens. The project root is not in the list; the caller adds it.
pub fn breadcrumbs(path: &str) -> Vec<(String, String)> {
    path.split('/')
        .filter(|name| !name.is_empty())
        .scan(String::new(), |so_far, name| {
            if !so_far.is_empty() {
                so_far.push('/');
            }
            so_far.push_str(name);
            Some((name.to_owned(), so_far.clone()))
        })
        .collect()
}

/// "1 file", "12 files", or "No files".
pub fn file_count_text(count: u32) -> String {
    match count {
        0 => "No files".to_owned(),
        1 => "1 file".to_owned(),
        n => format!("{n} files"),
    }
}

fn path_text(path: &str) -> String {
    if path.is_empty() {
        "(project root)".to_owned()
    } else {
        path.to_owned()
    }
}

/// Attaching the root sends "." because an empty path is refused by the daemon.
fn attach_path(path: &str) -> String {
    if path.is_empty() {
        ".".to_owned()
    } else {
        path.to_owned()
    }
}

fn use_label(path: &str) -> &'static str {
    if path.is_empty() {
        "Use the whole project"
    } else {
        "Use this folder"
    }
}

#[cfg(test)]
mod tests {
    use sentinel_core::api::RepoTree;

    use super::{
        Listing, attach_path, breadcrumbs, encode_query_value, file_count_text, parent_of,
        tree_url, use_label,
    };

    fn tree_at(path: &str, parent: Option<&str>) -> RepoTree {
        RepoTree {
            path: path.to_owned(),
            parent: parent.map(str::to_owned),
            folders: Vec::new(),
            files_here: 0,
        }
    }

    #[test]
    fn breadcrumbs_list_each_folder_with_the_path_it_opens() {
        assert_eq!(
            breadcrumbs("src/fees"),
            vec![
                ("src".to_owned(), "src".to_owned()),
                ("fees".to_owned(), "src/fees".to_owned()),
            ]
        );
    }

    #[test]
    fn breadcrumbs_of_the_root_are_empty() {
        assert_eq!(breadcrumbs(""), Vec::<(String, String)>::new());
    }

    #[test]
    fn file_count_text_reads_zero_one_and_many() {
        assert_eq!(file_count_text(0), "No files");
        assert_eq!(file_count_text(1), "1 file");
        assert_eq!(file_count_text(12), "12 files");
    }

    #[test]
    fn tree_url_encodes_spaces_and_ampersands_but_keeps_slashes() {
        assert_eq!(
            tree_url("src/my folder&x"),
            "/api/repo/tree?path=src/my%20folder%26x"
        );
        assert_eq!(encode_query_value(""), "");
    }

    #[test]
    fn up_goes_to_the_parent_folder_and_the_root_has_none() {
        let inside = Listing::Ready(tree_at("src/fees", Some("src")));
        assert_eq!(parent_of(&inside), Some("src".to_owned()));
        let top = Listing::Ready(tree_at("src", Some("")));
        assert_eq!(parent_of(&top), Some(String::new()));
        let root = Listing::Ready(tree_at("", None));
        assert_eq!(parent_of(&root), None);
        assert_eq!(parent_of(&Listing::Loading), None);
    }

    #[test]
    fn the_root_button_says_whole_project_and_a_folder_says_this_folder() {
        assert_eq!(use_label(""), "Use the whole project");
        assert_eq!(use_label("src"), "Use this folder");
    }

    #[test]
    fn attaching_the_root_sends_a_dot_and_a_folder_sends_its_path() {
        assert_eq!(attach_path(""), ".");
        assert_eq!(attach_path("src/fees"), "src/fees");
    }
}
