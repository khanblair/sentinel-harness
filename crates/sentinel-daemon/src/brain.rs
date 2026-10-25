//! The Brain in the daemon. The vault and the index open on first use. A watcher keeps the index in
//! step with edits made in Obsidian or an editor. The other `brain_*` modules add the calls.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sentinel_brain::note::{Note, parse_note, render_note};
use sentinel_brain::schedule::Date;
use sentinel_brain::{BrainError, Index, Vault, VaultWatcher, watch_vault};
use sentinel_core::{Event, NoteId};
use time::OffsetDateTime;

use crate::controls::Caller;
use crate::engine::{Engine, EngineError, Result, lock};

/// Edits that arrive this close together are read as one change.
const DEBOUNCE: Duration = Duration::from_millis(200);

/// The open Brain: the note files, the index over them, and the watcher that keeps the index
/// current. Dropping it stops the watcher.
pub struct BrainState {
    pub(crate) vault: Vault,
    pub(crate) index: Mutex<Index>,
    watcher: Mutex<Option<VaultWatcher>>,
    /// Held while the seed writes, so two seeds never write the same notes at once.
    pub(crate) seed_lock: Mutex<()>,
}

impl BrainState {
    fn open(repo: &Path) -> Result<BrainState> {
        let vault = Vault::open(repo.join(".sentinel/brain"))?;
        let mut index = Index::open(&repo.join(".sentinel/brain.db"))?;
        index.rebuild(&vault)?;
        Ok(BrainState {
            vault,
            index: Mutex::new(index),
            watcher: Mutex::new(None),
            seed_lock: Mutex::new(()),
        })
    }
}

impl Engine {
    /// The Brain, opened on first use: the vault, then the index rebuilt from the vault files.
    pub fn brain(&self) -> Result<Arc<BrainState>> {
        let (state, first_open) = self.open_brain()?;
        if first_open {
            self.seed_when_empty(&state);
        }
        Ok(state)
    }

    /// The open Brain, and whether this call opened it. The seed uses this so its own open does
    /// not start a second seed.
    pub(crate) fn open_brain(&self) -> Result<(Arc<BrainState>, bool)> {
        // The slot stays locked while the Brain opens. Two requests that arrive together must not
        // open it twice: the second open rebuilds the index file and can erase what the seed of
        // the first one has just written.
        let mut slot = lock(&self.inner.brain);
        if let Some(state) = slot.clone() {
            return Ok((state, false));
        }
        let opened = Arc::new(BrainState::open(&self.repo_path())?);
        *slot = Some(Arc::clone(&opened));
        drop(slot);
        self.start_brain_watcher();
        Ok((opened, true))
    }

    /// Watch the vault so an edit made outside Sentinel reaches the index. Starts once. A failure
    /// is logged and does not stop the daemon.
    pub fn start_brain_watcher(&self) {
        let Some(state) = lock(&self.inner.brain).clone() else {
            return;
        };
        if !state.vault.root().is_dir() {
            return;
        }
        let mut slot = lock(&state.watcher);
        if slot.is_some() {
            return;
        }
        let weak = Arc::downgrade(&state);
        let started = watch_vault(
            state.vault.root(),
            state.vault.recent_self_writes(),
            move |paths| {
                if let Some(state) = weak.upgrade() {
                    reindex_paths(&state, paths);
                }
            },
            DEBOUNCE,
        );
        match started {
            Ok(watcher) => *slot = Some(watcher),
            Err(error) => eprintln!("sentinel: the Brain watcher did not start: {error}"),
        }
    }

    /// Rebuild the index from the vault files. Returns the note count.
    pub fn rebuild_brain_index(&self) -> Result<usize> {
        let state = self.brain()?;
        let mut index = lock(&state.index);
        Ok(index.rebuild(&state.vault)?)
    }
}

