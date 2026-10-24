//! Write guard files for agents that support hooks or read an MCP file server.
//!
//! The file names the paths a task may write, so the agent can stay inside its scope.
//! The guard in `watcher` is the backstop when the agent ignores the file.

use std::fs;
use std::path::{Path, PathBuf};

use sentinel_core::{Scope, TaskId};

use crate::{Result, TreeError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardFiles {
    pub allowed_json: PathBuf,
}

/// Write `<dir>/<task id>.allowed.json` for one task. The write is atomic.
pub fn write_guard_files(dir: &Path, task: &TaskId, scope: &Scope) -> Result<GuardFiles> {
    let id = checked_id(task)?;
    fs::create_dir_all(dir)?;
    let allowed_json = dir.join(format!("{id}.allowed.json"));
    let allowed: Vec<&str> = scope.entries().collect();
    let payload = serde_json::json!({ "task": id, "allowed": allowed });
    let body = serde_json::to_string_pretty(&payload).map_err(json_error)?;
    let temp = dir.join(format!(".{id}.allowed.json.tmp"));
    fs::write(&temp, body)?;
    fs::rename(&temp, &allowed_json)?;
    Ok(GuardFiles { allowed_json })
}

/// Remove a task's guard file. A missing file is not an error.
pub fn remove_guard_files(dir: &Path, task: &TaskId) -> Result<()> {
    let id = checked_id(task)?;
    let target = dir.join(format!("{id}.allowed.json"));
    match fs::remove_file(target) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(TreeError::Io(error)),
    }
}

/// A task id becomes part of a file name, so it must not add a path segment.
fn checked_id(task: &TaskId) -> Result<&str> {
    let id = task.as_str();
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err(TreeError::InvalidPath(id.to_owned()));
    }
    Ok(id)
}

fn json_error(error: serde_json::Error) -> TreeError {
    TreeError::Io(std::io::Error::other(error))
}
