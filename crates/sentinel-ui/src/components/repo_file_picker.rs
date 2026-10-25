//! `PickerDialog`: the add dialogs for files, a folder, and a backlog file. The folder dialog is a
//! browser (`folder_browser.rs`). The "This project" tab of the other two lists the repository from
//! `GET /api/files`. The "This device" tab reads chosen files with `FileReader` and sends them as
//! base64 (`docs/ui-rules.md`, Composer).

use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::AddAttachment;
use sentinel_core::{AttachmentId, AttachmentKind, ChatId};

use crate::api::{self, ApiFailure};
use crate::chats::{PickerKind, filter_files, same_kind_ids, too_large};
use crate::components::attachment_chip::Chip;
use crate::components::device_file::{DeviceFile, input_files, list_files, take_files};
use crate::components::folder_browser::FolderBrowser;
use crate::components::icon::Icon;
use crate::state::{ToastKind, Ui, use_ui};
use crate::views::composer::reload_attachments;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Project,
    Device,
}

/// What the repository file list request returned.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FileList {
    Loading,
    Ready(Vec<String>),
    Unavailable,
}

/// A device file that goes to the daemon, already base64 encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Upload {
    name: String,
    content: String,
}

/// Everything one confirm sends: the chosen paths, the uploads, and the chips a single choice
/// replaces.
struct Selection {
    chat: ChatId,
    kind: PickerKind,
    picked: Vec<String>,
    uploads: Vec<Upload>,
    replaced: Vec<AttachmentId>,
}

fn title_for(kind: PickerKind) -> &'static str {
    match kind {
        PickerKind::Files => "Add files",
        PickerKind::Folder => "Add a folder",
        PickerKind::Backlog => "Use a backlog file",
    }
}

fn icon_for(kind: PickerKind) -> &'static str {
    match kind {
        PickerKind::Files => "file",
        PickerKind::Folder => "folder",
        PickerKind::Backlog => "tasks",
    }
}

#[component]
pub fn PickerDialog(kind: PickerKind) -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="modal-hd">
            <Icon name=icon_for(kind) size=18 />
            <h3>{title_for(kind)}</h3>
            <button class="ibtn" aria-label="Close" on:click=move |_| ui.modal.set(None)>
                <Icon name="x" size=17 />
            </button>
        </div>
        {match kind {
            PickerKind::Folder => view! { <FolderBrowser /> }.into_any(),
            PickerKind::Files | PickerKind::Backlog => view! { <ChoicePicker kind=kind /> }.into_any(),
        }}
    }
}

/// The file and backlog dialogs: a project list or device files, and a confirm button.
#[component]
fn ChoicePicker(kind: PickerKind) -> impl IntoView {
    let ui = use_ui();
    let tab = RwSignal::new(Tab::Project);
    let filter = RwSignal::new(String::new());
    let picked = RwSignal::new(Vec::<String>::new());
    let files = RwSignal::new(FileList::Loading);
    let devices = RwSignal::new(Vec::<DeviceFile>::new());
    load_files(files);
    let count = Memo::new(move |_| {
        picked.with(Vec::len)
            + devices.with(|list| list.iter().filter(|file| file.content.is_some()).count())
    });
    view! {
        <div class="modal-body" style="padding:1em 1.1em">
            {kind.has_device_tab().then(|| tab_row(tab))}
            {move || if tab.get() == Tab::Device && kind.has_device_tab() {
                view! { <DevicePane kind=kind devices=devices /> }.into_any()
            } else {
                view! { <ProjectPane kind=kind files=files filter=filter picked=picked /> }.into_any()
            }}
        </div>
        <div class="modal-ft">
            <button class="btn" on:click=move |_| ui.modal.set(None)>"Cancel"</button>
            <button class="btn pri" disabled=move || count.get() == 0
                on:click=move |_| confirm(ui, kind, picked.get_untracked(), &devices.get_untracked())>
                {move || kind.confirm_label(count.get())}
            </button>
        </div>
    }
}

/// The two tabs of the dialog. The current one is pressed.
fn tab_row(tab: RwSignal<Tab>) -> impl IntoView {
    let choice = move |which: Tab, label: &'static str| {
        view! {
            <button aria-pressed=move || tab.get() == which
                on:click=move |_| tab.set(which)>{label}</button>
        }
    };
    view! {
        <div class="seg" style="margin-bottom:.8em">
            {choice(Tab::Project, "This project")}
            {choice(Tab::Device, "This device")}
        </div>
    }
}

fn load_files(files: RwSignal<FileList>) {
    spawn_local(async move {
        let next = match api::get::<Vec<String>>("/api/files").await {
            Ok(list) => FileList::Ready(list),
            Err(_) => FileList::Unavailable,
        };
        let _ = files.try_set(next);
    });
}

#[component]
fn ProjectPane(
    kind: PickerKind,
    files: RwSignal<FileList>,
    filter: RwSignal<String>,
    picked: RwSignal<Vec<String>>,
) -> impl IntoView {
    view! {
        <input class="field" placeholder="Filter" aria-label="Filter" style="margin-bottom:.6em"
            prop:value=move || filter.get()
            on:input=move |event| filter.set(event_target_value(&event)) />
        <div>{move || files.with(|state| project_rows(kind, state, &filter.get(), picked))}</div>
    }
}

