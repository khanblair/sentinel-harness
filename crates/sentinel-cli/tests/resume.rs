//! Crash resume, end to end: start a run, kill the whole process with SIGKILL after the first
//! checkpoint, start the same run again, and check that it finishes with an honest report.
//!
//! The runs are child processes of this test binary. The ignored child test below runs the backlog
//! on the scripted agents, so the product binary never needs a fake-agent flag.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use std::fs;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sentinel_core::api::RunRequest;
use sentinel_daemon::cmd::SystemRunner;
use sentinel_daemon::engine::{Engine, EngineOptions, FakeAgents};
use sentinel_daemon::secrets::MemoryStore;

#[path = "../../sentinel-daemon/tests/common/mod.rs"]
mod common;

/// The environment variable that tells the child test which repository to run.
const CHILD_REPO: &str = "SENTINEL_RESUME_REPO";
const CHILD_TEST: &str = "resume_child_runs_the_backlog";

/// The child run. It is ignored so only the crash test starts it, and it does nothing unless the
/// repository variable is set.
#[test]
#[ignore = "started by the crash test as a child process"]
fn resume_child_runs_the_backlog() {
    let Some(repo) = std::env::var_os(CHILD_REPO) else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(run_backlog(Path::new(&repo)));
}

/// What `sentinel run backlog.md` does, on the scripted agents: open the state on disk, scan the
/// tools, start the run, and wait for it to finish.
async fn run_backlog(repo: &Path) {
    let fixtures = common::repo_root().join("fixtures/fake-cli");
    let engine = Engine::open(EngineOptions {
        repo_root: repo.to_path_buf(),
        home: repo.to_path_buf(),
        runner: Arc::new(SystemRunner),
        secrets: Arc::new(MemoryStore::new()),
        fake_agents: Some(FakeAgents {
            script: fixtures.join("fake-agent"),
            plans_dir: fixtures.join("plans"),
        }),
        in_memory: false,
    })
    .unwrap();
    engine.scan_tools().await.unwrap();
    engine
        .start_run(RunRequest {
            backlog: Some("backlog.md".to_owned()),
            ..RunRequest::default()
        })
        .await
        .unwrap();
    common::wait_done(&engine).await;
}

/// A temp copy of the sample repository with short timeouts for the run (the config is not tracked).
fn sample_repo() -> tempfile::TempDir {
    let dir = common::sample_repo();
    fs::create_dir_all(dir.path().join(".sentinel")).unwrap();
    fs::write(
        dir.path().join(".sentinel/config.toml"),
        "cap_usd = \"5.00\"\ndecision_timeout_seconds = 2\napproval_timeout_seconds = 2\ntask_timeout_seconds = 5\nmax_parallel = 3\n",
    )
    .unwrap();
    dir
}

/// This test binary, run again with only the child test selected.
fn child_run(repo: &Path) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", CHILD_TEST, "--ignored"])
        .env(CHILD_REPO, repo);
    command
}

fn log_text(repo: &Path) -> String {
    fs::read_to_string(repo.join(".sentinel/log.jsonl")).unwrap_or_default()
}

/// Wait until the log has the text, or fail the test.
fn wait_for(repo: &Path, needle: &str) {
    let start = Instant::now();
    while !log_text(repo).contains(needle) {
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "never saw {needle}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_run_killed_after_its_first_checkpoint_finishes_when_started_again() {
    let repo = sample_repo();
    let mut first = child_run(repo.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    wait_for(repo.path(), "\"kind\":\"Checkpoint\"");
    // SIGKILL to the whole group: Sentinel and every agent it started.
    Command::new("kill")
        .args(["-9", &format!("-{}", first.id())])
        .status()
        .unwrap();
    first.wait().unwrap();
    assert!(
        !repo.path().join(".sentinel/report.md").exists(),
        "the first run must not have finished"
    );

    let second = child_run(repo.path()).output().unwrap();
    assert!(
        second.status.success(),
        "the second run must finish:\n{}{}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr)
    );

    assert_finished_honestly(repo.path());
}

/// The report has the verdicts the run must reach, and the log shows one run that finished.
fn assert_finished_honestly(repo: &Path) {
    let report = fs::read_to_string(repo.join(".sentinel/report.md")).unwrap();
    for (id, status) in [
        ("T-01", "Verified"),
        ("T-02", "Verified"),
        ("T-04", "Verified"),
        ("T-06", "Blocked"),
        ("T-09", "Blocked"),
    ] {
        assert!(
            report.contains(&format!("| {id} | {status} |")),
            "{id} should be {status}:\n{report}"
        );
    }
    let log = log_text(repo);
    assert_eq!(log.matches("\"kind\":\"RunStarted\"").count(), 1, "one run");
    assert!(log.contains("Resuming after a stop"), "the stream says so");
    assert_eq!(log.matches("\"kind\":\"RunFinished\"").count(), 1);
}
