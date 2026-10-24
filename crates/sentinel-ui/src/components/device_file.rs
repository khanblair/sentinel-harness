//! Files chosen on this device. Each file is read with `FileReader` and sent as base64. A file over
//! the limit keeps no content, so it is never sent (`docs/ui-rules.md`, Composer).

use futures::StreamExt;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::chats::{PickerKind, base64_encode, too_large};
use crate::state::{ToastKind, Ui};

/// A file chosen on this device. A file over the limit has no content and is never sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceFile {
    pub name: String,
    pub size: u64,
    pub content: Option<String>,
}

pub fn input_files(element: &web_sys::HtmlInputElement) -> Vec<web_sys::File> {
    element
        .files()
        .map(|list| list_files(&list))
        .unwrap_or_default()
}

pub fn list_files(list: &web_sys::FileList) -> Vec<web_sys::File> {
    (0..list.length())
        .filter_map(|index| list.get(index))
        .collect()
}

/// Reads the chosen files. A single choice keeps only the first file and replaces the last one.
pub fn take_files(
    ui: Ui,
    devices: RwSignal<Vec<DeviceFile>>,
    kind: PickerKind,
    files: Vec<web_sys::File>,
) {
    let limit = if kind.single() { 1 } else { usize::MAX };
    if kind.single() {
        devices.set(Vec::new());
    }
    for file in files.into_iter().take(limit) {
        spawn_local(read_device_file(ui, devices, file));
    }
}

/// Adds one device file. A file over the limit is listed without content. A file that cannot be
/// read is not listed and a toast says so.
pub async fn read_device_file(ui: Ui, devices: RwSignal<Vec<DeviceFile>>, file: web_sys::File) {
    let name = file.name();
    let size = device_size(&file);
    let content = if too_large(size) {
        None
    } else if let Ok(bytes) = read_bytes(&file).await {
        Some(base64_encode(&bytes))
    } else {
        ui.show_toast(format!("{name} could not be read."), ToastKind::Danger);
        return;
    };
    let _ = devices.try_update(|list| {
        list.push(DeviceFile {
            name,
            size,
            content,
        });
    });
}

/// The byte size of a file. The browser reports it as a float, but a file size is a whole number,
/// so its text form parses exactly. A size that does not parse counts as too large.
pub fn device_size(file: &web_sys::File) -> u64 {
    format!("{:.0}", file.size()).parse().unwrap_or(u64::MAX)
}

/// Reads a file's bytes with `FileReader`. The reader's load or error event ends the wait.
pub async fn read_bytes(file: &web_sys::File) -> Result<Vec<u8>, ()> {
    let reader = web_sys::FileReader::new().map_err(|_| ())?;
    let (sender, mut outcome) = futures::channel::mpsc::unbounded::<bool>();
    let loaded_sender = sender.clone();
    let on_load = Closure::once_into_js(move |_: web_sys::Event| {
        let _ = loaded_sender.unbounded_send(true);
    });
    let on_error = Closure::once_into_js(move |_: web_sys::Event| {
        let _ = sender.unbounded_send(false);
    });
    reader.set_onload(Some(on_load.unchecked_ref()));
    reader.set_onerror(Some(on_error.unchecked_ref()));
    reader.read_as_array_buffer(file).map_err(|_| ())?;
    if !outcome.next().await.unwrap_or(false) {
        return Err(());
    }
    let buffer = reader.result().map_err(|_| ())?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}
