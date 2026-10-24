//! The file tools. Every path is checked against the repository before a byte moves, and a write
//! is accepted only for a path the task reserved. The rules follow `sentinel-workers`' agent
//! tools; this crate does not depend on that one.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::protocol::argument;

pub const TOOL_NAMES: [&str; 4] = ["read_file", "list_dir", "write_file", "request_files"];
const READ_LIMIT_BYTES: usize = 100_000;
const LIST_LIMIT: usize = 200;
/// Folders no tool may read, list, or write, at any depth.
const HIDDEN_DIRS: [&str; 2] = [".git", ".sentinel"];
const REFUSED_PATH: &str = "That path is not allowed.";
/// The daemon owns reservations, so this tool grants nothing and writes nothing.
const REQUEST_ANSWER: &str = "Ask for more files in your report. The reviewer can widen the scope.";

pub struct FileTools {
    workdir: PathBuf,
    scope: Vec<String>,
}

impl FileTools {
    pub fn new(workdir: PathBuf, scope: Vec<String>) -> Self {
        Self { workdir, scope }
    }

    /// Run one file tool. Errors come back as the text the model reads.
    pub fn call(&self, name: &str, args: &Value) -> Result<String, String> {
        match name {
            "read_file" => self.read_file(argument(args, "path")?),
            "list_dir" => self.list_dir(argument(args, "path")?),
            "write_file" => self.write_file(argument(args, "path")?, argument(args, "content")?),
            "request_files" => Ok(REQUEST_ANSWER.to_owned()),
            other => Err(format!("Unknown tool: {other}.")),
        }
    }

    fn read_file(&self, raw: &str) -> Result<String, String> {
        let target = self.read_target(raw)?;
        let bytes = fs::read(&target).map_err(|error| format!("Cannot read the file: {error}."))?;
        let text = String::from_utf8(bytes).map_err(|_| "That file is not text.".to_owned())?;
        Ok(cut_at_limit(&text))
    }

    fn list_dir(&self, raw: &str) -> Result<String, String> {
        let folder = self.read_target(raw)?;
        let entries =
            fs::read_dir(&folder).map_err(|error| format!("Cannot list the folder: {error}."))?;
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry_name(&entry))
            .collect();
        names.sort();
        let total = names.len();
        names.truncate(LIST_LIMIT);
        if total > LIST_LIMIT {
            names.push(format!("[... {} names cut ...]", total - LIST_LIMIT));
        }
        Ok(names.join("\n"))
    }

    fn write_file(&self, raw: &str, content: &str) -> Result<String, String> {
        let relative = relative_path(raw).ok_or_else(|| REFUSED_PATH.to_owned())?;
        if !in_scope(&relative, &self.scope) {
            return Err(not_reserved(raw, &self.scope));
        }
        let target = self.checked_target(&relative)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Cannot create the folder: {error}."))?;
        }
        fs::write(&target, content).map_err(|error| format!("Cannot write the file: {error}."))?;
        Ok(format!("Wrote {raw} ({} bytes).", content.len()))
    }

    fn read_target(&self, raw: &str) -> Result<PathBuf, String> {
        let relative = relative_path(raw).ok_or_else(|| REFUSED_PATH.to_owned())?;
        self.checked_target(&relative)
    }

    /// The absolute path for a relative path. Refused when it names a hidden folder or resolves
    /// outside the repository.
    fn checked_target(&self, relative: &Path) -> Result<PathBuf, String> {
        let target = self.workdir.join(relative);
        if is_hidden(relative) || !resolves_inside(&self.workdir, &target) {
            return Err(REFUSED_PATH.to_owned());
        }
        Ok(target)
    }
}

/// A path from the model as a clean relative path. `..`, a leading `/`, or a drive prefix is
/// refused, so no path can name a place outside the repository by its text.
fn relative_path(raw: &str) -> Option<PathBuf> {
    let mut clean = PathBuf::new();
    for component in Path::new(raw).components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(clean)
}

fn is_hidden(relative: &Path) -> bool {
    relative.components().any(|part| {
        part.as_os_str()
            .to_str()
            .is_some_and(|name| HIDDEN_DIRS.contains(&name))
    })
}

/// True when the path, or its deepest existing part, resolves inside the repository. A symlink at
/// the path itself is refused, even a dangling one, so a write cannot go through a link.
fn resolves_inside(workdir: &Path, target: &Path) -> bool {
    let Ok(root) = workdir.canonicalize() else {
        return false;
    };
    if target
        .symlink_metadata()
        .is_ok_and(|meta| meta.file_type().is_symlink())
    {
        return false;
    }
    let mut probe = target;
    loop {
        if let Ok(resolved) = probe.canonicalize() {
            return resolved.starts_with(&root);
        }
        let Some(parent) = probe.parent() else {
            return false;
        };
        probe = parent;
    }
}

/// True when the path is a scope entry or lies under a scope folder. An empty entry is ignored,
/// so it cannot reserve the whole repository by accident.
fn in_scope(relative: &Path, scope: &[String]) -> bool {
    scope
        .iter()
        .filter_map(|entry| relative_path(entry))
        .filter(|reserved| !reserved.as_os_str().is_empty())
        .any(|reserved| relative.starts_with(reserved))
}

fn not_reserved(raw: &str, scope: &[String]) -> String {
    let allowed = if scope.is_empty() {
        "nothing".to_owned()
    } else {
        scope.join(", ")
    };
    format!("{raw} is not reserved for this task. Write only: {allowed}.")
}

fn cut_at_limit(text: &str) -> String {
    if text.len() <= READ_LIMIT_BYTES {
        return text.to_owned();
    }
    let end = (0..=READ_LIMIT_BYTES)
        .rev()
        .find(|&index| text.is_char_boundary(index))
        .unwrap_or(0);
    format!("{}\n[... file cut at 100 KB ...]", &text[..end])
}

fn entry_name(entry: &fs::DirEntry) -> Option<String> {
    let name = entry.file_name().into_string().ok()?;
    if HIDDEN_DIRS.contains(&name.as_str()) {
        return None;
    }
    let is_folder = entry.file_type().is_ok_and(|kind| kind.is_dir());
    Some(if is_folder { format!("{name}/") } else { name })
}
