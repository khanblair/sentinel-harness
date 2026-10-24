//! A whole run with the fake agents on the sample repository. It proves the harness end to end:
//! reserved files, waves, the write guard, checkpoints, the barrier, verdicts, and the report.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sentinel_core::api::RunRequest;
use sentinel_core::{Event, HarnessState, TaskId, TaskStatus};
use sentinel_daemon::cmd::SystemRunner;
use sentinel_daemon::engine::{Engine, EngineOptions, FakeAgents};
use sentinel_daemon::secrets::MemoryStore;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A temp copy of the sample repository with its backlog, committed and clean.
fn sample_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let fixtures = repo_root().join("fixtures");
    copy_dir(&fixtures.join("sample-repo"), dir.path());
    fs::copy(
        fixtures.join("sample-backlog.md"),
        dir.path().join("backlog.md"),
    )
    .unwrap();
    git(dir.path(), &["init", "-b", "main"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-m", "start"]);
    dir
}

fn engine_for(repo: &Path) -> Engine {
    let fixtures = repo_root().join("fixtures/fake-cli");
    let engine = Engine::open(EngineOptions {
        repo_root: repo.to_path_buf(),
        home: repo.to_path_buf(),
        runner: Arc::new(SystemRunner),
        secrets: Arc::new(MemoryStore::new()),
        fake_agents: Some(FakeAgents {
            script: fixtures.join("fake-agent"),
            plans_dir: fixtures.join("plans"),
        }),
        in_memory: true,
    })
    .unwrap();
    engine
        .update_config(|c| {
            "5.00".clone_into(&mut c.cap_usd);
            c.decision_timeout_seconds = 2;
            c.approval_timeout_seconds = 2;
            c.task_timeout_seconds = 5;
            c.max_parallel = 3;
        })
        .unwrap();
    engine
}

async fn wait_done(engine: &Engine) {
    let start = Instant::now();
    while engine.harness_state() != HarnessState::Done {
        assert!(
            start.elapsed() < Duration::from_secs(240),
            "the run did not finish: {:?}",
            engine.harness_state()
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

fn status_of(engine: &Engine, id: &str) -> TaskStatus {
    engine
        .store()
        .get_task(&TaskId::new(id))
        .unwrap()
        .unwrap()
        .status
}

/// The status each task of the sample backlog must end with, and why.
fn assert_sample_statuses(engine: &Engine) {
    for (id, expected, why) in [
        ("T-01", TaskStatus::Verified, "real work"),
        ("T-02", TaskStatus::Verified, "real work"),
        ("T-04", TaskStatus::Verified, "real work"),
        ("T-06", TaskStatus::Skipped, "claims done"),
        (
            "T-03",
            TaskStatus::Contradiction,
            "unanswered contradiction",
        ),
        (
            "T-07",
            TaskStatus::Contradiction,
            "unanswered contradiction",
        ),
        (
            "T-05",
            TaskStatus::Partial,
            "wrote an unreserved file, no new test",
        ),
        ("T-09", TaskStatus::Blocked, "failing agent"),
        ("T-10", TaskStatus::Blocked, "edited an existing test"),
        ("T-08", TaskStatus::Blocked, "too slow"),
    ] {
        assert_eq!(status_of(engine, id), expected, "{id}: {why}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_full_run_on_the_sample_repo_ends_with_honest_statuses() {
    let dir = sample_repo();
    let engine = engine_for(dir.path());
    engine.scan_tools().await.unwrap();
    engine.start_run(RunRequest::default()).await.unwrap();
    wait_done(&engine).await;

    assert_sample_statuses(&engine);

    let events = engine.events_after(0, 100_000).unwrap();
    assert!(
        events
            .iter()
            .any(|r| matches!(r.event, Event::WriteBlocked { .. })),
        "the guard blocked a write"
    );
    assert!(
        events
            .iter()
            .any(|r| matches!(r.event, Event::BarrierEnd { passed: true, .. }))
    );
    assert!(events.iter().any(|r| matches!(r.event, Event::ReportReady)));

    let report = fs::read_to_string(dir.path().join(".sentinel/report.md")).unwrap();
    assert!(report.contains("Verified"), "{report}");
    assert!(report.contains("T-09"), "{report}");
}
