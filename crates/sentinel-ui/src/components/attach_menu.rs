//! The entries of the composer's plus menu. Each entry opens one add dialog
//! (`docs/product-scope.md`, F23; `docs/ui-rules.md`, Composer).

use std::collections::BTreeMap;

use leptos::prelude::*;

use crate::components::icon::Icon;
use crate::state::{Modal, Ui};

/// The backlog name when the settings do not name one.
pub const DEFAULT_BACKLOG: &str = "backlog.md";

/// The configured backlog file name, shown under Use a backlog file.
pub fn backlog_name(settings: &BTreeMap<String, String>) -> String {
    settings
        .get("backlog")
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .map_or_else(|| DEFAULT_BACKLOG.to_owned(), str::to_owned)
}

/// One line of the plus menu.
struct AttachEntry {
    icon: &'static str,
    label: &'static str,
    desc: String,
    modal: Modal,
}

fn entries(backlog: String) -> Vec<AttachEntry> {
    vec![
        AttachEntry {
            icon: "file",
            label: "Add files",
            desc: "From this project or this device".to_owned(),
            modal: Modal::AddFiles,
        },
        AttachEntry {
            icon: "folder",
            label: "Add a folder",
            desc: "A folder of this project".to_owned(),
            modal: Modal::AddFolder,
        },
        AttachEntry {
            icon: "tasks",
            label: "Use a backlog file",
            desc: backlog,
            modal: Modal::AddBacklog,
        },
        AttachEntry {
            icon: "book",
            label: "Use a skill",
            desc: "Adds a skill to the next message".to_owned(),
            modal: Modal::AddSkill,
        },
        AttachEntry {
            icon: "edit",
            label: "Add a context note",
            desc: "Goes into every agent prompt".to_owned(),
            modal: Modal::AddNote,
        },
    ]
}

/// The five plus menu entries, in order.
pub fn attach_items(ui: Ui) -> impl IntoView {
    let backlog = ui
        .snapshot
        .with(|snapshot| backlog_name(&snapshot.settings));
    entries(backlog)
        .into_iter()
        .map(|entry| entry_view(ui, entry))
        .collect_view()
}

fn entry_view(ui: Ui, entry: AttachEntry) -> impl IntoView {
    let modal = entry.modal;
    view! {
        <button class="mi" on:click=move |_| open_dialog(ui, modal.clone())>
            <Icon name=entry.icon size=16 />
            <span>
                <span>{entry.label}</span>
                <div class="d">{entry.desc}</div>
            </span>
        </button>
    }
}

/// Closes the menu and opens the dialog.
fn open_dialog(ui: Ui, modal: Modal) {
    ui.popover.set(None);
    ui.modal.set(Some(modal));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backlog_entry_shows_the_configured_name() {
        let mut settings = BTreeMap::new();
        settings.insert("backlog".to_owned(), "fee-ledger/backlog.md".to_owned());
        assert_eq!(backlog_name(&settings), "fee-ledger/backlog.md");
    }

    #[test]
    fn the_backlog_entry_falls_back_to_the_default_name() {
        let mut settings = BTreeMap::new();
        settings.insert("backlog".to_owned(), "  ".to_owned());
        assert_eq!(backlog_name(&settings), DEFAULT_BACKLOG);
        assert_eq!(backlog_name(&BTreeMap::new()), DEFAULT_BACKLOG);
    }

    #[test]
    fn the_plus_menu_has_five_entries_in_order() {
        let labels: Vec<&str> = entries("backlog.md".to_owned())
            .iter()
            .map(|entry| entry.label)
            .collect();
        assert_eq!(
            labels,
            vec![
                "Add files",
                "Add a folder",
                "Use a backlog file",
                "Use a skill",
                "Add a context note"
            ]
        );
    }
}