/// The rows of the project tab: a note while the list loads or cannot load, else the rows.
fn project_rows(
    kind: PickerKind,
    state: &FileList,
    query: &str,
    picked: RwSignal<Vec<String>>,
) -> AnyView {
    match state {
        FileList::Loading => note("Loading the file list."),
        FileList::Unavailable => note("The file list is not available."),
        FileList::Ready(list) => {
            let rows = filter_files(&kind.rows(list), query);
            if rows.is_empty() {
                return note("No match.");
            }
            rows.into_iter()
                .map(|path| pick_row(kind, picked, path))
                .collect_view()
                .into_any()
        }
    }
}

fn note(text: &'static str) -> AnyView {
    view! { <div class="note">{text}</div> }.into_any()
}

/// One row with a checkbox, or a radio button for a single choice.
fn pick_row(kind: PickerKind, picked: RwSignal<Vec<String>>, path: String) -> AnyView {
    let single = kind.single();
    let input_type = if single { "radio" } else { "checkbox" };
    let checked_path = path.clone();
    let change_path = path.clone();
    view! {
        <label class="mi">
            <input type=input_type name="dlgPick" value=path.clone()
                prop:checked=move || picked.with(|list| list.contains(&checked_path))
                on:change=move |event| toggle_pick(picked, single, change_path.clone(), event_target_checked(&event)) />
            <span class="mono">{path}</span>
        </label>
    }
    .into_any()
}

/// A radio replaces the choice. A checkbox adds or removes its path.
fn toggle_pick(picked: RwSignal<Vec<String>>, single: bool, path: String, checked: bool) {
    picked.update(|list| {
        if single {
            *list = if checked { vec![path] } else { Vec::new() };
        } else if checked {
            list.push(path);
        } else {
            list.retain(|chosen| *chosen != path);
        }
    });
}

#[component]
fn DevicePane(kind: PickerKind, devices: RwSignal<Vec<DeviceFile>>) -> impl IntoView {
    let ui = use_ui();
    let input = NodeRef::<html::Input>::new();
    let choose = move |_| {
        if let Some(element) = input.get_untracked() {
            element.click();
        }
    };
    let chosen = move |_| {
        if let Some(element) = input.get_untracked() {
            let files = input_files(&element);
            element.set_value("");
            take_files(ui, devices, kind, files);
        }
    };
    view! {
        <div class="dropzone"
            on:dragover=|event: web_sys::DragEvent| event.prevent_default()
            on:drop=move |event: web_sys::DragEvent| {
                event.prevent_default();
                let dropped = event.data_transfer().and_then(|data| data.files());
                take_files(ui, devices, kind, dropped.map(|list| list_files(&list)).unwrap_or_default());
            }>
            <span>"Drop files here, or choose them"</span>
            <div class="row wrap" style="justify-content:center">
                <button class="btn" on:click=choose>
                    <Icon name="upload" size=15 />" Choose files"
                </button>
            </div>
        </div>
        <input type="file" multiple=kind != PickerKind::Backlog hidden="hidden" node_ref=input on:change=chosen />
        <div class="chips" style="margin-top:.8em">
            {move || devices.get().into_iter().map(device_chip).collect_view()}
        </div>
    }
}

fn device_chip(file: DeviceFile) -> impl IntoView {
    let warn = too_large(file.size);
    view! { <Chip icon="upload" name=file.name warn=warn /> }
}

fn uploads_of(devices: &[DeviceFile]) -> Vec<Upload> {
    devices
        .iter()
        .filter_map(|file| {
            file.content.clone().map(|content| Upload {
                name: file.name.clone(),
                content,
            })
        })
        .collect()
}

/// Closes the dialog and adds the choice. A single choice first removes the chips of its kind.
pub(crate) fn confirm(ui: Ui, kind: PickerKind, picked: Vec<String>, devices: &[DeviceFile]) {
    let Some(chat) = ui.open_chat.get_untracked() else {
        ui.show_toast("Open a chat first.", ToastKind::Warn);
        return;
    };
    let replaced = if kind.single() {
        ui.attachments
            .with_untracked(|list| same_kind_ids(list, kind.attachment_kind()))
    } else {
        Vec::new()
    };
    let selection = Selection {
        chat,
        kind,
        picked,
        uploads: uploads_of(devices),
        replaced,
    };
    spawn_local(async move {
        match add_all(selection).await {
            Ok(()) => {
                ui.modal.set(None);
                reload_attachments(ui);
            }
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

async fn add_all(selection: Selection) -> Result<(), ApiFailure> {
    let Selection {
        chat,
        kind,
        picked,
        uploads,
        replaced,
    } = selection;
    for id in &replaced {
        api::remove_attachment(&chat, id).await?;
    }
    for path in &picked {
        api::add_attachment(&chat, &path_body(kind, path)).await?;
    }
    for upload in &uploads {
        api::add_attachment(&chat, &upload_body(kind, upload)).await?;
    }
    Ok(())
}

fn path_body(kind: PickerKind, path: &str) -> AddAttachment {
    AddAttachment {
        kind: kind.attachment_kind(),
        path: Some(path.to_owned()),
        text: None,
        name: None,
        content_base64: None,
    }
}

fn upload_body(kind: PickerKind, upload: &Upload) -> AddAttachment {
    let upload_kind = if kind == PickerKind::Backlog {
        AttachmentKind::Backlog
    } else {
        AttachmentKind::Upload
    };
    AddAttachment {
        kind: upload_kind,
        path: None,
        text: None,
        name: Some(upload.name.clone()),
        content_base64: Some(upload.content.clone()),
    }
}
