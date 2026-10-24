//! A reverted checkpoint puts files back and keeps history append only.
#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use std::fs;
use std::path::Path;
use std::process::Command;

use sentinel_tree::{Git, checkpoint, revert_checkpoint};

fn git_cmd(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_owned()
}

fn repo_with_one_commit() -> (tempfile::TempDir, Git) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git_cmd(root, &["init", "-b", "main"]);
    git_cmd(root, &["config", "user.name", "Test"]);
    git_cmd(root, &["config", "user.email", "test@example.com"]);
    fs::write(root.join("a.txt"), "one\n").unwrap();
    git_cmd(root, &["add", "a.txt"]);
    git_cmd(root, &["commit", "-m", "start"]);
    let git = Git::new(root, root.join(".sentinel/index")).unwrap();
    git.reset_private_index().unwrap();
    (dir, git)
}

#[test]
fn revert_restores_a_changed_file_and_removes_an_added_one() {
    let (dir, git) = repo_with_one_commit();
    fs::write(dir.path().join("a.txt"), "two\n").unwrap();
    fs::write(dir.path().join("b.txt"), "new\n").unwrap();
    let cp = checkpoint(&git, &["a.txt".into(), "b.txt".into()], "T-01")
        .unwrap()
        .unwrap();
    let undo = revert_checkpoint(&git, &cp, "revert T-01")
        .unwrap()
        .unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "one\n"
    );
    assert!(!dir.path().join("b.txt").exists());
    assert_eq!(git.head().unwrap().as_deref(), Some(undo.commit.as_str()));
    assert!(git.is_clean().unwrap());
}

#[test]
fn history_keeps_the_original_checkpoint_and_the_revert() {
    let (dir, git) = repo_with_one_commit();
    fs::write(dir.path().join("a.txt"), "two\n").unwrap();
    let cp = checkpoint(&git, &["a.txt".into()], "T-01")
        .unwrap()
        .unwrap();
    revert_checkpoint(&git, &cp, "revert T-01").unwrap();
    let log = git_cmd(dir.path(), &["log", "--format=%s"]);
    assert_eq!(
        log.lines().collect::<Vec<_>>(),
        ["revert T-01", "T-01", "start"]
    );
}

#[test]
fn reverting_a_root_commit_is_an_error() {
    let (dir, git) = repo_with_one_commit();
    let root_commit = git.head().unwrap().unwrap();
    let cp = sentinel_tree::Checkpoint {
        commit: root_commit,
        paths: vec!["a.txt".into()],
    };
    assert!(revert_checkpoint(&git, &cp, "revert").is_err());
    drop(dir);
}
