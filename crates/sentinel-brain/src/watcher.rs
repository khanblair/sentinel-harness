//! Vault watcher. Reports note files that changed outside Sentinel, after a debounce.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::error::Result;
use crate::vault::{SelfWriteTracker, Vault, is_note_file};

/// Keeps the watch alive. Dropping it stops the watch and its thread.
pub struct VaultWatcher {
    _watcher: RecommendedWatcher,
}

/// Watch the vault folder. `on_change` gets the changed note files once no new event
/// arrived for `debounce`. Files whose content matches a Sentinel write are left out.
/// Pass `Vault::recent_self_writes()` as `self_writes`.
pub fn watch_vault(
    vault_root: &Path,
    self_writes: SelfWriteTracker,
    on_change: impl Fn(Vec<PathBuf>) + Send + 'static,
    debounce: Duration,
) -> Result<VaultWatcher> {
    let root = std::fs::canonicalize(vault_root)?;
    let (sender, events) = mpsc::channel::<Event>();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
        if let Ok(event) = result {
            sender.send(event).ok();
        }
    })
    .map_err(std::io::Error::other)?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(std::io::Error::other)?;
    let filter = ChangeFilter { root, self_writes };
    thread::spawn(move || report_changes(&events, &filter, &on_change, debounce));
    Ok(VaultWatcher { _watcher: watcher })
}

/// Decides which event paths are reported: note files that Sentinel did not write.
struct ChangeFilter {
    root: PathBuf,
    self_writes: SelfWriteTracker,
}

impl ChangeFilter {
    fn changed_notes(&self, mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
        paths.sort();
        paths.dedup();
        paths
            .into_iter()
            .filter(|path| self.is_vault_note(path))
            .filter(|path| !self.is_self_write(path))
            .collect()
    }

    fn is_vault_note(&self, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return false;
        };
        is_note_file(path) && !relative.starts_with("_proposals")
    }

    fn is_self_write(&self, path: &Path) -> bool {
        match std::fs::read_to_string(path) {
            Ok(text) => self.self_writes.contains(path, Vault::content_hash(&text)),
            Err(_) => false,
        }
    }
}

/// Group events into batches. A batch ends after `debounce` with no new event.
fn report_changes(
    events: &Receiver<Event>,
    filter: &ChangeFilter,
    on_change: &impl Fn(Vec<PathBuf>),
    debounce: Duration,
) {
    while let Ok(first) = events.recv() {
        let mut paths = first.paths;
        while let Ok(event) = events.recv_timeout(debounce) {
            paths.extend(event.paths);
        }
        let changed = filter.changed_notes(paths);
        if !changed.is_empty() {
            on_change(changed);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use sentinel_core::{NoteId, NoteType};
    use tempfile::tempdir;

    use super::*;
    use crate::note::{Author, Note};

    fn sample_note() -> Note {
        Note {
            id: NoteId::new("watched-1"),
            note_type: NoteType::Convention,
            title: "Watched".to_string(),
            tags: vec![],
            by: Author::You,
            strength: 0.5,
            confirmed: None,
            pinned: false,
            lesson: None,
            body: "First.".to_string(),
            extra: vec![],
        }
    }

    #[test]
    fn outside_edit_reports_and_sentinel_write_does_not() {
        let dir = tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        let note = sample_note();
        vault.write(&note, None).unwrap();
        let file = vault.path_for(&note);

        let (sender, reports) = mpsc::channel::<Vec<PathBuf>>();
        let _watch = watch_vault(
            dir.path(),
            vault.recent_self_writes(),
            move |paths| {
                sender.send(paths).ok();
            },
            Duration::from_millis(200),
        )
        .unwrap();

        std::fs::write(
            &file,
            "---\nid: watched-1\ntype: convention\ntitle: Edited\n---\nEdited.\n",
        )
        .unwrap();
        let outside = reports.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(outside.iter().any(|p| p.ends_with("watched-1.md")));
        while reports.recv_timeout(Duration::from_millis(500)).is_ok() {}

        let mut edited = note.clone();
        edited.body = "Sentinel wrote this.".to_string();
        vault.write(&edited, None).unwrap();
        assert!(reports.recv_timeout(Duration::from_secs(1)).is_err());
    }
}
