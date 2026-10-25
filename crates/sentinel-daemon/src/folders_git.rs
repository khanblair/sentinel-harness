//! The git half of the repository check: the clean working tree row, and the commit that takes
//! Sentinel's own backlog file out of the way. `folders.rs` runs the row; the engine runs the commit.

use std::path::Path;
use std::process::Command;

use sentinel_core::api::{CheckRow, CheckStatus};

const CLEAN_ROW: &str = "Clean working tree";
const BACKLOG: &str = "backlog.md";
const COMMIT_FIX: &str = "Commit backlog.md";
const COMMIT_MESSAGE: &str = "Add the Sentinel backlog";
/// Sentinel's own state. The check ignores it, the same way git ignores it in this clone.
const SENTINEL_FOLDER: &str = ".sentinel";
/// The most file names one row lists.
const NAMES_SHOWN: usize = 3;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CommitError {
    #[error("Other files have changes too. Commit or stash them yourself, then check again.")]
    OtherChanges,
    #[error("backlog.md has no changes to commit.")]
    NothingToCommit,
    #[error(
        "Git has no name or email set. Run git config user.name and git config user.email, then try again."
    )]
    NoIdentity,
    #[error("{0}")]
    Git(String),
}

/// The clean working tree row. Sentinel's own folder does not count. When the only change is
/// the starter backlog, the row offers the commit button.
pub fn clean_row(path: &Path, has_git: bool) -> CheckRow {
    if !has_git {
        return row(
            CheckStatus::NeedsAction,
            "Skipped because this folder is not a git repository",
            None,
        );
    }
    match git_status(path) {
        Ok(porcelain) => clean_row_for(&porcelain),
        Err(detail) => row(CheckStatus::NeedsAction, &detail, None),
    }
}

fn row(status: CheckStatus, detail: &str, fix: Option<String>) -> CheckRow {
    CheckRow {
        name: CLEAN_ROW.to_owned(),
        status,
        detail: detail.to_owned(),
        fix,
    }
}

fn clean_row_for(porcelain: &str) -> CheckRow {
    let paths = changed_paths(porcelain);
    if paths.is_empty() {
        return row(CheckStatus::Ok, "No uncommitted changes", None);
    }
    let (names, more) = dirty_summary(porcelain);
    let fix = only_backlog_changed(&paths).then(|| COMMIT_FIX.to_owned());
    let detail = format!("Uncommitted changes: {}", list_text(&names, more));
    row(CheckStatus::NeedsAction, &detail, fix)
}

/// The names to show in the row, and how many changed paths are left after them.
pub fn dirty_summary(porcelain: &str) -> (Vec<String>, usize) {
    let paths = changed_paths(porcelain);
    let shown = paths.iter().take(NAMES_SHOWN).cloned().collect();
    (shown, paths.len().saturating_sub(NAMES_SHOWN))
}

fn list_text(names: &[String], more: usize) -> String {
    let joined = names.join(", ");
    match more {
        0 => joined,
        _ => format!("{joined} and {more} more"),
    }
}

/// True when the backlog file is the one and only changed path.
pub fn only_backlog_changed(paths: &[String]) -> bool {
    matches!(paths, [only] if only == BACKLOG)
}

/// The changed paths of `git status --porcelain`, without Sentinel's own folder.
fn changed_paths(porcelain: &str) -> Vec<String> {
    porcelain
        .lines()
        .filter_map(porcelain_path)
        .filter(|path| !is_sentinel_folder(path))
        .collect()
}

/// A porcelain line is a two-letter status, a space, then the path. A rename reads `old -> new`,
/// and the new path is the one that is changed now.
fn porcelain_path(line: &str) -> Option<String> {
    let text = line.get(3..)?.trim();
    let path = text.rsplit(" -> ").next()?.trim_matches('"');
    (!path.is_empty()).then(|| path.to_owned())
}

fn is_sentinel_folder(path: &str) -> bool {
    path == SENTINEL_FOLDER || path.starts_with(&format!("{SENTINEL_FOLDER}/"))
}

/// The `git status --porcelain` output of the folder.
fn git_status(path: &Path) -> Result<String, String> {
    run_git(path, &["status", "--porcelain"])
}

/// Runs git in the folder. The error is git's own message, or a plain word when it has none.
fn run_git(path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .map_err(|error| format!("Cannot run git: {error}"))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(if stderr.is_empty() {
        "git failed".to_owned()
    } else {
        stderr
    })
}

/// Commits `backlog.md` alone. It refuses unless the file is the only change in the folder and
/// git knows the author. Nothing is staged before those checks pass.
pub fn commit_backlog(path: &Path) -> Result<(), CommitError> {
    let porcelain = git_status(path).map_err(CommitError::Git)?;
    let paths = changed_paths(&porcelain);
    if paths.is_empty() {
        return Err(CommitError::NothingToCommit);
    }
    if !only_backlog_changed(&paths) {
        return Err(CommitError::OtherChanges);
    }
    let name = config_value(path, "user.name");
    let email = config_value(path, "user.email");
    if !identity_ready(&name, &email) {
        return Err(CommitError::NoIdentity);
    }
    run_git(path, &["add", "--", BACKLOG]).map_err(CommitError::Git)?;
    run_git(path, &["commit", "-m", COMMIT_MESSAGE, "--", BACKLOG]).map_err(CommitError::Git)?;
    Ok(())
}

