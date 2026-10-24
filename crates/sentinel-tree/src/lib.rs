//! Tree guard plumbing: private-index checkpoints, file restore, and test tamper checks.
//!
//! Every git call runs against a private index file, so the user's index is never written.

use std::path::{Component, Path, PathBuf};

use thiserror::Error;

pub mod checkpoint;
pub mod git;
pub mod guard_config;
pub mod restore;
pub mod revert;
pub mod tamper;
pub mod watcher;

pub use checkpoint::{Checkpoint, checkpoint};
pub use git::Git;
pub use guard_config::{GuardFiles, remove_guard_files, write_guard_files};
pub use restore::{RestoreOutcome, restore_path};
pub use revert::revert_checkpoint;
pub use tamper::{edited_tests, looks_like_test};
pub use watcher::{GuardHandle, GuardOptions, WritePolicy, WriteVerdict, start_guard};

#[derive(Debug, Error)]
pub enum TreeError {
    #[error("git {args} failed: {stderr}")]
    Git { args: String, stderr: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("not a git repository: {}", .0.display())]
    NotARepo(PathBuf),
    #[error("invalid path: {0}")]
    InvalidPath(String),
}

pub type Result<T> = std::result::Result<T, TreeError>;

/// Validate a repo-relative path and return it in git's form, with `.` parts removed.
///
/// Rejecting traversal here keeps every later join inside the repo root.
pub(crate) fn repo_path(path: &str) -> Result<String> {
    let invalid = || TreeError::InvalidPath(path.to_owned());
    if path.is_empty() {
        return Err(invalid());
    }
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Err(invalid());
    }
    let mut parts = Vec::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(invalid());
            }
        }
    }
    if parts.is_empty() {
        return Err(invalid());
    }
    Ok(parts.join("/"))
}
