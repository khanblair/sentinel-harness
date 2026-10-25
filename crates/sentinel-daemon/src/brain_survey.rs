//! The survey: a walk of the repository root that lists its files, its area folders, and its
//! top-level layout. The seed reads it, and it runs no agent.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::brain::slug;

const MAX_FILES: usize = 5_000;
const MAX_AREAS: usize = 12;
const MAX_LAYOUT: usize = 40;
/// Hidden folders are skipped by their dot, so `.git` and `.sentinel` need no entry here.
const SKIPPED_DIRS: [&str; 4] = ["target", "node_modules", "dist", "build"];

/// Every file under the root, as paths relative to it, sorted. Stops at `MAX_FILES`.
pub(crate) fn survey(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![PathBuf::new()];
    while files.len() < MAX_FILES {
        let Some(dir) = pending.pop() else { break };
        for entry in sorted_entries(&root.join(&dir)) {
            visit_entry(&entry, &dir, &mut files, &mut pending);
        }
    }
    files.sort();
    files.truncate(MAX_FILES);
    files
}

/// Sorts one entry of `dir`: a folder waits in `pending`, a file joins `files`. Hidden and
/// skipped folders are dropped here.
fn visit_entry(
    entry: &fs::DirEntry,
    dir: &Path,
    files: &mut Vec<PathBuf>,
    pending: &mut Vec<PathBuf>,
) {
    let name = entry.file_name().to_string_lossy().into_owned();
    if name.starts_with('.') {
        return;
    }
    let relative = dir.join(&name);
    match entry.file_type() {
        Ok(kind) if kind.is_dir() && !SKIPPED_DIRS.contains(&name.as_str()) => {
            pending.push(relative);
        }
        Ok(kind) if kind.is_file() => files.push(relative),
        _ => {}
    }
}

fn sorted_entries(dir: &Path) -> Vec<fs::DirEntry> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<fs::DirEntry> = read.flatten().collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    entries
}

fn is_dir_entry(entry: &fs::DirEntry) -> bool {
    entry.file_type().is_ok_and(|kind| kind.is_dir())
}

/// Top-level folders that get an area note: no hidden or skipped ones, one per slug, at most
/// `MAX_AREAS`.
pub(crate) fn area_folders(root: &Path) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut folders = Vec::new();
    for entry in sorted_entries(root)
        .iter()
        .filter(|entry| is_dir_entry(entry))
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let stem = slug(&name);
        if name.starts_with('.') || SKIPPED_DIRS.contains(&name.as_str()) || stem.is_empty() {
            continue;
        }
        if seen.insert(stem) {
            folders.push(name);
        }
        if folders.len() == MAX_AREAS {
            break;
        }
    }
    folders
}

/// Top-level entries shown in the project note. Folders end with a slash.
pub(crate) fn layout(root: &Path) -> Vec<String> {
    sorted_entries(root)
        .iter()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = is_dir_entry(entry);
            let skipped =
                name.starts_with('.') || (is_dir && SKIPPED_DIRS.contains(&name.as_str()));
            (!skipped).then(|| if is_dir { format!("{name}/") } else { name })
        })
        .take(MAX_LAYOUT)
        .collect()
}