/// The value of a git setting in this folder. An unset setting reads as empty.
fn config_value(path: &Path, key: &str) -> String {
    run_git(path, &["config", "--get", key])
        .map(|value| value.trim().to_owned())
        .unwrap_or_default()
}

/// Git needs both an author name and an email before it can commit.
fn identity_ready(name: &str, email: &str) -> bool {
    !name.trim().is_empty() && !email.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use super::*;

    fn git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?} failed");
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// A repository with one commit, so HEAD exists and a second commit can follow it.
    fn repo_with_commit() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        git(dir.path(), &["config", "user.name", "Test Author"]);
        git(dir.path(), &["config", "user.email", "author@example.test"]);
        git(dir.path(), &["config", "commit.gpgsign", "false"]);
        fs::write(dir.path().join("README.md"), "hello\n").unwrap();
        git(dir.path(), &["add", "--", "README.md"]);
        git(dir.path(), &["commit", "-q", "-m", "First"]);
        dir
    }

    fn commit_count(dir: &Path) -> String {
        git(dir, &["rev-list", "--count", "HEAD"]).trim().to_owned()
    }

    #[test]
    fn committing_only_the_backlog_leaves_a_clean_tree() {
        let dir = repo_with_commit();
        fs::write(dir.path().join("backlog.md"), "# Backlog\n").unwrap();
        assert_eq!(commit_backlog(dir.path()), Ok(()));
        assert_eq!(commit_count(dir.path()), "2");
        assert_eq!(git(dir.path(), &["status", "--porcelain"]), "");
    }

    #[test]
    fn another_changed_file_refuses_the_commit_and_changes_nothing() {
        let dir = repo_with_commit();
        fs::write(dir.path().join("backlog.md"), "# Backlog\n").unwrap();
        fs::write(dir.path().join("notes.txt"), "mine\n").unwrap();
        assert_eq!(commit_backlog(dir.path()), Err(CommitError::OtherChanges));
        assert_eq!(commit_count(dir.path()), "1");
        assert_eq!(git(dir.path(), &["diff", "--cached", "--name-only"]), "");
        assert_eq!(
            git(dir.path(), &["status", "--porcelain"]),
            "?? backlog.md\n?? notes.txt\n"
        );
    }

    #[test]
    fn a_clean_tree_has_nothing_to_commit() {
        let dir = repo_with_commit();
        assert_eq!(
            commit_backlog(dir.path()),
            Err(CommitError::NothingToCommit)
        );
        assert_eq!(commit_count(dir.path()), "1");
    }

    #[test]
    fn a_missing_name_or_email_is_not_ready() {
        assert!(identity_ready("Blair", "dev@example.test"));
        assert!(!identity_ready("", "dev@example.test"));
        assert!(!identity_ready("Blair", "   "));
    }

    #[test]
    fn the_summary_names_at_most_three_files_and_counts_the_rest() {
        assert_eq!(dirty_summary(""), (vec![], 0));
        assert_eq!(dirty_summary("?? a.md\n"), (vec!["a.md".to_owned()], 0));
        let three = "?? a.md\n?? b.md\n?? c.md\n";
        assert_eq!(dirty_summary(three).1, 0);
        let four = "?? a.md\n?? b.md\n?? c.md\n?? d.md\n";
        assert_eq!(
            dirty_summary(four),
            (
                vec!["a.md".to_owned(), "b.md".to_owned(), "c.md".to_owned()],
                1
            )
        );
    }

    #[test]
    fn sentinel_folder_changes_do_not_count() {
        assert_eq!(clean_row_for("?? .sentinel/\n").status, CheckStatus::Ok);
        assert_eq!(
            clean_row_for("?? .sentinel/x.json\n").status,
            CheckStatus::Ok
        );
    }

    #[test]
    fn a_changed_backlog_offers_the_commit_fix() {
        let row = clean_row_for(" M backlog.md\n");
        assert_eq!(row.status, CheckStatus::NeedsAction);
        assert_eq!(row.fix.as_deref(), Some("Commit backlog.md"));
        assert_eq!(row.detail, "Uncommitted changes: backlog.md");
    }

    #[test]
    fn a_backlog_with_other_changes_offers_no_fix() {
        let row = clean_row_for("?? backlog.md\n?? other.txt\n");
        assert_eq!(row.fix, None);
        assert_eq!(row.detail, "Uncommitted changes: backlog.md, other.txt");
    }

    #[test]
    fn a_renamed_file_counts_by_its_new_name() {
        assert_eq!(
            porcelain_path("R  old.md -> backlog.md").as_deref(),
            Some("backlog.md")
        );
        assert_eq!(
            porcelain_path("?? \"my notes.md\"").as_deref(),
            Some("my notes.md")
        );
    }

    #[test]
    fn only_a_lone_backlog_path_counts_as_the_backlog_change() {
        assert!(!only_backlog_changed(&[]));
        assert!(only_backlog_changed(&["backlog.md".to_owned()]));
        assert!(!only_backlog_changed(&[
            "backlog.md".to_owned(),
            "a.md".to_owned()
        ]));
        assert!(!only_backlog_changed(&["a.md".to_owned()]));
    }
}
