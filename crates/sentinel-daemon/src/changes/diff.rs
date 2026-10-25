//! The diff of one file against HEAD, for the Changes panel. An untracked file shows every line as
//! added. The text is cut at 200 KB.

use std::fs;
use std::path::{Component, Path};

use sentinel_core::api::ChangeDiff;

use super::{git, has_head, is_repo, is_sentinel_path};
use crate::engine::{EngineError, Result};

/// Most bytes of diff text one answer carries.
const MAX_DIFF_BYTES: usize = 200 * 1024;

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

fn is_tracked(root: &Path, path: &str) -> bool {
    git(root, &["ls-files", "--error-unmatch", "--", path]).is_ok()
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

#[cfg(test)]
mod tests {
    use super::super::tests::{committed_repo, git_available};
    use super::*;

    #[test]
    fn paths_outside_the_project_are_refused() {
        assert!(checked_relative("../secret").is_err());
        assert!(checked_relative("/etc/passwd").is_err());
        assert!(checked_relative(".sentinel/log.jsonl").is_err());
        assert!(checked_relative("").is_err());
        assert_eq!(checked_relative("src/a.rs").ok(), Some("src/a.rs"));
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
}
