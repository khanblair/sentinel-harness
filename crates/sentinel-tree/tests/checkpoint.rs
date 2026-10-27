//! Integration tests for checkpoints, restore, clean-tree checks, and tamper checks.
//!
//! Every test builds a real git repo in a temp folder and drives the real git binary.

use std::fs;
use std::path::Path;
use std::process::Command;

use sentinel_tree::{
    Checkpoint, Git, RestoreOutcome, TreeError, checkpoint, edited_tests, restore_path,
};

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

struct TestRepo {
    dir: tempfile::TempDir,
    git: Git,
}

impl TestRepo {
    fn root(&self) -> &Path {
        self.dir.path()
    }
}

fn git_cmd(root: &Path, args: &[&str]) -> TestResult<String> {
    let output = Command::new("git")
        .current_dir(root)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .args(args)
        .output()?;
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_owned())
}

fn write_file(root: &Path, path: &str, body: &str) -> TestResult<()> {
    let full = root.join(path);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(full, body)?;
    Ok(())
}

fn init_repo() -> TestResult<tempfile::TempDir> {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    git_cmd(root, &["init", "-b", "main"])?;
    git_cmd(root, &["config", "user.name", "Test"])?;
    git_cmd(root, &["config", "user.email", "test@example.com"])?;
    git_cmd(root, &["config", "commit.gpgsign", "false"])?;
    Ok(dir)
}

fn repo_with_commit() -> TestResult<TestRepo> {
    let dir = init_repo()?;
    let root = dir.path();
    write_file(root, "src/lib.rs", "one\n")?;
    write_file(root, "README.md", "readme\n")?;
    git_cmd(root, &["add", "--", "src/lib.rs", "README.md"])?;
    git_cmd(root, &["commit", "-m", "init"])?;
    let git = Git::new(root, root.join(".sentinel/index"))?;
    git.reset_private_index()?;
    Ok(TestRepo { dir, git })
}

fn repo_without_commit() -> TestResult<TestRepo> {
    let dir = init_repo()?;
    let root = dir.path();
    write_file(root, "src/lib.rs", "one\n")?;
    let git = Git::new(root, root.join(".sentinel/index"))?;
    git.reset_private_index()?;
    Ok(TestRepo { dir, git })
}

fn commit_paths(root: &Path, from: &str, to: &str) -> TestResult<Vec<String>> {
    let raw = git_cmd(
        root,
        &[
            "diff-tree",
            "-r",
            "-z",
            "--name-only",
            "--no-commit-id",
            from,
            to,
        ],
    )?;
    Ok(raw
        .split('\0')
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect())
}

#[test]
fn git_new_rejects_a_folder_without_a_repository() {
    let dir = tempfile::tempdir().unwrap();
    let error = Git::new(dir.path(), dir.path().join("index")).unwrap_err();
    assert!(matches!(error, TreeError::NotARepo(_)));
}

#[test]
fn changed_paths_reports_modified_untracked_and_deleted_files() {
    let repo = repo_with_commit().unwrap();
    write_file(repo.root(), "src/lib.rs", "two\n").unwrap();
    write_file(repo.root(), "new.txt", "new\n").unwrap();
    fs::remove_file(repo.root().join("README.md")).unwrap();
    write_file(repo.root(), ".sentinel/scratch", "private\n").unwrap();

    assert_eq!(
        repo.git.changed_paths().unwrap(),
        vec!["README.md", "new.txt", "src/lib.rs"]
    );
}

#[test]
fn checkpoint_commits_only_the_named_paths_and_leaves_the_real_index_alone() {
    let repo = repo_with_commit().unwrap();
    write_file(repo.root(), "src/lib.rs", "two\n").unwrap();
    write_file(repo.root(), "src/main.rs", "main\n").unwrap();
    write_file(repo.root(), "README.md", "changed\n").unwrap();
    let index_path = repo.root().join(".git/index");
    let real_index_before = fs::read(&index_path).unwrap();
    let head_before = repo.git.head().unwrap().unwrap();
    let paths = vec!["src/lib.rs".to_owned(), "src/main.rs".to_owned()];

    let committed = checkpoint(&repo.git, &paths, "P2-02: two files")
        .unwrap()
        .unwrap();

    assert_eq!(fs::read(&index_path).unwrap(), real_index_before);
    assert_eq!(
        commit_paths(repo.root(), &head_before, &committed.commit).unwrap(),
        vec!["src/lib.rs", "src/main.rs"]
    );
    assert_eq!(repo.git.changed_paths().unwrap(), vec!["README.md"]);
    assert_eq!(committed.paths, paths);
}

