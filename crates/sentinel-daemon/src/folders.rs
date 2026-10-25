//! Folder browsing for a phone, repository checks, and the starter backlog.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use sentinel_core::api::{CheckRow, CheckStatus, FolderEntry, RepoCheckResponse};

pub use crate::folders_tree::{TREE_FOLDER_LIMIT, repo_tree};

pub(crate) const STARTER_BACKLOG: &str = "# Backlog

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
    #[error("path is outside the repository")]
    OutsideRepo,
    #[error("not a folder in this repository: {0}")]
    NotFolder(String),
    #[error("file already exists: {0}")]
    Exists(PathBuf),
    #[error("cannot use folder: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, FolderError>;

/// The folders below `path`. An empty path, `~`, and `~/name` mean the home folder and below it.
pub fn list_folders(home: &Path, path: &Path) -> Result<Vec<FolderEntry>> {
    let home = fs::canonicalize(home)?;
    let folder = resolve_inside(&home, &expand_home(&home, path))?;
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

/// The browser sends `~` for the home folder. Only the `~` at the start means home.
fn expand_home(home: &Path, path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if text.is_empty() || text == "~" {
        return home.to_path_buf();
    }
    match text.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => path.to_path_buf(),
    }
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
/// Keep Sentinel's own folder out of the repository's `git status` without touching any tracked
/// file: add it to `.git/info/exclude`, which is local to this clone.
pub fn exclude_sentinel_folder(repo: &Path) {
    let exclude = repo.join(".git/info/exclude");
    let known = fs::read_to_string(&exclude).unwrap_or_default();
    if known.lines().any(|line| line.trim() == "/.sentinel/") {
        return;
    }
    if let Some(parent) = exclude.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut text = known;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("/.sentinel/\n");
    // A repository we cannot write to is reported later, when git itself fails.
    let _ = fs::write(&exclude, text);
}

/// A plain message box, for a program that has no terminal to print to.
pub fn alert(text: &str) {
    rfd::MessageDialog::new()
        .set_title("Sentinel")
        .set_description(text)
        .set_level(rfd::MessageLevel::Warning)
        .show();
}

/// The answer of the folder dialog. `Unavailable` means no dialog could open here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    Chosen(PathBuf),
    Cancelled,
    Unavailable,
}

/// Runs the computer's folder dialog. The dialog opens in `start` when it exists.
#[cfg(target_os = "macos")]
pub fn pick_native(start: Option<&Path>) -> Pick {
    // rfd's dialog does not reliably show when called from the daemon's thread, so macOS asks
    // Finder through osascript instead.
    match Command::new("osascript")
        .arg("-e")
        .arg(dialog_script(start))
        .output()
    {
        Ok(output) => parse_dialog_output(
            output.status.code().unwrap_or(-1),
            &String::from_utf8_lossy(&output.stdout),
            &String::from_utf8_lossy(&output.stderr),
        ),
        Err(_) => Pick::Unavailable,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn pick_native(start: Option<&Path>) -> Pick {
    let mut dialog = rfd::FileDialog::new();
    if let Some(start) = start {
        dialog = dialog.set_directory(start);
    }
    dialog.pick_folder().map_or(Pick::Cancelled, Pick::Chosen)
}

/// The `AppleScript` that shows the folder chooser. The start folder is optional.
#[cfg(target_os = "macos")]
fn dialog_script(start: Option<&Path>) -> String {
    let location = start
        .filter(|path| path.is_dir())
        .map_or_else(String::new, |path| {
            format!(
                " default location (POSIX file \"{}\")",
                escape_applescript(&path.to_string_lossy())
            )
        });
    format!("POSIX path of (choose folder with prompt \"Choose a project folder\"{location})")
}

/// Escapes the text for a double-quoted `AppleScript` string.
#[cfg(any(target_os = "macos", test))]
fn escape_applescript(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Reads what osascript printed. Exit 0 is the chosen folder. Exit 1 with error -128 is a
/// cancel. Anything else means the dialog did not run.
#[cfg(any(target_os = "macos", test))]
fn parse_dialog_output(status: i32, stdout: &str, stderr: &str) -> Pick {
    if status == 0 {
        return chosen_path(stdout).map_or(Pick::Unavailable, Pick::Chosen);
    }
    if status == 1 && stderr.contains("-128") {
        return Pick::Cancelled;
    }
    Pick::Unavailable
}

/// The printed path without its trailing slashes. A root path keeps its single slash.
#[cfg(any(target_os = "macos", test))]
fn chosen_path(stdout: &str) -> Option<PathBuf> {
    let text = stdout.trim();
    if text.is_empty() {
        return None;
    }
    let kept = text.trim_end_matches('/');
    Some(PathBuf::from(if kept.is_empty() { "/" } else { kept }))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{Pick, chosen_path, escape_applescript, list_folders, parse_dialog_output};

    fn repo_with(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for file in files {
            let path = dir.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "x").unwrap();
        }
        dir
    }

    fn names_in(home: &Path, path: &str) -> Vec<String> {
        list_folders(home, Path::new(path))
            .unwrap()
            .into_iter()
            .map(|entry| entry.name)
            .collect()
    }

    #[test]
    fn an_empty_path_and_a_tilde_both_list_the_home_folder() {
        let home = repo_with(&["code/a.txt", "notes/b.txt", "loose.txt"]);
        assert_eq!(names_in(home.path(), ""), vec!["code", "notes"]);
        assert_eq!(names_in(home.path(), "~"), vec!["code", "notes"]);
    }

    #[test]
    fn a_tilde_path_lists_a_folder_below_home() {
        let home = repo_with(&["code/sub/x.txt"]);
        assert_eq!(names_in(home.path(), "~/code"), vec!["sub"]);
    }

    #[test]
    fn escape_applescript_escapes_quotes_and_backslashes() {
        assert_eq!(escape_applescript(r#"a"b\c"#), r#"a\"b\\c"#);
        assert_eq!(escape_applescript("~/code"), "~/code");
    }

    #[test]
    fn exit_zero_with_a_path_is_chosen_without_trailing_slash() {
        let pick = parse_dialog_output(0, "/Users/me/code/app/\n", "");
        assert_eq!(pick, Pick::Chosen(PathBuf::from("/Users/me/code/app")));
    }

    #[test]
    fn a_root_path_keeps_its_slash() {
        assert_eq!(chosen_path("/\n"), Some(PathBuf::from("/")));
    }

    #[test]
    fn exit_zero_with_no_path_is_unavailable() {
        assert_eq!(parse_dialog_output(0, "  \n", ""), Pick::Unavailable);
    }

    #[test]
    fn user_canceled_error_is_cancelled() {
        let stderr = "execution error: User canceled. (-128)";
        assert_eq!(parse_dialog_output(1, "", stderr), Pick::Cancelled);
    }

    #[test]
    fn other_dialog_failures_are_unavailable() {
        assert_eq!(
            parse_dialog_output(1, "", "execution error: Not authorized. (-1743)"),
            Pick::Unavailable
        );
        assert_eq!(parse_dialog_output(-1, "", ""), Pick::Unavailable);
    }
}
