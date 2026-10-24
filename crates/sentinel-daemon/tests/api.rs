//! The HTTP API, served on a real listener and called with `reqwest`. The WebSocket framing is
//! tested with unit tests in `src/ws.rs`; the socket itself is not exercised here.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::future::BoxFuture;
use reqwest::StatusCode;
use reqwest::header::{AUTHORIZATION, ORIGIN};
use sentinel_core::api::TailscaleStatus;
use sentinel_core::snapshot::Snapshot;
use sentinel_core::state::TailscaleState;
use sentinel_daemon::cmd::{CmdOutput, CommandRunner, SystemRunner};
use sentinel_daemon::engine::{Engine, EngineOptions, FakeAgents};
use sentinel_daemon::secrets::MemoryStore;

mod common;
use common::{copy_dir, git, repo_root};
use sentinel_daemon::server::{self, ServerOptions};
use sentinel_daemon::tailscale::{self, BindError, BindMode};
use serde_json::{Value, json};
use tokio::net::TcpListener;

const RUNNING_STATUS: &str =
    r#"{"BackendState":"Running","Self":{"TailscaleIPs":["100.64.0.5"],"HostName":"test-pc"}}"#;

/// Answers `tailscale` as a running tailnet and runs every other program for real.
struct TailnetRunner;

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

fn engine_in(repo: &Path) -> Engine {
    let fixtures = repo_root().join("fixtures/fake-cli");
    let engine = Engine::open(EngineOptions {
        repo_root: repo.to_path_buf(),
        home: repo.to_path_buf(),
        runner: Arc::new(TailnetRunner),
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
            c.task_timeout_seconds = 5;
            c.max_parallel = 3;
        })
        .unwrap();
    engine
}

/// Serves the router on a free loopback port and returns its base URL.
async fn start(engine: Engine) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, server::router(engine)).await;
    });
    format!("http://{addr}")
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

/// A desktop token for the engine, the way the CLI gets one.
fn desktop_token(engine: &Engine) -> String {
    engine.ensure_desktop_token().unwrap()
}