#[test]
fn checkpoint_with_no_change_returns_none_and_keeps_head() {
    let repo = repo_with_commit().unwrap();
    let head_before = repo.git.head().unwrap();

    let result = checkpoint(&repo.git, &["src/lib.rs".to_owned()], "no change").unwrap();

    assert!(result.is_none());
    assert_eq!(repo.git.head().unwrap(), head_before);
}

#[test]
fn checkpoint_records_a_deletion() {
    let repo = repo_with_commit().unwrap();
    fs::remove_file(repo.root().join("README.md")).unwrap();

    let committed = checkpoint(&repo.git, &["README.md".to_owned()], "remove readme")
        .unwrap()
        .unwrap();

    assert!(
        repo.git
            .file_at(&committed.commit, "README.md")
            .unwrap()
            .is_none()
    );
    assert_eq!(repo.git.changed_paths().unwrap(), Vec::<String>::new());
}

#[test]
fn checkpoint_in_a_repo_without_commits_makes_a_root_commit() {
    let repo = repo_without_commit().unwrap();
    assert!(repo.git.head().unwrap().is_none());

    let committed = checkpoint(&repo.git, &["src/lib.rs".to_owned()], "root")
        .unwrap()
        .unwrap();

    let lineage = git_cmd(
        repo.root(),
        &["rev-list", "--parents", "-n", "1", &committed.commit],
    )
    .unwrap();
    assert_eq!(lineage.split_whitespace().count(), 1);
    assert_eq!(
        repo.git.head().unwrap().as_deref(),
        Some(committed.commit.as_str())
    );
    assert!(repo.git.is_clean().unwrap());
}

#[test]
fn two_checkpoints_form_a_parent_chain() {
    let repo = repo_with_commit().unwrap();
    write_file(repo.root(), "src/lib.rs", "two\n").unwrap();
    let first = checkpoint(&repo.git, &["src/lib.rs".to_owned()], "first")
        .unwrap()
        .unwrap();
    write_file(repo.root(), "src/lib.rs", "three\n").unwrap();
    let second = checkpoint(&repo.git, &["src/lib.rs".to_owned()], "second")
        .unwrap()
        .unwrap();

    let parent_spec = format!("{}^", second.commit);
    let parent = git_cmd(repo.root(), &["rev-parse", &parent_spec]).unwrap();

    assert_eq!(parent, first.commit);
}

#[test]
fn checkpoint_rejects_paths_that_leave_the_repo() {
    let repo = repo_with_commit().unwrap();
    let head_before = repo.git.head().unwrap();

    for path in ["../x", "/etc/passwd", ""] {
        let error = checkpoint(&repo.git, &[path.to_owned()], "bad path").unwrap_err();
        assert!(
            matches!(error, TreeError::InvalidPath(_)),
            "{path} should be rejected"
        );
    }
    assert_eq!(repo.git.head().unwrap(), head_before);
}

#[test]
fn is_clean_is_false_until_every_changed_path_is_checkpointed() {
    let repo = repo_with_commit().unwrap();
    assert!(repo.git.is_clean().unwrap());

    write_file(repo.root(), "src/lib.rs", "changed\n").unwrap();
    assert!(!repo.git.is_clean().unwrap());
    let changed = repo.git.changed_paths().unwrap();
    assert_eq!(changed, vec!["src/lib.rs"]);
    checkpoint(&repo.git, &changed, "P2-06: tracked")
        .unwrap()
        .unwrap();
    assert!(repo.git.is_clean().unwrap());

    write_file(repo.root(), "stray.txt", "stray\n").unwrap();
    assert!(!repo.git.is_clean().unwrap());
    let changed = repo.git.changed_paths().unwrap();
    assert_eq!(changed, vec!["stray.txt"]);
    checkpoint(&repo.git, &changed, "P2-06: stray")
        .unwrap()
        .unwrap();
    assert!(repo.git.is_clean().unwrap());
}

#[test]
fn restore_path_rewrites_removes_and_reports_unchanged() {
    let repo = repo_with_commit().unwrap();
    let head = repo.git.head().unwrap().unwrap();
    let tracked = repo.root().join("src/lib.rs");
    write_file(repo.root(), "src/lib.rs", "changed\n").unwrap();

    assert_eq!(
        restore_path(&repo.git, "src/lib.rs", &head).unwrap(),
        RestoreOutcome::Rewritten
    );
    assert_eq!(fs::read_to_string(&tracked).unwrap(), "one\n");
    assert_eq!(
        restore_path(&repo.git, "src/lib.rs", &head).unwrap(),
        RestoreOutcome::Unchanged
    );

    write_file(repo.root(), "stray.txt", "stray\n").unwrap();
    assert_eq!(
        restore_path(&repo.git, "stray.txt", &head).unwrap(),
        RestoreOutcome::Removed
    );
    assert!(!repo.root().join("stray.txt").exists());
    assert_eq!(
        restore_path(&repo.git, "stray.txt", &head).unwrap(),
        RestoreOutcome::Unchanged
    );
}

