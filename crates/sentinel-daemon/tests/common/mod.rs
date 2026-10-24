//! Helpers the integration tests share: the repository root, a copy of the sample repository, git
//! commands, and a wait for the end of a run.

#![allow(
    dead_code,
    clippy::unwrap_used,
    reason = "each test file uses some helpers; helpers fail the test by panicking"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use sentinel_core::HarnessState;
use std::sync::Arc;

use futures::future::BoxFuture;
use sentinel_daemon::cmd::{CmdOutput, CommandRunner, SystemRunner};
use sentinel_daemon::engine::{Engine, EngineOptions, FakeAgents};
use sentinel_daemon::secrets::MemoryStore;

pub const RUNNING_STATUS: &str =
    r#"{"BackendState":"Running","Self":{"TailscaleIPs":["100.64.0.5"],"HostName":"test-pc"}}"#;

/// Answers `tailscale` as a running tailnet so pairing works, and runs every other program for real.
pub struct TailnetRunner;

impl CommandRunner for TailnetRunner {
    fn run<'a>(&'a self, program: &'a str, args: &'a [&'a str]) -> BoxFuture<'a, CmdOutput> {
        Box::pin(async move {
            if program == "tailscale" {
                CmdOutput::ok(RUNNING_STATUS)
            } else {
                SystemRunner.run(program, args).await
            }
        })
    }
}

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

pub fn copy_dir(from: &Path, to: &Path) {
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

pub fn git(dir: &Path, args: &[&str]) {
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
pub fn sample_repo() -> tempfile::TempDir {
    let backlog = fs::read_to_string(repo_root().join("fixtures/sample-backlog.md")).unwrap();
    sample_repo_with(&backlog)
}

/// A temp copy of the sample repository with the given backlog, committed and clean.
pub fn sample_repo_with(backlog: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&repo_root().join("fixtures/sample-repo"), dir.path());
    fs::write(dir.path().join("backlog.md"), backlog).unwrap();
    git(dir.path(), &["init", "-b", "main"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-m", "start"]);
    dir
}

pub async fn wait_done(engine: &Engine) {
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

/// An engine with the scripted agents on a repository. `runner` answers the external programs.
pub fn engine_with_runner(repo: &Path, runner: Arc<dyn CommandRunner>) -> Engine {
    let fixtures = repo_root().join("fixtures/fake-cli");
    Engine::open(EngineOptions {
        repo_root: repo.to_path_buf(),
        home: repo.to_path_buf(),
        runner,
        secrets: Arc::new(MemoryStore::new()),
        fake_agents: Some(FakeAgents {
            script: fixtures.join("fake-agent"),
            plans_dir: fixtures.join("plans"),
        }),
        in_memory: true,
    })
    .unwrap()
}

/// An engine with the scripted agents on a repository, with short timeouts and a $5 cap.
pub fn fake_engine(repo: &Path) -> Engine {
    let engine = engine_with_runner(repo, Arc::new(SystemRunner));
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
