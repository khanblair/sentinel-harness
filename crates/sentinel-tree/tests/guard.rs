//! Integration tests for the write guard watcher and the guard files.
//!
//! Every guard test builds a real git repo in a temp folder and drives the real git binary.
//! Waiting uses `tokio::time::timeout`; the debounce does the timing, not a sleep.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use sentinel_core::{Event, Scope, TaskId};
use sentinel_tree::{
    Git, GuardHandle, GuardOptions, TreeError, WritePolicy, WriteVerdict, remove_guard_files,
    start_guard, write_guard_files,
};
use tokio::sync::mpsc::{self, UnboundedReceiver};

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

const THREE_SECONDS: Duration = Duration::from_secs(3);
const ONE_SECOND: Duration = Duration::from_secs(1);
const QUIET_WINDOW: Duration = Duration::from_millis(700);

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

fn read_file(root: &Path, path: &str) -> TestResult<String> {
    Ok(fs::read_to_string(root.join(path))?)
}

fn repo_with_commit() -> TestResult<TestRepo> {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    git_cmd(root, &["init", "-b", "main"])?;
    git_cmd(root, &["config", "user.name", "Test"])?;
    git_cmd(root, &["config", "user.email", "test@example.com"])?;
    git_cmd(root, &["config", "commit.gpgsign", "false"])?;
    write_file(root, "src/lib.rs", "one\n")?;
    write_file(root, "README.md", "readme\n")?;
    write_file(root, ".gitignore", "target/\n")?;
    git_cmd(root, &["add", "-A"])?;
    git_cmd(root, &["commit", "-m", "init"])?;
    let git = Git::new(root, root.join(".sentinel/index"))?;
    git.reset_private_index()?;
    Ok(TestRepo { dir, git })
}

/// Blocks every path except the ones named in `allowed`.
struct BlockingPolicy {
    allowed: HashSet<String>,
    holder: Option<TaskId>,
}

impl WritePolicy for BlockingPolicy {
    fn check(&self, path: &str) -> WriteVerdict {
        if self.allowed.contains(path) {
            WriteVerdict::Allowed
        } else {
            WriteVerdict::Blocked {
                holder: self.holder.clone(),
            }
        }
    }
}

fn policy(allowed: &[&str], holder: Option<TaskId>) -> Arc<dyn WritePolicy> {
    Arc::new(BlockingPolicy {
        allowed: allowed.iter().map(|path| (*path).to_owned()).collect(),
        holder,
    })
}

fn start(
    repo: &TestRepo,
    policy: Arc<dyn WritePolicy>,
) -> TestResult<(GuardHandle, UnboundedReceiver<Event>)> {
    let (events, incoming) = mpsc::unbounded_channel();
    let handle = start_guard(repo.git.clone(), policy, events, GuardOptions::default())?;
    Ok((handle, incoming))
}

async fn next_event(events: &mut UnboundedReceiver<Event>) -> TestResult<Event> {
    let event = tokio::time::timeout(THREE_SECONDS, events.recv())
        .await?
        .ok_or("guard closed the channel")?;
    Ok(event)
}

async fn expect_no_event(events: &mut UnboundedReceiver<Event>) -> TestResult<()> {
    assert!(
        tokio::time::timeout(QUIET_WINDOW, events.recv())
            .await
            .is_err(),
        "a blocked write should not be reported"
    );
    Ok(())
}

#[tokio::test]
async fn a_tracked_edit_is_restored_and_reported_within_a_second() -> TestResult<()> {
    let repo = repo_with_commit()?;
    let (_handle, mut events) = start(&repo, policy(&[], None))?;

    write_file(repo.root(), "src/lib.rs", "stray\n")?;
    let event = tokio::time::timeout(ONE_SECOND, events.recv())
        .await?
        .ok_or("event did not arrive within one second")?;

    assert_eq!(
        event,
        Event::WriteBlocked {
            task: None,
            path: "src/lib.rs".to_owned(),
            restored: true,
        }
    );
    assert_eq!(read_file(repo.root(), "src/lib.rs")?, "one\n");
    Ok(())
}

#[tokio::test]
async fn a_new_untracked_file_is_removed_and_reported() -> TestResult<()> {
    let repo = repo_with_commit()?;
    let (_handle, mut events) = start(&repo, policy(&[], None))?;

    write_file(repo.root(), "notes.txt", "stray\n")?;
    let event = next_event(&mut events).await?;

    assert_eq!(
        event,
        Event::WriteBlocked {
            task: None,
            path: "notes.txt".to_owned(),
            restored: true,
        }
    );
    assert!(!repo.root().join("notes.txt").exists());
    Ok(())
}

#[tokio::test]
async fn a_write_to_an_allowed_path_stays_and_sends_nothing() -> TestResult<()> {
    let repo = repo_with_commit()?;
    let (_handle, mut events) = start(&repo, policy(&["src/allowed.rs"], None))?;

    write_file(repo.root(), "src/allowed.rs", "mine\n")?;

    expect_no_event(&mut events).await?;
    assert_eq!(read_file(repo.root(), "src/allowed.rs")?, "mine\n");
    Ok(())
}

