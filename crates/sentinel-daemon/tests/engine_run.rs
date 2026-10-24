//! A whole run with the fake agents on the sample repository. It proves the harness end to end:
//! reserved files, waves, the write guard, checkpoints, the barrier, verdicts, and the report.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use std::fs;

use sentinel_core::api::RunRequest;
use sentinel_core::{Event, TaskId, TaskStatus};
use sentinel_daemon::engine::Engine;

mod common;
use common::{fake_engine, sample_repo, wait_done};

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
    let engine = fake_engine(dir.path());
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
