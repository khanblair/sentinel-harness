//! The lines of a chat menu: what each line does, and which lines are disabled and why. The menu
//! itself is `components/chat_menu.rs`.

use sentinel_core::ChatInfo;

/// What a chat menu entry does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Rename,
    TogglePin,
    ToggleArchive,
    Delete,
    ExportReport,
    StopRun,
}

/// One clickable line of a chat menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMenuItem {
    pub action: MenuAction,
    pub icon: &'static str,
    pub label: String,
    /// Shown under the label. For a disabled entry it says why.
    pub note: Option<&'static str>,
    pub disabled: bool,
    pub danger: bool,
}

/// A line of a chat menu, or the line between two groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    Item(ChatMenuItem),
    Separator,
}

const NEEDS_COMPUTER: &str = "Needs the computer";
const STOP_FIRST: &str = "Stop the run first.";
const CHECKPOINTS_KEPT: &str = "Checkpoints are kept";

fn item(action: MenuAction, icon: &'static str, label: impl Into<String>) -> ChatMenuItem {
    ChatMenuItem {
        action,
        icon,
        label: label.into(),
        note: None,
        disabled: false,
        danger: false,
    }
}

fn rename_entry() -> MenuEntry {
    MenuEntry::Item(item(MenuAction::Rename, "edit", "Rename"))
}

fn pin_entry(chat: &ChatInfo) -> MenuEntry {
    let label = if chat.pinned { "Unpin" } else { "Pin" };
    MenuEntry::Item(item(MenuAction::TogglePin, "pin", label))
}

/// Archive is blocked while the chat holds the live run. Unarchive is always allowed.
fn archive_entry(chat: &ChatInfo, live: bool) -> MenuEntry {
    let label = if chat.archived {
        "Unarchive"
    } else {
        "Archive"
    };
    let mut entry = item(MenuAction::ToggleArchive, "archive", label);
    if live && !chat.archived {
        entry.disabled = true;
        entry.note = Some(STOP_FIRST);
    }
    MenuEntry::Item(entry)
}

/// Delete is blocked on a phone and while the chat holds the live run.
fn delete_entry(live: bool, phone: bool) -> MenuEntry {
    let mut entry = item(MenuAction::Delete, "trash", "Delete");
    entry.danger = true;
    if phone {
        entry.disabled = true;
        entry.note = Some(NEEDS_COMPUTER);
    } else if live {
        entry.disabled = true;
        entry.note = Some(STOP_FIRST);
    }
    MenuEntry::Item(entry)
}

/// The menu of a chat row: Rename, Pin or Unpin, Archive or Unarchive, a line, then Delete.
pub fn menu_items(chat: &ChatInfo, live: bool, phone: bool) -> Vec<MenuEntry> {
    vec![
        rename_entry(),
        pin_entry(chat),
        archive_entry(chat, live),
        MenuEntry::Separator,
        delete_entry(live, phone),
    ]
}

/// The menu of the top bar title: the row menu with Export report, and Stop run while the chat
/// holds the live run. Both sit above the archive line.
pub fn title_menu_items(chat: &ChatInfo, live: bool, phone: bool) -> Vec<MenuEntry> {
    let mut entries = vec![rename_entry(), pin_entry(chat)];
    entries.push(MenuEntry::Item(item(
        MenuAction::ExportReport,
        "download",
        "Export report",
    )));
    if live {
        let mut stop = item(MenuAction::StopRun, "stop", "Stop run");
        stop.danger = true;
        stop.note = Some(CHECKPOINTS_KEPT);
        entries.push(MenuEntry::Item(stop));
    }
    entries.push(MenuEntry::Separator);
    entries.push(archive_entry(chat, live));
    entries.push(delete_entry(live, phone));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{ChatId, ChatRunState};
    use time::OffsetDateTime;

    fn chat(id: &str, project: &str, title: &str) -> ChatInfo {
        ChatInfo {
            id: ChatId::new(id),
            project: project.to_owned(),
            title: title.to_owned(),
            pinned: false,
            archived: false,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
            run: ChatRunState::Idle,
            verified: 0,
            total: 0,
            needs_you: false,
            agent: None,
        }
    }

    fn labels(entries: &[MenuEntry]) -> Vec<String> {
        entries
            .iter()
            .map(|entry| match entry {
                MenuEntry::Item(item) => item.label.clone(),
                MenuEntry::Separator => "-".to_owned(),
            })
            .collect()
    }

    fn item_of(entries: &[MenuEntry], action: MenuAction) -> ChatMenuItem {
        entries
            .iter()
            .find_map(|entry| match entry {
                MenuEntry::Item(item) if item.action == action => Some(item.clone()),
                MenuEntry::Item(_) | MenuEntry::Separator => None,
            })
            .expect("the menu has this action")
    }

    #[test]
    fn the_row_menu_has_rename_pin_archive_then_delete() {
        let entries = menu_items(&chat("c1", "api", "Fees"), false, false);
        assert_eq!(
            labels(&entries),
            vec!["Rename", "Pin", "Archive", "-", "Delete"]
        );
    }

    #[test]
    fn pin_and_archive_labels_follow_the_chat_state() {
        let mut sample = chat("c1", "api", "Fees");
        sample.pinned = true;
        sample.archived = true;
        let entries = menu_items(&sample, false, false);
        assert_eq!(labels(&entries)[1], "Unpin");
        assert_eq!(labels(&entries)[2], "Unarchive");
    }

    #[test]
    fn delete_is_blocked_on_a_phone_and_while_live() {
        let sample = chat("c1", "api", "Fees");
        let phone = item_of(&menu_items(&sample, false, true), MenuAction::Delete);
        assert!(phone.disabled);
        assert_eq!(phone.note, Some("Needs the computer"));
        let live = item_of(&menu_items(&sample, true, false), MenuAction::Delete);
        assert!(live.disabled);
        assert_eq!(live.note, Some("Stop the run first."));
        let idle = item_of(&menu_items(&sample, false, false), MenuAction::Delete);
        assert!(!idle.disabled);
        assert!(idle.danger);
    }

    #[test]
    fn archive_is_blocked_while_live_but_unarchive_is_not() {
        let sample = chat("c1", "api", "Fees");
        let live = item_of(&menu_items(&sample, true, false), MenuAction::ToggleArchive);
        assert!(live.disabled);
        assert_eq!(live.note, Some("Stop the run first."));
        let mut old = sample.clone();
        old.archived = true;
        let unarchive = item_of(&menu_items(&old, true, false), MenuAction::ToggleArchive);
        assert!(!unarchive.disabled);
    }

    #[test]
    fn the_title_menu_adds_export_and_stop_above_the_archive_line() {
        let sample = chat("c1", "api", "Fees");
        assert_eq!(
            labels(&title_menu_items(&sample, false, false)),
            vec!["Rename", "Pin", "Export report", "-", "Archive", "Delete"]
        );
        assert_eq!(
            labels(&title_menu_items(&sample, true, false)),
            vec![
                "Rename",
                "Pin",
                "Export report",
                "Stop run",
                "-",
                "Archive",
                "Delete"
            ]
        );
    }
}
