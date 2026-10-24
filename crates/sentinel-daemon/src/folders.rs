//! Folder browsing for a phone, repository checks, and the starter backlog.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use sentinel_core::api::{CheckRow, CheckStatus, FolderEntry, RepoCheckResponse};

const STARTER_BACKLOG: &str = "# Backlog

## T-01 Add a greeting command
Done when the command prints a greeting to the terminal.

## T-02 Cover the greeting with a test
Done when a test runs the command and checks the printed text.

## T-03 Document how to run the project
Done when the README explains the setup and the run command.
";

#[derive(Debug, thiserror::Error)]
pub enum FolderError {
    #[error("path is outside the home folder")]
    OutsideHome,
    #[error("path may not contain ..")]
    ParentTraversal,
    #[error("file already exists: {0}")]
    Exists(PathBuf),
    #[error("cannot use folder: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, FolderError>;

pub fn list_folders(home: &Path, path: &Path) -> Result<Vec<FolderEntry>> {
    let home = fs::canonicalize(home)?;
    let folder = resolve_inside(&home, path)?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(&folder)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(resolved) = fs::canonicalize(entry.path()) else {
            // A broken link is not a folder the user can open.
            continue;
        };
        if resolved.is_dir() && resolved.starts_with(&home) {
            entries.push(FolderEntry {
                name,
                path: resolved.to_string_lossy().into_owned(),
            });
        }
    }
    entries.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(entries)
}

pub fn within_home(home: &Path, path: &Path) -> bool {
    if has_parent_dir(path) {
        return false;
    }
    let Ok(home) = fs::canonicalize(home) else {
        return false;
    };
    let Ok(path) = fs::canonicalize(path) else {
        return false;
    };
    path.starts_with(home)
}

fn resolve_inside(home: &Path, path: &Path) -> Result<PathBuf> {
    if has_parent_dir(path) {
        return Err(FolderError::ParentTraversal);
    }
    let folder = fs::canonicalize(path)?;
    if !folder.starts_with(home) {
        return Err(FolderError::OutsideHome);
    }
    Ok(folder)
}

fn has_parent_dir(path: &Path) -> bool {
    path.components()
        .any(|component| component == Component::ParentDir)
}

pub fn check_repo(path: &Path) -> RepoCheckResponse {
    let shown = path.to_string_lossy().into_owned();
    if !path.is_dir() {
        return RepoCheckResponse {
            path: shown,
            rows: vec![CheckRow {
                name: "Folder".to_owned(),
                status: CheckStatus::Invalid,
                detail: "Not a folder".to_owned(),
                fix: None,
            }],
            usable: false,
        };
    }
    let git = git_row(path);
    let backlog = backlog_row(path);
    let clean = clean_row(path, git.status == CheckStatus::Ok);
    let usable = [git.status, backlog.status, clean.status]
        .iter()
        .all(|status| *status == CheckStatus::Ok);
    RepoCheckResponse {
        path: shown,
        rows: vec![git, backlog, clean],
        usable,
    }
}

fn git_row(path: &Path) -> CheckRow {
    if path.join(".git").exists() {
        CheckRow {
            name: "Git repository".to_owned(),
            status: CheckStatus::Ok,
            detail: "This folder is a git repository".to_owned(),
            fix: None,
        }
    } else {
        CheckRow {
            name: "Git repository".to_owned(),
            status: CheckStatus::NeedsAction,
            detail: "No .git entry in this folder".to_owned(),
            fix: None,
        }
    }
}

fn backlog_row(path: &Path) -> CheckRow {
    if path.join("backlog.md").is_file() {
        CheckRow {
            name: "backlog.md".to_owned(),
            status: CheckStatus::Ok,
            detail: "backlog.md is present".to_owned(),
            fix: None,
        }
    } else {
        CheckRow {
            name: "backlog.md".to_owned(),
            status: CheckStatus::NeedsAction,
            detail: "No backlog.md in this folder".to_owned(),
            fix: Some("Create a starter backlog".to_owned()),
        }
    }
}

fn clean_row(path: &Path, has_git: bool) -> CheckRow {
    if !has_git {
        return CheckRow {
            name: "Clean working tree".to_owned(),
            status: CheckStatus::NeedsAction,
            detail: "Skipped because this folder is not a git repository".to_owned(),
            fix: None,
        };
    }
    match git_status(path) {
        Ok(true) => CheckRow {
            name: "Clean working tree".to_owned(),
            status: CheckStatus::Ok,
            detail: "No uncommitted changes".to_owned(),
            fix: None,
        },
        Ok(false) => CheckRow {
            name: "Clean working tree".to_owned(),
            status: CheckStatus::NeedsAction,
            detail: "Uncommitted changes in this folder".to_owned(),
            fix: None,
        },
        Err(detail) => CheckRow {
            name: "Clean working tree".to_owned(),
            status: CheckStatus::NeedsAction,
            detail,
            fix: None,
        },
    }
}

fn git_status(path: &Path) -> std::result::Result<bool, String> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output()
        .map_err(|error| format!("Cannot run git status: {error}"))?;
    if !output.status.success() {
        return Err("git status failed".to_owned());
    }
    Ok(output.stdout.iter().all(u8::is_ascii_whitespace))
}

pub fn create_starter_backlog(path: &Path) -> Result<()> {
    let target = path.join("backlog.md");
    if target.exists() {
        return Err(FolderError::Exists(target));
    }
    fs::write(target, STARTER_BACKLOG)?;
    Ok(())
}

/// Blocking native dialog. The caller runs this inside `spawn_blocking`.
pub fn pick_native(start: Option<&Path>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(start) = start {
        dialog = dialog.set_directory(start);
    }
    dialog.pick_folder()
}
