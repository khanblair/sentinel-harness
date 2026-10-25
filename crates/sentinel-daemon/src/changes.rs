//! Live project state for the Changes and Board panels: the git working tree, the diff of one
//! file, and the backlog tasks before a run plans them. Git runs read-only with
//! `--no-optional-locks`, so a panel refresh never takes the index lock.

use std::collections::HashMap;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Component, Path};
use std::process::{Command, Stdio};

use sentinel_core::api::{BacklogTask, ChangeDiff, ChangedFile};

use crate::engine::{EngineError, Result};

/// Most files the Changes panel lists.
const MAX_FILES: usize = 300;
/// Most bytes of diff text one answer carries.
const MAX_DIFF_BYTES: usize = 200 * 1024;
/// Most characters of a backlog task summary.
const MAX_SUMMARY_CHARS: usize = 160;

/// Added and removed line counts, keyed by repository-relative path.
type LineCounts = HashMap<String, (u32, u32)>;

/// The working tree of the repository at `root`, sorted by path. A folder that is not a git
/// repository has no changes.
pub fn working_tree(root: &Path) -> Result<Vec<ChangedFile>> {
    if !is_repo(root) {
        return Ok(Vec::new());
    }
    let status = git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    let counts = line_counts(root)?;
    let mut files: Vec<ChangedFile> = parse_status(&status)
        .into_iter()
        .filter(|(path, _)| !is_sentinel_path(path))
        .map(|(path, word)| changed_file(root, path, word, &counts))
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files.truncate(MAX_FILES);
    Ok(files)
}

/// The diff of one file against HEAD. A file git does not track shows every line as added.
pub fn file_diff(root: &Path, path: &str) -> Result<ChangeDiff> {
    let safe = checked_relative(path)?;
    let text = if is_repo(root) {
        diff_text(root, safe)?
    } else {
        String::new()
    };
    let (diff, truncated) = cap_text(text);
    Ok(ChangeDiff {
        path: safe.to_owned(),
        diff,
        truncated,
    })
}

/// The tasks of the backlog file, parsed the way a run parses it. A missing file, or one with
/// no tasks, has none.
pub fn backlog_tasks(root: &Path, backlog: &str) -> Result<Vec<BacklogTask>> {
    let text = match fs::read_to_string(root.join(backlog)) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let items = sentinel_plan::backlog::parse_backlog(&text).unwrap_or_default();
    Ok(items
        .into_iter()
        .map(|item| BacklogTask {
            id: item.id.to_string(),
            summary: summary_of(&item.body),
            title: item.title,
        })
        .collect())
}

/// The first non-empty line of a task body, cut to the summary length.
pub fn summary_of(body: &str) -> String {
    body.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .chars()
        .take(MAX_SUMMARY_CHARS)
        .collect()
}

/// The word the Changes panel shows for a two-letter porcelain status.
pub fn status_word(xy: &[u8]) -> &'static str {
    let has = |letter: u8| xy.contains(&letter);
    if has(b'?') {
        "untracked"
    } else if has(b'R') || has(b'C') {
        "renamed"
    } else if has(b'D') {
        "deleted"
    } else if has(b'A') {
        "added"
    } else {
        "modified"
    }
}

/// Runs `git -C <root> <args>` and returns its stdout. A non-zero exit is an error.
fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("--no-optional-locks")
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(io::Error::other(stderr).into())
}

fn is_repo(root: &Path) -> bool {
    git(root, &["rev-parse", "--is-inside-work-tree"]).is_ok_and(|out| out.starts_with(b"true"))
}

fn has_head(root: &Path) -> bool {
    git(root, &["rev-parse", "--verify", "--quiet", "HEAD"]).is_ok()
}

fn is_tracked(root: &Path, path: &str) -> bool {
    git(root, &["ls-files", "--error-unmatch", "--", path]).is_ok()
}

/// One entry per changed path: its status word and path. A rename or copy record is followed by
/// its old path, which is skipped.
fn parse_status(raw: &[u8]) -> Vec<(String, &'static str)> {
    let mut entries = Vec::new();
    let mut records = raw.split(|byte| *byte == 0);
    while let Some(record) = records.next() {
        if record.len() < 4 {
            continue;
        }
        let (xy, rest) = record.split_at(2);
        let path = String::from_utf8_lossy(&rest[1..]).into_owned();
        entries.push((path, status_word(xy)));
        if xy.iter().any(|byte| matches!(byte, b'R' | b'C')) {
            records.next();
        }
    }
    entries
}

