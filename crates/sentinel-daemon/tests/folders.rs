//! Folder browser, repository checks, and starter backlog behavior.

use std::fs;
use std::path::Path;
use std::process::Command;

use sentinel_core::api::{CheckRow, CheckStatus, FolderEntry, RepoCheckResponse};
use sentinel_daemon::folders::{
    self, FolderError, check_repo, create_starter_backlog, list_folders, within_home,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn git(folder: &Path, args: &[&str]) -> TestResult {
    let status = Command::new("git")
        .args(args)
        .current_dir(folder)
        .status()?;
    assert!(status.success(), "git {args:?} failed");
    Ok(())
}

fn init_repo_with_backlog(folder: &Path) -> TestResult {
    git(folder, &["init"])?;
    fs::write(folder.join("backlog.md"), "# Backlog\n")?;
    git(folder, &["add", "."])?;
    git(
        folder,
        &[
            "-c",
            "user.email=test@example.com",
            "-c",
            "user.name=Test",
            "commit",
            "-m",
            "init",
        ],
    )
}

fn row<'a>(report: &'a RepoCheckResponse, name: &str) -> std::result::Result<&'a CheckRow, String> {
    report
        .rows
        .iter()
        .find(|row| row.name == name)
        .ok_or_else(|| format!("missing row {name}"))
}

#[test]
fn list_sorts_folders_by_name_and_hides_dot_folders() -> TestResult {
    let home = tempfile::tempdir()?;
    for name in ["beta", "Alpha", ".hidden", "gamma"] {
        fs::create_dir(home.path().join(name))?;
    }
    let folders: Vec<FolderEntry> = list_folders(home.path(), home.path())?;
    let names: Vec<&str> = folders.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, vec!["Alpha", "beta", "gamma"]);
    assert!(
        folders
            .iter()
            .all(|entry| Path::new(&entry.path).is_absolute())
    );
    Ok(())
}

#[test]
fn list_refuses_a_path_that_leaves_home_by_parent_traversal() -> TestResult {
    let home = tempfile::tempdir()?;
    let error = list_folders(home.path(), &home.path().join(".."))
        .err()
        .ok_or("expected a refusal")?;
    assert!(matches!(error, FolderError::ParentTraversal));
    Ok(())
}

#[cfg(unix)]
#[test]
fn list_refuses_a_symlink_that_leaves_home() -> TestResult {
    let home = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    std::os::unix::fs::symlink(outside.path(), home.path().join("escape"))?;
    let folders = list_folders(home.path(), home.path())?;
    assert_eq!(folders, Vec::<FolderEntry>::new());
    assert!(!within_home(home.path(), &home.path().join("escape")));
    Ok(())
}

#[test]
fn check_repo_accepts_a_clean_git_repo_with_a_backlog() -> TestResult {
    let folder = tempfile::tempdir()?;
    init_repo_with_backlog(folder.path())?;
    let report = check_repo(folder.path());
    assert!(report.usable, "{report:?}");
    assert_eq!(report.rows.len(), 3);
    assert_eq!(row(&report, "Git repository")?.status, CheckStatus::Ok);
    assert_eq!(row(&report, "backlog.md")?.status, CheckStatus::Ok);
    assert_eq!(row(&report, "Clean working tree")?.status, CheckStatus::Ok);
    Ok(())
}

#[test]
fn check_repo_missing_backlog_offers_the_fix_label() -> TestResult {
    let folder = tempfile::tempdir()?;
    git(folder.path(), &["init"])?;
    let report = check_repo(folder.path());
    let backlog = row(&report, "backlog.md")?;
    assert_eq!(backlog.status, CheckStatus::NeedsAction);
    assert_eq!(backlog.fix.as_deref(), Some("Create a starter backlog"));
    assert!(!report.usable);
    Ok(())
}

#[test]
fn check_repo_dirty_tree_is_not_usable() -> TestResult {
    let folder = tempfile::tempdir()?;
    init_repo_with_backlog(folder.path())?;
    fs::write(folder.path().join("new-file.txt"), "work")?;
    let report = check_repo(folder.path());
    assert_eq!(
        row(&report, "Clean working tree")?.status,
        CheckStatus::NeedsAction
    );
    assert!(!report.usable);
    Ok(())
}

#[test]
fn check_repo_skips_the_tree_row_without_git() -> TestResult {
    let folder = tempfile::tempdir()?;
    fs::write(folder.path().join("backlog.md"), "# Backlog\n")?;
    let report = check_repo(folder.path());
    assert_eq!(
        row(&report, "Clean working tree")?.status,
        CheckStatus::NeedsAction
    );
    assert!(!report.usable);
    Ok(())
}

#[test]
fn check_repo_of_a_file_is_invalid() -> TestResult {
    let folder = tempfile::tempdir()?;
    let file = folder.path().join("notes.txt");
    fs::write(&file, "text")?;
    let report = check_repo(&file);
    assert!(!report.usable);
    assert_eq!(report.rows.len(), 1);
    assert_eq!(report.rows[0].status, CheckStatus::Invalid);
    Ok(())
}

#[test]
fn starter_backlog_is_created_once() -> TestResult {
    let folder = tempfile::tempdir()?;
    create_starter_backlog(folder.path())?;
    let text = fs::read_to_string(folder.path().join("backlog.md"))?;
    assert!(text.starts_with("# Backlog"));
    assert_eq!(text.matches("## T-0").count(), 3);
    let error = create_starter_backlog(folder.path())
        .err()
        .ok_or("expected a refusal")?;
    assert!(matches!(error, FolderError::Exists(_)));
    Ok(())
}

#[test]
fn within_home_accepts_a_child_folder() -> TestResult {
    let home = tempfile::tempdir()?;
    fs::create_dir(home.path().join("child"))?;
    assert!(within_home(home.path(), &home.path().join("child")));
    assert!(within_home(home.path(), home.path()));
    Ok(())
}

#[test]
fn an_empty_home_lists_no_children() -> TestResult {
    let home = tempfile::tempdir()?;
    let folders = folders::list_folders(home.path(), home.path())?;
    assert_eq!(folders, Vec::<FolderEntry>::new());
    Ok(())
}

#[test]
fn sentinel_folder_is_excluded_locally_once() {
    let dir = tempfile::tempdir().unwrap();
    std::process::Command::new("git")
        .current_dir(dir.path())
        .args(["init", "-q", "-b", "main"])
        .status()
        .unwrap();
    sentinel_daemon::folders::exclude_sentinel_folder(dir.path());
    sentinel_daemon::folders::exclude_sentinel_folder(dir.path());
    let text = std::fs::read_to_string(dir.path().join(".git/info/exclude")).unwrap();
    assert_eq!(text.matches("/.sentinel/").count(), 1, "{text}");
}
