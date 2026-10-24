//! The vault: one markdown file per note, read and written atomically.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use sentinel_core::NoteId;

use crate::error::{BrainError, Result};
use crate::note::{NOTE_TYPES, Note, check_note, note_type_dir, parse_note, render_note};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Hashes of files Sentinel wrote, keyed by path. The watcher skips these edits.
#[derive(Debug, Clone, Default)]
pub struct SelfWriteTracker {
    inner: Arc<Mutex<HashMap<PathBuf, u64>>>,
}

impl SelfWriteTracker {
    pub fn record(&self, path: PathBuf, hash: u64) {
        self.entries().insert(path, hash);
    }

    /// True when the file at `path` was last written by Sentinel with this hash.
    pub fn contains(&self, path: &Path, hash: u64) -> bool {
        self.entries().get(path) == Some(&hash)
    }

    fn entries(&self) -> MutexGuard<'_, HashMap<PathBuf, u64>> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The Brain vault: a folder of markdown notes, grouped by type.
pub struct Vault {
    root: PathBuf,
    self_writes: SelfWriteTracker,
}

impl Vault {
    /// Open the vault at `root`, creating the folders that are missing.
    pub fn open(root: impl Into<PathBuf>) -> Result<Vault> {
        let root = root.into();
        std::fs::create_dir_all(root.join("_proposals"))?;
        for t in NOTE_TYPES {
            std::fs::create_dir_all(root.join(note_type_dir(t)))?;
        }
        // The watcher reports canonical paths; the self-write set must use the same ones.
        let root = std::fs::canonicalize(root)?;
        Ok(Vault {
            root,
            self_writes: SelfWriteTracker::default(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The file a note is written to: `<root>/<type folder>/<id>.md`.
    pub fn path_for(&self, note: &Note) -> PathBuf {
        self.root
            .join(note_type_dir(note.note_type))
            .join(format!("{}.md", note.id.as_str()))
    }

    /// Read a note by id. Finds the note in any type folder.
    pub fn read(&self, id: &NoteId) -> Result<Option<Note>> {
        match self.find(id)? {
            Some((path, text)) => parse_note(&text, &path.to_string_lossy()).map(Some),
            None => Ok(None),
        }
    }

    /// Every note that parses, sorted by id. Broken files are left out.
    pub fn list(&self) -> Result<Vec<Note>> {
        let mut notes: Vec<Note> = self
            .parse_all()?
            .into_iter()
            .filter_map(|(_, parsed)| parsed.ok())
            .collect();
        notes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(notes)
    }

    /// Files that do not parse, with the reason.
    pub fn list_errors(&self) -> Result<Vec<(PathBuf, String)>> {
        Ok(self
            .parse_all()?
            .into_iter()
            .filter_map(|(path, parsed)| parsed.err().map(|e| (path, e.to_string())))
            .collect())
    }

    /// Write a note with an atomic replace. Returns the hash of the new file text.
    ///
    /// With `expected_hash`, fails with `Conflict` when the file on disk no longer has that hash.
    pub fn write(&self, note: &Note, expected_hash: Option<u64>) -> Result<u64> {
        check_id(note.id.as_str())?;
        check_note(note)?;
        let current = self.find(&note.id)?;
        check_expected(
            expected_hash,
            current.as_ref().map(|(_, text)| text.as_str()),
            &note.id,
        )?;

        let rendered = render_note(note);
        let hash = Self::content_hash(&rendered);
        let target = self.path_for(note);
        write_atomic(&target, &rendered)?;
        if let Some((old, _)) = current
            && old != target
        {
            std::fs::remove_file(&old)?;
        }
        self.self_writes.record(target, hash);
        Ok(hash)
    }

    /// FNV-1a 64-bit hash of text.
    pub fn content_hash(text: &str) -> u64 {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        text.bytes().fold(OFFSET, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(PRIME)
        })
    }

    /// Delete a note's file. Returns false when no such note exists.
    pub fn delete(&self, id: &NoteId) -> Result<bool> {
        match self.find(id)? {
            Some((path, _)) => {
                std::fs::remove_file(path)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Handle to the self-write set, for the watcher.
    pub fn recent_self_writes(&self) -> SelfWriteTracker {
        self.self_writes.clone()
    }

    /// The file holding note `id` and its text. A file named `<id>.md` wins; otherwise
    /// every note file is parsed to find the id, so a note in the wrong folder is found.
    fn find(&self, id: &NoteId) -> Result<Option<(PathBuf, String)>> {
        let files = self.note_files()?;
        if let Some(path) = files.iter().find(|p| file_stem(p) == id.as_str()) {
            return Ok(Some((path.clone(), std::fs::read_to_string(path)?)));
        }
        for path in files {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if parse_note(&text, &path.to_string_lossy()).is_ok_and(|note| note.id == *id) {
                return Ok(Some((path, text)));
            }
        }
        Ok(None)
    }

    fn note_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        for t in NOTE_TYPES {
            let dir = self.root.join(note_type_dir(t));
            if !dir.is_dir() {
                continue;
            }
            for entry in std::fs::read_dir(&dir)? {
                let path = entry?.path();
                if is_note_file(&path) {
                    files.push(path);
                }
            }
        }
        files.sort();
        Ok(files)
    }

    fn parse_all(&self) -> Result<Vec<(PathBuf, Result<Note>)>> {
        Ok(self
            .note_files()?
            .into_iter()
            .map(|path| {
                let parsed = std::fs::read_to_string(&path)
                    .map_err(BrainError::from)
                    .and_then(|text| parse_note(&text, &path.to_string_lossy()));
                (path, parsed)
            })
            .collect())
    }
}

/// True for a note file: a `.md` file whose name does not start with a dot.
pub(crate) fn is_note_file(path: &Path) -> bool {
    let is_md = path.extension().and_then(|e| e.to_str()) == Some("md");
    let hidden = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_none_or(|n| n.starts_with('.'));
    is_md && !hidden
}

fn file_stem(path: &Path) -> &str {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
}

fn check_id(id: &str) -> Result<()> {
    let bad = id.is_empty() || id.starts_with('.') || id.contains(['/', '\\']);
    if bad {
        return Err(BrainError::Format {
            path: id.to_string(),
            reason: "note id is not a safe file name".to_string(),
        });
    }
    Ok(())
}

fn check_expected(expected: Option<u64>, current: Option<&str>, id: &NoteId) -> Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    match current {
        Some(text) if Vault::content_hash(text) == expected => Ok(()),
        _ => Err(BrainError::Conflict(format!(
            "note {id} changed since it was read"
        ))),
    }
}

fn write_atomic(target: &Path, text: &str) -> Result<()> {
    let Some(parent) = target.parent() else {
        return Err(std::io::Error::other("note path has no folder").into());
    };
    std::fs::create_dir_all(parent)?;
    let count = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = parent.join(format!(".{}.{count}.tmp", std::process::id()));
    std::fs::write(&temp, text)?;
    if let Err(error) = std::fs::rename(&temp, target) {
        let _ = std::fs::remove_file(&temp);
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_is_stable_and_sensitive() {
        assert_eq!(Vault::content_hash("abc"), Vault::content_hash("abc"));
        assert_ne!(Vault::content_hash("abc"), Vault::content_hash("abd"));
    }

    #[test]
    fn note_files_skip_hidden_and_non_markdown() {
        assert!(is_note_file(Path::new("conventions/a.md")));
        assert!(!is_note_file(Path::new("conventions/.a.1.tmp")));
        assert!(!is_note_file(Path::new("conventions/a.txt")));
    }
}