/// Added and removed lines per path, from `git diff --numstat -z HEAD`. A binary file counts as
/// zero lines. A rename record names its new path last.
fn parse_numstat(raw: &[u8]) -> LineCounts {
    let mut counts = LineCounts::new();
    let mut records = raw.split(|byte| *byte == 0);
    while let Some(record) = records.next() {
        let mut fields = record.splitn(3, |byte| *byte == b'\t');
        let (Some(added), Some(removed), Some(path)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let path = match path {
            [] => {
                records.next();
                records.next()
            }
            _ => Some(path),
        };
        let Some(path) = path else { continue };
        let key = String::from_utf8_lossy(path).into_owned();
        counts.insert(key, (count_field(added), count_field(removed)));
    }
    counts
}

fn count_field(field: &[u8]) -> u32 {
    std::str::from_utf8(field)
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(0)
}

fn line_counts(root: &Path) -> Result<LineCounts> {
    if !has_head(root) {
        return Ok(LineCounts::new());
    }
    let raw = git(root, &["diff", "--numstat", "-z", "--no-ext-diff", "HEAD"])?;
    Ok(parse_numstat(&raw))
}

fn changed_file(root: &Path, path: String, word: &str, counts: &LineCounts) -> ChangedFile {
    let (added, removed) = match counts.get(&path) {
        Some(pair) => *pair,
        None if word == "untracked" || word == "added" => (count_disk_lines(root, &path), 0),
        None => (0, 0),
    };
    ChangedFile {
        path,
        status: word.to_owned(),
        added,
        removed,
    }
}

/// The lines of a file on disk, for a file git has no diff for. Unreadable files count as zero.
fn count_disk_lines(root: &Path, path: &str) -> u32 {
    fs::read(root.join(path)).map_or(0, |bytes| {
        let text = String::from_utf8_lossy(&bytes);
        u32::try_from(text.lines().count()).unwrap_or(u32::MAX)
    })
}

fn diff_text(root: &Path, path: &str) -> Result<String> {
    let tracked = if has_head(root) {
        git(root, &["diff", "--no-ext-diff", "HEAD", "--", path])?
    } else {
        Vec::new()
    };
    if !tracked.is_empty() || is_tracked(root, path) || !root.join(path).is_file() {
        return Ok(String::from_utf8_lossy(&tracked).into_owned());
    }
    synthetic_added(root, path)
}

/// The text of an untracked file, shaped like a diff that adds every line.
fn synthetic_added(root: &Path, path: &str) -> Result<String> {
    let bytes = fs::read(root.join(path))?;
    let body = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = body.lines().collect();
    if lines.is_empty() {
        return Ok(String::new());
    }
    let mut text = format!(
        "--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{} @@\n",
        lines.len()
    );
    for line in lines {
        text.push('+');
        text.push_str(line);
        text.push('\n');
    }
    Ok(text)
}

/// Cuts diff text at the byte limit, on a character boundary. Says whether it cut.
fn cap_text(mut text: String) -> (String, bool) {
    if text.len() <= MAX_DIFF_BYTES {
        return (text, false);
    }
    let mut cut = MAX_DIFF_BYTES;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    text.truncate(cut);
    (text, true)
}

/// The path as given, when it names a file inside the project. Absolute paths, `..`, and the
/// `.sentinel` folder are refused.
fn checked_relative(path: &str) -> Result<&str> {
    let inside = Path::new(path)
        .components()
        .all(|part| matches!(part, Component::Normal(_)));
    if path.is_empty() || !inside || is_sentinel_path(path) {
        return Err(EngineError::Invalid(
            "Send a path inside the project.".to_owned(),
        ));
    }
    Ok(path)
}

fn is_sentinel_path(path: &str) -> bool {
    path == ".sentinel" || path.starts_with(".sentinel/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_available() -> bool {
        Command::new("git").arg("--version").output().is_ok()
    }

    /// Runs git in `root` with a fixed identity, and fails the test when git fails.
    fn git_in(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?} failed");
    }

    /// A repository with `tracked.txt` and `gone.txt` committed.
    fn committed_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        git_in(dir.path(), &["init", "-q"]);
        fs::write(dir.path().join("tracked.txt"), "a\nb\n").expect("write");
        fs::write(dir.path().join("gone.txt"), "old\n").expect("write");
        git_in(dir.path(), &["add", "-A"]);
        git_in(dir.path(), &["commit", "-q", "-m", "start"]);
        dir
    }

    #[test]
    fn status_word_names_each_porcelain_pair() {
        assert_eq!(status_word(b"??"), "untracked");
        assert_eq!(status_word(b" M"), "modified");
        assert_eq!(status_word(b"M "), "modified");
        assert_eq!(status_word(b"A "), "added");
        assert_eq!(status_word(b" D"), "deleted");
        assert_eq!(status_word(b"R "), "renamed");
    }

    #[test]
    fn numstat_counts_a_rename_under_its_new_path() {
        let counts = parse_numstat(b"1\t2\t\0old.rs\0new.rs\0-\t-\tlogo.png\0");
        assert_eq!(counts.get("new.rs"), Some(&(1, 2)));
        assert_eq!(counts.get("logo.png"), Some(&(0, 0)));
        assert_eq!(counts.get("old.rs"), None);
    }

    #[test]
    fn summary_is_the_first_text_line_cut_to_160_characters() {
        assert_eq!(summary_of("\n  \nFirst line.\nSecond."), "First line.");
        let long = "x".repeat(300);
        assert_eq!(summary_of(&long).chars().count(), 160);
        assert_eq!(summary_of(""), "");
    }

    #[test]
    fn paths_outside_the_project_are_refused() {
        assert!(checked_relative("../secret").is_err());
        assert!(checked_relative("/etc/passwd").is_err());
        assert!(checked_relative(".sentinel/log.jsonl").is_err());
        assert!(checked_relative("").is_err());
        assert_eq!(checked_relative("src/a.rs").ok(), Some("src/a.rs"));
    }

    #[test]
    fn a_folder_that_is_not_git_has_no_changes() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert_eq!(working_tree(dir.path()).expect("list").len(), 0);
    }

    #[test]
    fn working_tree_lists_modified_added_untracked_and_deleted_files() {
        if !git_available() {
            return;
        }
        let dir = committed_repo();
        fs::write(dir.path().join("tracked.txt"), "a\nc\n").expect("write");
        fs::write(dir.path().join("new.txt"), "x\ny\n").expect("write");
        fs::remove_file(dir.path().join("gone.txt")).expect("remove");
        let files = working_tree(dir.path()).expect("list");
        let view: Vec<(&str, &str, u32, u32)> = files
            .iter()
            .map(|f| (f.path.as_str(), f.status.as_str(), f.added, f.removed))
            .collect();
        assert_eq!(
            view,
            vec![
                ("gone.txt", "deleted", 0, 1),
                ("new.txt", "untracked", 2, 0),
                ("tracked.txt", "modified", 1, 1),
            ]
        );
    }

    #[test]
    fn the_diff_of_a_modified_file_holds_the_change() {
        if !git_available() {
            return;
        }
        let dir = committed_repo();
        fs::write(dir.path().join("tracked.txt"), "a\nc\n").expect("write");
        let diff = file_diff(dir.path(), "tracked.txt").expect("diff");
        assert!(diff.diff.contains("+c\n"));
        assert!(diff.diff.contains("-b\n"));
        assert!(!diff.truncated);
    }

    #[test]
    fn the_diff_of_an_untracked_file_adds_every_line() {
        if !git_available() {
            return;
        }
        let dir = committed_repo();
        fs::write(dir.path().join("new.txt"), "x\ny\n").expect("write");
        let diff = file_diff(dir.path(), "new.txt").expect("diff");
        assert!(diff.diff.contains("+x\n+y\n"));
    }

    #[test]
    fn a_diff_request_with_dot_dot_is_rejected() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(file_diff(dir.path(), "../outside.txt").is_err());
    }

    #[test]
    fn long_diffs_are_cut_on_a_character_boundary() {
        let text = "é".repeat(MAX_DIFF_BYTES);
        let (cut, truncated) = cap_text(text);
        assert!(truncated);
        assert!(cut.len() <= MAX_DIFF_BYTES);
        assert!(cut.is_char_boundary(cut.len()));
    }

    #[test]
    fn backlog_parses_three_tasks_and_a_missing_file_has_none() {
        let dir = tempfile::tempdir().expect("temp dir");
        let text = "## T-01 First task\nDo the first thing.\n\n## T-02 Second task\nDo two.\n\n## T-03 Third task\nDo three.\n";
        fs::write(dir.path().join("backlog.md"), text).expect("write");
        let tasks = backlog_tasks(dir.path(), "backlog.md").expect("parse");
        let ids: Vec<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["T-01", "T-02", "T-03"]);
        assert_eq!(tasks[0].title, "First task");
        assert_eq!(tasks[0].summary, "Do the first thing.");
        let none = backlog_tasks(dir.path(), "missing.md").expect("missing is fine");
        assert_eq!(none.len(), 0);
    }
}
