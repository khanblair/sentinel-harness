//! Crash resume, end to end: start a run, kill the whole process with SIGKILL after the first
//! checkpoint, start the same command again, and check that the run finishes with an honest report.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[path = "../../sentinel-daemon/tests/common/mod.rs"]
mod common;

fn fixtures() -> PathBuf {
    common::repo_root().join("fixtures")
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

fn sentinel(repo: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sentinel"));
    command
        .args(["run", "backlog.md", "--fake-agents", "--repo"])
        .arg(repo)
        .env("SENTINEL_FIXTURES", fixtures())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
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
    let mut first = sentinel(repo.path()).process_group(0).spawn().unwrap();
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

    let second = sentinel(repo.path())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(
        second.status.success(),
        "the second run must finish: {}",
        String::from_utf8_lossy(&second.stderr)
    );

    let report = fs::read_to_string(repo.path().join(".sentinel/report.md")).unwrap();
    for (id, status) in [
        ("T-01", "Verified"),
        ("T-02", "Verified"),
        ("T-04", "Verified"),
        ("T-06", "Skipped"),
        ("T-09", "Blocked"),
    ] {
        assert!(
            report.contains(&format!("| {id} | {status} |")),
            "{id} should be {status}:\n{report}"
        );
    }
    let log = log_text(repo.path());
    assert_eq!(log.matches("\"kind\":\"RunStarted\"").count(), 1, "one run");
    assert!(log.contains("Resuming after a stop"), "the stream says so");
    assert_eq!(log.matches("\"kind\":\"RunFinished\"").count(), 1);
}