#[tokio::test]
async fn sentinel_git_and_ignored_paths_are_left_alone() -> TestResult<()> {
    let repo = repo_with_commit()?;
    let (_handle, mut events) = start(&repo, policy(&[], None))?;

    write_file(repo.root(), ".sentinel/scratch", "private\n")?;
    write_file(repo.root(), ".git/guard-note", "private\n")?;
    write_file(repo.root(), "target/out.txt", "build\n")?;

    expect_no_event(&mut events).await?;
    assert!(repo.root().join(".sentinel/scratch").exists());
    assert!(repo.root().join(".git/guard-note").exists());
    assert!(repo.root().join("target/out.txt").exists());
    Ok(())
}

#[tokio::test]
async fn a_burst_of_writes_sends_one_event_per_path_and_ends_restored() -> TestResult<()> {
    let repo = repo_with_commit()?;
    let (_handle, mut events) = start(&repo, policy(&[], None))?;

    for attempt in 0..30 {
        write_file(repo.root(), "src/lib.rs", &format!("stray {attempt}\n"))?;
    }
    let first = next_event(&mut events).await?;
    assert!(matches!(first, Event::WriteBlocked { restored: true, .. }));
    while let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(300), events.recv()).await {}
    assert_eq!(read_file(repo.root(), "src/lib.rs")?, "one\n");
    Ok(())
}

#[tokio::test]
async fn the_policy_holder_travels_with_the_blocked_event() -> TestResult<()> {
    let repo = repo_with_commit()?;
    let holder = TaskId::new("T-07");
    let (_handle, mut events) = start(&repo, policy(&[], Some(holder.clone())))?;

    write_file(repo.root(), "src/lib.rs", "stray\n")?;
    let event = next_event(&mut events).await?;

    assert_eq!(
        event,
        Event::WriteBlocked {
            task: Some(holder),
            path: "src/lib.rs".to_owned(),
            restored: true,
        }
    );
    Ok(())
}

#[tokio::test]
async fn dropping_the_handle_stops_the_guard() -> TestResult<()> {
    let repo = repo_with_commit()?;
    let (handle, mut events) = start(&repo, policy(&[], None))?;

    drop(handle);
    write_file(repo.root(), "src/lib.rs", "stray\n")?;

    assert!(
        tokio::time::timeout(QUIET_WINDOW, events.recv())
            .await?
            .is_none(),
        "the stopped guard must not send events"
    );
    assert_eq!(read_file(repo.root(), "src/lib.rs")?, "stray\n");
    Ok(())
}

#[test]
fn guard_files_hold_the_sorted_scope() -> TestResult<()> {
    let dir = tempfile::tempdir()?;
    let guards = dir.path().join("guards");
    let task = TaskId::new("T-01");
    let scope = Scope::from_paths(["src/limits/*", "src/a.rs"])?;

    let files = write_guard_files(&guards, &task, &scope)?;

    assert_eq!(files.allowed_json, guards.join("T-01.allowed.json"));
    let text = fs::read_to_string(&files.allowed_json)?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(value["task"], "T-01");
    assert_eq!(
        value["allowed"],
        serde_json::json!(["src/a.rs", "src/limits/*"])
    );
    assert!(text.contains('\n'), "guard file should be pretty printed");
    Ok(())
}

#[test]
fn rewriting_guard_files_replaces_the_old_content() -> TestResult<()> {
    let dir = tempfile::tempdir()?;
    let guards = dir.path().join("guards");
    let task = TaskId::new("T-02");

    let first = write_guard_files(&guards, &task, &Scope::from_paths(["src/a.rs"])?)?;
    let second = write_guard_files(&guards, &task, &Scope::from_paths(["src/b.rs"])?)?;

    assert_eq!(first.allowed_json, second.allowed_json);
    let value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&second.allowed_json)?)?;
    assert_eq!(value["allowed"], serde_json::json!(["src/b.rs"]));
    let entries: Vec<String> = fs::read_dir(&guards)?
        .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, std::io::Error>>()?;
    assert_eq!(entries, vec!["T-02.allowed.json"]);
    Ok(())
}

#[test]
fn a_task_id_with_a_path_part_is_rejected() -> TestResult<()> {
    let dir = tempfile::tempdir()?;
    let guards = dir.path().join("guards");

    for id in ["T/01", "T\\01", "T..01"] {
        let task = TaskId::new(id);
        let error = write_guard_files(&guards, &task, &Scope::new()).unwrap_err();
        assert!(
            matches!(error, TreeError::InvalidPath(_)),
            "{id} should be rejected"
        );
        assert!(remove_guard_files(&guards, &task).is_err());
    }
    assert!(!guards.exists());
    Ok(())
}

#[test]
fn removing_a_guard_file_twice_succeeds() -> TestResult<()> {
    let dir = tempfile::tempdir()?;
    let guards = dir.path().join("guards");
    let task = TaskId::new("T-03");
    let files = write_guard_files(&guards, &task, &Scope::from_paths(["src/a.rs"])?)?;

    remove_guard_files(&guards, &task)?;
    remove_guard_files(&guards, &task)?;

    assert!(!files.allowed_json.exists());
    Ok(())
}