async fn get_json(base: &str, path: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new().get(format!("{base}{path}"));
    if let Some(token) = token {
        request = request.header(AUTHORIZATION, bearer(token));
    }
    let response = request.send().await.unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

async fn send_json(
    base: &str,
    (method, path): (reqwest::Method, &str),
    token: &str,
    body: &Value,
) -> (StatusCode, Value) {
    let response = reqwest::Client::new()
        .request(method, format!("{base}{path}"))
        .header(AUTHORIZATION, bearer(token))
        .json(body)
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

async fn post_json(base: &str, path: &str, token: &str, body: &Value) -> (StatusCode, Value) {
    send_json(base, (reqwest::Method::POST, path), token, body).await
}

#[tokio::test]
async fn a_request_without_a_token_gets_401_on_state() {
    let dir = tempfile::tempdir().unwrap();
    let base = start(engine_in(dir.path())).await;
    let (status, body) = get_json(&base, "/api/state", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body["error"].is_string(), "{body}");
}

#[tokio::test]
async fn a_wrong_token_gets_401_on_state() {
    let dir = tempfile::tempdir().unwrap();
    let base = start(engine_in(dir.path())).await;
    let (status, _) = get_json(&base, "/api/state", Some("not-a-real-token")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn health_answers_without_a_token() {
    let dir = tempfile::tempdir().unwrap();
    let base = start(engine_in(dir.path())).await;
    let (status, body) = get_json(&base, "/api/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "status": "ok" }));
}

#[tokio::test]
async fn the_desktop_token_reads_a_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let token = desktop_token(&engine);
    let base = start(engine).await;
    let response = reqwest::Client::new()
        .get(format!("{base}/api/state"))
        .header(AUTHORIZATION, bearer(&token))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let snapshot: Snapshot = response.json().await.unwrap();
    assert_eq!(snapshot.tasks, Vec::new());
}

#[tokio::test]
async fn a_changing_request_from_another_origin_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let token = desktop_token(&engine);
    let base = start(engine).await;
    let response = reqwest::Client::new()
        .post(format!("{base}/api/message"))
        .header(AUTHORIZATION, bearer(&token))
        .header(ORIGIN, "http://evil.example")
        .json(&json!({ "text": "hello" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_changing_request_from_the_same_origin_passes() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let token = desktop_token(&engine);
    let base = start(engine).await;
    let origin = base.clone();
    let response = reqwest::Client::new()
        .post(format!("{base}/api/message"))
        .header(AUTHORIZATION, bearer(&token))
        .header(ORIGIN, origin)
        .json(&json!({ "text": "hello" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// Pair a phone through the real routes and return its token.
async fn pair_phone(base: &str, desktop: &str) -> String {
    let (status, started) = post_json(base, "/api/pair/start", desktop, &json!({})).await;
    assert_eq!(status, StatusCode::OK, "{started}");
    let code = started["code"].as_str().unwrap().to_owned();
    let (_, submitted) = post_json(
        base,
        "/api/pair",
        "",
        &json!({ "code": code, "device_name": "Blair phone" }),
    )
    .await;
    let id = submitted["id"].as_str().unwrap().to_owned();
    let (approved, _) = post_json(
        base,
        &format!("/api/pair/{id}/approve"),
        desktop,
        &json!({}),
    )
    .await;
    assert_eq!(approved, StatusCode::OK);
    let (_, outcome) = get_json(base, &format!("/api/pair/{id}/outcome"), None).await;
    assert_eq!(outcome["state"], "approved", "{outcome}");
    outcome["token"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn a_paired_phone_reads_state_but_cannot_use_desktop_routes() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let desktop = desktop_token(&engine);
    let base = start(engine).await;
    let phone = pair_phone(&base, &desktop).await;

    let (read, _) = get_json(&base, "/api/state", Some(&phone)).await;
    assert_eq!(read, StatusCode::OK);
    let (pick, _) = post_json(&base, "/api/fs/pick", &phone, &json!({})).await;
    assert_eq!(pick, StatusCode::FORBIDDEN);
    let (start_again, _) = post_json(&base, "/api/pair/start", &phone, &json!({})).await;
    assert_eq!(start_again, StatusCode::FORBIDDEN);
    let (scan, _) = post_json(&base, "/api/tools/scan", &phone, &json!({})).await;
    assert_eq!(scan, StatusCode::FORBIDDEN);
    // The folder browser is for phones, so it is not refused for who is asking.
    let (browse, _) = get_json(&base, "/api/fs/list", Some(&phone)).await;
    assert_ne!(browse, StatusCode::FORBIDDEN);
    let (routing, _) = send_json(
        &base,
        (reqwest::Method::PUT, "/api/routing"),
        &phone,
        &json!({ "routing": {} }),
    )
    .await;
    assert_eq!(routing, StatusCode::FORBIDDEN);
    let (bypass, _) = send_json(
        &base,
        (reqwest::Method::PUT, "/api/permissions"),
        &phone,
        &json!({ "mode": "bypass" }),
    )
    .await;
    assert_eq!(bypass, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_cap_can_be_lowered_but_never_raised() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let token = desktop_token(&engine);
    let config_engine = engine.clone();
    let base = start(engine).await;

    let (raise, _) = post_json(&base, "/api/cap/lower", &token, &json!({ "cap": "9.00" })).await;
    assert_eq!(raise, StatusCode::FORBIDDEN);
    assert_eq!(config_engine.config().cap_usd, "5.00");

    let (lower, body) = post_json(&base, "/api/cap/lower", &token, &json!({ "cap": "1.50" })).await;
    assert_eq!(lower, StatusCode::OK);
    assert_eq!(body, json!({ "cap": "1.50" }));

    let (missing, _) = post_json(&base, "/api/cap/raise", &token, &json!({ "cap": "9.00" })).await;
    assert_eq!(missing, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_body_over_one_megabyte_gets_413() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let token = desktop_token(&engine);
    let base = start(engine).await;
    let big = "a".repeat(1024 * 1024 + 16);
    let (status, _) = post_json(&base, "/api/message", &token, &json!({ "text": big })).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn an_unknown_api_path_gets_a_json_404() {
    let dir = tempfile::tempdir().unwrap();
    let base = start(engine_in(dir.path())).await;
    let (status, body) = get_json(&base, "/api/nope", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].is_string(), "{body}");
}

#[tokio::test]
async fn an_unknown_page_gets_the_ui_index() {
    let dir = tempfile::tempdir().unwrap();
    let base = start(engine_in(dir.path())).await;
    let response = reqwest::get(format!("{base}/nope")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let text = response.text().await.unwrap();
    // Either the placeholder or the real build: both are one HTML page.
    assert!(text.to_lowercase().contains("<html"), "{text}");
}

#[test]
fn the_tailnet_refuses_the_wildcard_addresses() {
    for address in ["0.0.0.0", "::"] {
        let status = TailscaleStatus {
            state: TailscaleState::Running,
            address: Some(address.to_owned()),
            machine_name: None,
        };
        assert_eq!(
            tailscale::bind_address(&status, BindMode::Tailscale),
            Err(BindError::Refused),
            "{address}"
        );
    }
}

#[tokio::test]
async fn bind_uses_loopback_in_dev_mode() {
    let dir = tempfile::tempdir().unwrap();
    let options = ServerOptions {
        port: 0,
        bind: BindMode::DevLoopback,
    };
    let bound = server::bind(engine_in(dir.path()), &options).await.unwrap();
    assert!(bound.addr.ip().is_loopback());
    assert_ne!(bound.addr.port(), 0);
}

/// A temp copy of the sample repo with a two-task backlog, committed and clean.
fn two_task_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&repo_root().join("fixtures/sample-repo"), dir.path());
    let backlog = "# fee-ledger backlog\n\n\
        ## T-01 Add a health check function\n\
        A `health()` function returns `ok`. Done when a test proves it.\n\n\
        ## T-02 Reject negative amounts\n\
        Posting a negative amount returns an error and changes nothing. Done when a test proves it.\n";
    fs::write(dir.path().join("backlog.md"), backlog).unwrap();
    git(dir.path(), &["init", "-b", "main"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-m", "start"]);
    dir
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_run_started_over_the_api_finishes_with_a_report() {
    let dir = two_task_repo();
    let engine = engine_in(dir.path());
    let token = desktop_token(&engine);
    let base = start(engine).await;

    let (scan, _) = post_json(&base, "/api/tools/scan", &token, &json!({})).await;
    assert_eq!(scan, StatusCode::OK);
    let (started, body) = post_json(&base, "/api/run", &token, &json!({})).await;
    assert_eq!(started, StatusCode::OK, "{body}");

    let deadline = Instant::now() + Duration::from_secs(240);
    loop {
        let (_, state) = get_json(&base, "/api/state", Some(&token)).await;
        if state["harness"] == "done" {
            break;
        }
        assert!(Instant::now() < deadline, "the run did not finish: {state}");
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    let (report_status, report) = get_json(&base, "/api/report", Some(&token)).await;
    assert_eq!(report_status, StatusCode::OK);
    assert!(
        report["markdown"]
            .as_str()
            .unwrap_or_default()
            .contains("Verified"),
        "{report}"
    );
}

#[tokio::test]
async fn the_cap_can_be_set_before_a_run_only_from_the_computer() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let desktop = desktop_token(&engine);
    let base = start(engine).await;
    let phone = pair_phone(&base, &desktop).await;

    let (status, set) = send_json(
        &base,
        (reqwest::Method::PUT, "/api/config/cap"),
        &desktop,
        &json!({ "cap": "7.50" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{set}");
    assert_eq!(set["cap"], "7.50");
    let (from_phone, _) = send_json(
        &base,
        (reqwest::Method::PUT, "/api/config/cap"),
        &phone,
        &json!({ "cap": "9.00" }),
    )
    .await;
    assert_eq!(from_phone, StatusCode::FORBIDDEN);
    let (zero, _) = send_json(
        &base,
        (reqwest::Method::PUT, "/api/config/cap"),
        &desktop,
        &json!({ "cap": "0" }),
    )
    .await;
    assert_eq!(zero, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_file_list_names_files_git_knows_and_hides_the_sentinel_folder() {
    let dir = tempfile::tempdir().unwrap();
    let status = std::process::Command::new("git")
        .current_dir(dir.path())
        .args(["init", "-q"])
        .status()
        .unwrap();
    assert!(status.success());
    std::fs::write(dir.path().join("notes.txt"), "hello").unwrap();
    let engine = engine_in(dir.path());
    let token = desktop_token(&engine);
    let base = start(engine).await;

    let (code, files) = get_json(&base, "/api/files", Some(&token)).await;
    assert_eq!(code, StatusCode::OK, "{files}");
    let names: Vec<&str> = files
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|name| name.as_str())
        .collect();
    assert!(names.contains(&"notes.txt"), "{names:?}");
    assert!(names.iter().all(|name| !name.starts_with(".sentinel/")));
}

#[tokio::test]
async fn a_connection_test_answers_for_a_known_name_and_404_for_another() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let token = desktop_token(&engine);
    let base = start(engine).await;

    let (known, row) = post_json(&base, "/api/connections/git/test", &token, &json!({})).await;
    assert_eq!(known, StatusCode::OK, "{row}");
    assert_eq!(
        row["name"].as_str().map(str::to_lowercase),
        Some("git".to_owned())
    );
    let (missing, _) = post_json(&base, "/api/connections/nope/test", &token, &json!({})).await;
    assert_eq!(missing, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_openrouter_key_is_stored_and_never_echoed() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let token = desktop_token(&engine);
    let base = start(engine).await;

    let (status, reply) = send_json(
        &base,
        (reqwest::Method::PUT, "/api/secrets/openrouter"),
        &token,
        &json!({ "key": "sk-test-key" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    assert!(!reply.to_string().contains("sk-test-key"));
    let (empty, _) = send_json(
        &base,
        (reqwest::Method::PUT, "/api/secrets/openrouter"),
        &token,
        &json!({ "key": "  " }),
    )
    .await;
    assert_eq!(empty, StatusCode::BAD_REQUEST);
}
