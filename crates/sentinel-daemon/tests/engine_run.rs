//! A whole run with the fake agents on the sample repository. It proves the harness end to end:
//! reserved files, waves, the write guard, checkpoints, the barrier, verdicts, and the report.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use std::fs;
use std::path::Path;
use std::sync::Arc;

use sentinel_core::api::RunRequest;
use sentinel_core::{Event, EventRecord, HarnessState, TaskId, TaskStatus};
use sentinel_daemon::cmd::SystemRunner;
use sentinel_daemon::engine::{Engine, EngineOptions, FakeAgents};
use sentinel_daemon::secrets::MemoryStore;
use tokio::sync::broadcast;

mod common;
use common::{copy_dir, fake_engine, repo_root, sample_repo, wait_done};

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

/// A planner that prints one plan, for a message that asks for work.
const PLAN_FOR_HEALTH: &str = r#"say {"tasks":[{"id":"T-01","scope":["src/health.rs","tests/health.rs"],"depends_on":[],"read_after_write":[],"tier":"cheap","already_done":false,"contradicts":[]}]}"#;

/// A folder of the fixture plans, with `planner` as the script of the fake planner (task `PLAN`).
fn planner_plans(planner: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&repo_root().join("fixtures/fake-cli/plans"), dir.path());
    fs::write(dir.path().join("PLAN.fake"), planner).unwrap();
    dir
}

/// An engine with the scripted agents, reading their plans from `plans`.
fn engine_with_plans(repo: &Path, plans: &Path) -> Engine {
    let fixtures = repo_root().join("fixtures/fake-cli");
    let engine = Engine::open(EngineOptions {
        repo_root: repo.to_path_buf(),
        home: repo.to_path_buf(),
        runner: Arc::new(SystemRunner),
        secrets: Arc::new(MemoryStore::new()),
        fake_agents: Some(FakeAgents {
            script: fixtures.join("fake-agent"),
            plans_dir: plans.to_path_buf(),
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

fn message(text: &str) -> RunRequest {
    RunRequest {
        text: Some(text.to_owned()),
        ..RunRequest::default()
    }
}

/// The next event that `wanted` accepts. It waits on the stream, so the test never polls.
async fn next_event(
    events: &mut broadcast::Receiver<EventRecord>,
    wanted: impl Fn(&Event) -> bool,
) -> Event {
    loop {
        match events.recv().await {
            Ok(record) if wanted(&record.event) => return record.event,
            Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => panic!("the event stream closed"),
        }
    }
}

fn is_sentinel_message(event: &Event) -> bool {
    matches!(event, Event::Message { from, .. } if from == "sentinel")
}

fn is_run_started(event: &Event) -> bool {
    matches!(event, Event::RunStarted { .. })
}

/// The harness has left Running: the run is over, or no run ever started.
fn is_settled(event: &Event) -> bool {
    matches!(event, Event::HarnessState { state } if *state != HarnessState::Running)
}

fn message_text(event: Event) -> String {
    let Event::Message { text, .. } = event else {
        panic!("the event is not a message");
    };
    text
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_greeting_gets_a_reply_and_no_run() {
    let dir = sample_repo();
    let plans = planner_plans("say Hello! What would you like to build?\n");
    let engine = engine_with_plans(dir.path(), plans.path());
    engine.scan_tools().await.unwrap();
    let mut events = engine.subscribe();
    engine.start_run(message("hi")).await.unwrap();

    let reply = message_text(next_event(&mut events, is_sentinel_message).await);
    next_event(&mut events, is_settled).await;

    assert!(
        reply.starts_with("Hello! What would you like to build?"),
        "{reply}"
    );
    assert_eq!(engine.harness_state(), HarnessState::Idle);
    assert_eq!(engine.store().list_tasks().unwrap().len(), 0);
    let history = engine.events_after(0, 100_000).unwrap();
    assert!(
        !history.iter().any(|r| is_run_started(&r.event)),
        "a greeting started a run"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_planner_that_fails_gives_the_could_not_reach_message_and_no_run() {
    let dir = sample_repo();
    let plans = planner_plans("fail 2\n");
    let engine = engine_with_plans(dir.path(), plans.path());
    engine.scan_tools().await.unwrap();
    let mut events = engine.subscribe();
    engine
        .start_run(message("Add a health check"))
        .await
        .unwrap();

    let text = message_text(next_event(&mut events, is_sentinel_message).await);
    next_event(&mut events, is_settled).await;

    assert!(
        text.starts_with("I could not reach a planning agent: "),
        "{text}"
    );
    assert!(
        text.ends_with("Check Settings, Agents and press Test."),
        "{text}"
    );
    assert_eq!(engine.harness_state(), HarnessState::Idle);
    assert_eq!(engine.store().list_tasks().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_usable_plan_from_a_message_starts_a_run() {
    let dir = sample_repo();
    let plans = planner_plans(&format!("{PLAN_FOR_HEALTH}\n"));
    let engine = engine_with_plans(dir.path(), plans.path());
    engine.scan_tools().await.unwrap();
    let mut events = engine.subscribe();
    engine
        .start_run(message("Add a health check function"))
        .await
        .unwrap();

    next_event(&mut events, is_run_started).await;
    wait_done(&engine).await;

    assert!(
        engine
            .store()
            .get_task(&TaskId::new("T-01"))
            .unwrap()
            .is_some(),
        "the planned task was stored"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_backlog_run_without_a_usable_plan_uses_the_offline_planner() {
    let dir = sample_repo();
    let plans = planner_plans("say Hello! What would you like to build?\n");
    let engine = engine_with_plans(dir.path(), plans.path());
    engine.scan_tools().await.unwrap();
    let mut events = engine.subscribe();
    let request = RunRequest {
        backlog: Some("backlog.md".to_owned()),
        ..RunRequest::default()
    };
    engine.start_run(request).await.unwrap();

    next_event(&mut events, |event| {
        matches!(event, Event::Message { text, .. } if text.starts_with("No agent gave a usable plan"))
    })
    .await;
    wait_done(&engine).await;

    assert_ne!(engine.store().list_tasks().unwrap().len(), 0);
}
