//! The `OpenRouter` worker offered as a tool in a real-mode engine: its entry after a scan, and a
//! one-task run routed to it against a fake API. The key must never reach an event or a tool row.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use std::fs;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use sentinel_core::api::RunRequest;
use sentinel_core::{Event, EventRecord, Routing, TaskId, ToolId, ToolInfo, ToolStatus};
use sentinel_daemon::cmd::{CmdOutput, CommandRunner};
use sentinel_daemon::engine::{Engine, EngineOptions};
use sentinel_daemon::secrets::MemoryStore;
use serde_json::{Value, json};
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

mod common;
use common::{sample_repo_with, wait_done};

const KEY: &str = "sk-or-daemon-test-0123456789";
const CHAT_PATH: &str = "/api/v1/chat/completions";
const PLANNER_MARKER: &str = "task planner for Sentinel";
const WORKER_MARKER: &str = "You are one of several coding agents";
const BACKLOG: &str = "## T-01 Write the hello note\n\
Write notes/hello.txt with one line of text. Done when the file exists.\n";

/// Every program is missing, so the scan finds no CLI this test must not depend on.
struct MissingRunner;

impl CommandRunner for MissingRunner {
    fn run<'a>(
        &'a self,
        _program: &'a str,
        _args: &'a [&'a str],
    ) -> Pin<Box<dyn Future<Output = CmdOutput> + Send + 'a>> {
        Box::pin(async { CmdOutput::never_started() })
    }
}

fn real_engine(repo: &std::path::Path) -> Engine {
    Engine::open(EngineOptions {
        repo_root: repo.to_path_buf(),
        home: repo.to_path_buf(),
        runner: Arc::new(MissingRunner),
        secrets: Arc::new(MemoryStore::new()),
        fake_agents: None,
        in_memory: true,
    })
    .unwrap()
}

fn openrouter_row(tools: &[ToolInfo]) -> ToolInfo {
    tools
        .iter()
        .find(|t| t.id == ToolId::new("openrouter"))
        .cloned()
        .expect("the scan lists openrouter")
}

fn reply_text(text: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "choices": [{ "message": { "role": "assistant", "content": text } }],
        "usage": {"total_tokens": 50, "cost": 0.0001},
    }))
}

/// A reply that asks for one tool call and costs half a cent.
fn tool_reply(name: &str, arguments: &Value) -> ResponseTemplate {
    let call = json!({
        "id": format!("call_{name}"),
        "type": "function",
        "function": {"name": name, "arguments": arguments.to_string()},
    });
    let message = json!({"role": "assistant", "content": null, "tool_calls": [call]});
    ResponseTemplate::new(200).set_body_json(json!({
        "choices": [{ "message": message }],
        "usage": {"total_tokens": 120, "cost": 0.0005},
    }))
}

const PLAN: &str = r#"{"tasks":[{"id":"T-01","scope":["notes/hello.txt"],"depends_on":[],"read_after_write":[],"tier":"cheap","already_done":false,"contradicts":[]}]}"#;

/// Planner first, then the worker's two replies, then plain text for anything else (the verifier).
async fn mount_fake_api(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path(CHAT_PATH))
        .and(body_string_contains(PLANNER_MARKER))
        .respond_with(reply_text(PLAN))
        .mount(server)
        .await;
    for (name, arguments) in [
        (
            "write_file",
            json!({"path": "notes/hello.txt", "content": "hello\n"}),
        ),
        ("finish", json!({"summary": "Done."})),
    ] {
        Mock::given(method("POST"))
            .and(path(CHAT_PATH))
            .and(body_string_contains(WORKER_MARKER))
            .respond_with(tool_reply(name, &arguments))
            .up_to_n_times(1)
            .mount(server)
            .await;
    }
    Mock::given(method("POST"))
        .and(path(CHAT_PATH))
        .respond_with(reply_text("Looks fine."))
        .mount(server)
        .await;
}

fn all_events(engine: &Engine) -> Vec<EventRecord> {
    engine.events_after(0, 100_000).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_scan_lists_openrouter_as_needing_login_until_a_key_is_saved() {
    let repo = sample_repo_with(BACKLOG);
    let engine = real_engine(repo.path());

    let before = openrouter_row(&engine.scan_tools().await.unwrap());
    assert_eq!(before.status, ToolStatus::NeedsLogin);
    assert!(!before.enabled);

    engine.set_openrouter_key(KEY).unwrap();
    let after = openrouter_row(&engine.scan_tools().await.unwrap());
    assert_eq!(after.status, ToolStatus::Ready);
    assert_eq!(after.adapter, "openrouter-loop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_key_text_reaches_no_event_and_no_tool_row() {
    let repo = sample_repo_with(BACKLOG);
    let engine = real_engine(repo.path());
    engine.set_openrouter_key(KEY).unwrap();
    let tools = engine.scan_tools().await.unwrap();
    engine
        .toggle_tool(&ToolId::new("openrouter"), true)
        .unwrap();

    let tools_text = serde_json::to_string(&tools).unwrap();
    let events_text = serde_json::to_string(&all_events(&engine)).unwrap();
    assert!(!tools_text.contains(KEY), "tool rows show the key");
    assert!(!events_text.contains(KEY), "events show the key");
}

/// Point the engine at the fake API, save the key, and route every tier to `OpenRouter`.
async fn route_everything_to_openrouter(engine: &Engine, server: &MockServer) {
    engine
        .update_config(|c| {
            c.openrouter_base_url = format!("{}/api/v1", server.uri());
            "5.00".clone_into(&mut c.cap_usd);
            c.test_command = vec!["true".to_owned()];
            c.task_timeout_seconds = 60;
        })
        .unwrap();
    engine.set_openrouter_key(KEY).unwrap();
    engine.scan_tools().await.unwrap();
    let route = ToolId::new("openrouter");
    engine.toggle_tool(&route, true).unwrap();
    engine
        .set_routing(Routing {
            cheap: Some(route.clone()),
            mid: Some(route.clone()),
            smart: Some(route),
        })
        .unwrap();
}

/// The micro-dollar totals of every cost event, in order.
fn cost_totals(engine: &Engine) -> Vec<u64> {
    all_events(engine)
        .iter()
        .filter_map(|record| {
            if let Event::CostUpdate { total, .. } = &record.event {
                Some(total.micro_usd())
            } else {
                None
            }
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_one_task_run_routed_to_openrouter_writes_the_file_and_records_cost() {
    let server = MockServer::start().await;
    mount_fake_api(&server).await;
    let repo = sample_repo_with(BACKLOG);
    let engine = real_engine(repo.path());
    route_everything_to_openrouter(&engine, &server).await;

    let request = RunRequest {
        task: Some(TaskId::new("T-01")),
        ..RunRequest::default()
    };
    engine.start_run(request).await.unwrap();
    wait_done(&engine).await;

    let written = fs::read_to_string(repo.path().join("notes/hello.txt")).unwrap();
    assert_eq!(written, "hello\n");
    let costs = cost_totals(&engine);
    assert!(
        costs.iter().any(|micro| *micro > 0),
        "no cost was recorded: {costs:?}"
    );
    let events_text = serde_json::to_string(&all_events(&engine)).unwrap();
    assert!(!events_text.contains(KEY), "events show the key");
}