/// Re-read each changed note file into the index. A file that is gone, or does not parse, leaves
/// the index.
fn reindex_paths(state: &BrainState, paths: Vec<PathBuf>) {
    let mut index = lock(&state.index);
    for path in paths {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let parsed = fs::read_to_string(&path)
            .ok()
            .and_then(|text| parse_note(&text, &path.to_string_lossy()).ok());
        let result = match parsed {
            Some(note) => index.upsert(&note),
            None => index.remove(&NoteId::new(stem)),
        };
        if let Err(error) = result {
            eprintln!("sentinel: could not reindex {stem}: {error}");
        }
    }
}

/// Write a note to the vault and put it in the index. `expected` is the hash of the file text that
/// the caller read, so a write over a newer file is refused.
pub(crate) fn write_note(state: &BrainState, note: &Note, expected: Option<u64>) -> Result<()> {
    state.vault.write(note, expected)?;
    lock(&state.index).upsert(note)?;
    Ok(())
}

/// Tell the stream that a note changed. The write already happened, so a failed event is only logged.
pub(crate) fn note_written(engine: &Engine, id: &NoteId) {
    engine.emit_quiet(&Event::NoteWritten { note: id.clone() });
}

/// The text of a note file as it is on disk. The version of a note is the hash of this text.
pub(crate) fn note_text(vault: &Vault, note: &Note) -> String {
    fs::read_to_string(vault.path_for(note)).unwrap_or_else(|_| render_note(note))
}

pub(crate) fn version_of(text: &str) -> String {
    format!("{:x}", Vault::content_hash(text))
}

/// Refuses a phone. Every call a phone may not make starts with this.
pub(crate) fn require_pc(caller: &Caller) -> Result<()> {
    if caller.from_pc {
        Ok(())
    } else {
        Err(EngineError::Refused(
            "This needs the computer. Do it there.".to_owned(),
        ))
    }
}

/// Today's date in UTC.
pub(crate) fn today() -> Date {
    let now = OffsetDateTime::now_utc();
    Date {
        year: now.year(),
        month: u8::from(now.month()),
        day: now.day(),
    }
}

/// Parses `YYYY-MM-DD`. Any other text gives `None`.
pub(crate) fn parse_date(text: &str) -> Option<Date> {
    let mut parts = text.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;
    Date::new(year, month, day)
}

/// A note id from a title: lowercase ASCII words joined by dashes.
pub(crate) fn slug(title: &str) -> String {
    title
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("-")
}

impl From<BrainError> for EngineError {
    fn from(error: BrainError) -> Self {
        match error {
            BrainError::Format { reason, .. } => EngineError::Invalid(reason),
            BrainError::NotFound(what) => EngineError::NotFound(what),
            BrainError::Conflict(message) => EngineError::Conflict(message),
            BrainError::Io(error) => EngineError::Io(error),
            BrainError::Sqlite(error) => EngineError::Io(std::io::Error::other(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_becomes_a_lowercase_dashed_id() {
        assert_eq!(slug("Use Tabs, not spaces!"), "use-tabs-not-spaces");
        assert_eq!(slug("  --  "), "");
    }

    #[test]
    fn a_date_parses_only_when_it_is_a_real_day() {
        assert_eq!(
            parse_date("2026-02-28"),
            Date::new(2026, 2, 28),
            "a valid day parses"
        );
        assert_eq!(parse_date("2026-02-30"), None);
        assert_eq!(parse_date("yesterday"), None);
    }

    #[test]
    fn a_format_error_becomes_invalid_with_its_reason() {
        let error = BrainError::Format {
            path: "a.md".to_owned(),
            reason: "missing front matter".to_owned(),
        };
        let mapped = EngineError::from(error);
        assert!(matches!(mapped, EngineError::Invalid(ref text) if text == "missing front matter"));
    }

    #[test]
    fn a_conflict_stays_a_conflict() {
        let mapped = EngineError::from(BrainError::Conflict("changed".to_owned()));
        assert!(matches!(mapped, EngineError::Conflict(ref text) if text == "changed"));
    }
}