#[test]
fn restore_path_rejects_paths_that_leave_the_repo() {
    let repo = repo_with_commit().unwrap();
    let head = repo.git.head().unwrap().unwrap();
    let error = restore_path(&repo.git, "../x", &head).unwrap_err();
    assert!(matches!(error, TreeError::InvalidPath(_)));
}

#[test]
fn edited_tests_flags_edits_and_deletions_but_not_additions() {
    let repo = repo_with_commit().unwrap();
    write_file(repo.root(), "tests/a.rs", "test one\n").unwrap();
    write_file(repo.root(), "src/b.test.ts", "test one\n").unwrap();
    git_cmd(repo.root(), &["add", "--", "tests/a.rs", "src/b.test.ts"]).unwrap();
    git_cmd(repo.root(), &["commit", "-m", "base tests"]).unwrap();
    let base = git_cmd(repo.root(), &["rev-parse", "HEAD"]).unwrap();

    write_file(repo.root(), "tests/a.rs", "test two\n").unwrap();
    write_file(repo.root(), "src/b.test.ts", "test two\n").unwrap();
    write_file(repo.root(), "tests/new.rs", "new\n").unwrap();
    write_file(repo.root(), "src/lib.rs", "changed\n").unwrap();
    git_cmd(
        repo.root(),
        &[
            "add",
            "--",
            "tests/a.rs",
            "src/b.test.ts",
            "tests/new.rs",
            "src/lib.rs",
        ],
    )
    .unwrap();
    git_cmd(repo.root(), &["commit", "-m", "edit tests"]).unwrap();
    let head = git_cmd(repo.root(), &["rev-parse", "HEAD"]).unwrap();

    assert_eq!(
        edited_tests(&repo.git, &base, &head).unwrap(),
        vec!["src/b.test.ts", "tests/a.rs"]
    );
}

#[test]
fn edited_tests_flags_a_deleted_test_file() {
    let repo = repo_with_commit().unwrap();
    write_file(repo.root(), "tests/a.rs", "test one\n").unwrap();
    git_cmd(repo.root(), &["add", "--", "tests/a.rs"]).unwrap();
    git_cmd(repo.root(), &["commit", "-m", "base tests"]).unwrap();
    let base = git_cmd(repo.root(), &["rev-parse", "HEAD"]).unwrap();

    fs::remove_file(repo.root().join("tests/a.rs")).unwrap();
    git_cmd(repo.root(), &["add", "-A", "--", "tests/a.rs"]).unwrap();
    git_cmd(repo.root(), &["commit", "-m", "drop test"]).unwrap();
    let head = git_cmd(repo.root(), &["rev-parse", "HEAD"]).unwrap();

    assert_eq!(
        edited_tests(&repo.git, &base, &head).unwrap(),
        vec!["tests/a.rs"]
    );
}

#[test]
fn sync_real_index_makes_git_status_clean_after_checkpoints() {
    let repo = repo_with_commit().unwrap();
    write_file(repo.root(), "src/lib.rs", "two\n").unwrap();
    write_file(repo.root(), "sentinel-tests/t01.test.js", "test\n").unwrap();
    let paths = vec![
        "src/lib.rs".to_owned(),
        "sentinel-tests/t01.test.js".to_owned(),
    ];
    checkpoint(&repo.git, &paths, "T01: two files")
        .unwrap()
        .unwrap();
    let stale = git_cmd(repo.root(), &["status", "--porcelain"]).unwrap();
    assert_ne!(stale, "", "the real index is stale after a checkpoint");

    repo.git.sync_real_index().unwrap();

    // Only the private index folder is left, which a real repository ignores.
    assert_eq!(
        git_cmd(repo.root(), &["status", "--porcelain"]).unwrap(),
        "?? .sentinel/"
    );
    assert_eq!(
        fs::read_to_string(repo.root().join("src/lib.rs")).unwrap(),
        "two\n"
    );
}

#[test]
fn sync_real_index_does_nothing_in_a_repo_without_commits() {
    let repo = repo_without_commit().unwrap();

    repo.git.sync_real_index().unwrap();

    assert_eq!(repo.git.head().unwrap(), None);
}

#[test]
fn checkpoint_moves_head_and_keeps_the_private_index_aligned() {
    let repo = repo_with_commit().unwrap();
    write_file(repo.root(), "src/lib.rs", "two\n").unwrap();
    let committed: Checkpoint = checkpoint(&repo.git, &["src/lib.rs".to_owned()], "move")
        .unwrap()
        .unwrap();

    assert_eq!(
        repo.git.head().unwrap().as_deref(),
        Some(committed.commit.as_str())
    );
    assert!(repo.git.is_clean().unwrap());
}
