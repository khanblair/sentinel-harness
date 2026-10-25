//! The provider key routes over HTTP: the rows, a save, a delete, the refusals, and a test with no
//! saved key. The live check of a saved key runs against a mock server in `src/keys.rs`.

mod common;

use std::fmt::Debug;
use std::path::Path;
use std::sync::Arc;

use reqwest::{Client, Method, StatusCode};
use sentinel_daemon::engine::Engine;
use sentinel_daemon::server;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use common::TailnetRunner;

/// Shaped like an Anthropic key. Only its last four characters may show.
const SECRET: &str = "sk-ant-test-0123456789abcd";

/// One daemon on a free port, with a desktop token and a paired phone.
struct Daemon {
    base: String,
    desktop: String,
    phone: String,
    _dir: tempfile::TempDir,
}

impl Daemon {
    async fn start() -> Self {
        let dir = must(tempfile::tempdir());
        let engine = engine_in(dir.path());
        let desktop = must(engine.ensure_desktop_token());
        let base = serve(engine).await;
        let phone = pair_phone(&base, &desktop).await;
        Self {
            base,
            desktop,
            phone,
            _dir: dir,
        }
    }
}

/// The value of a result that a test needs. An error fails the test with its text.
fn must<T, E: Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{error:?}"),
    }
}

/// The value of an option that a test needs. A missing value fails the test.
fn present<T>(value: Option<T>) -> T {
    value.unwrap_or_else(|| panic!("a value the test needs is missing"))
}

fn engine_in(repo: &Path) -> Engine {
    let engine = common::engine_with_runner(repo, Arc::new(TailnetRunner));
    engine.set_headless(true);
    engine
}

async fn serve(engine: Engine) -> String {
    let listener = must(TcpListener::bind("127.0.0.1:0").await);
    let addr = must(listener.local_addr());
    tokio::spawn(async move {
        let _ = axum::serve(listener, server::router(engine)).await;
    });
    format!("http://{addr}")
}

async fn pair_phone(base: &str, desktop: &str) -> String {
    let (_, started) = send(
        base,
        Some(desktop),
        (Method::POST, "/api/pair/start"),
        json!({}),
    )
    .await;
    let code = present(started["code"].as_str()).to_owned();
    let (_, submitted) = send(
        base,
        None,
        (Method::POST, "/api/pair"),
        json!({ "code": code, "device_name": "Test phone" }),
    )
    .await;
    let id = present(submitted["id"].as_str()).to_owned();
    send(
        base,
        Some(desktop),
        (Method::POST, &format!("/api/pair/{id}/approve")),
        json!({}),
    )
    .await;
    let (_, outcome) = send(
        base,
        None,
        (Method::GET, &format!("/api/pair/{id}/outcome")),
        Value::Null,
    )
    .await;
    present(outcome["token"].as_str()).to_owned()
}

/// Sends one request. The answer is JSON, or the plain text when the body is not JSON.
async fn send(
    base: &str,
    token: Option<&str>,
    route: (Method, &str),
    body: Value,
) -> (StatusCode, Value) {
    let (method, path) = route;
    let mut request = Client::new().request(method, format!("{base}{path}"));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    if !body.is_null() {
        request = request.json(&body);
    }
    let response = must(request.send().await);
    let status = response.status();
    let text = must(response.text().await);
    (
        status,
        serde_json::from_str(&text).unwrap_or(Value::String(text)),
    )
}

fn row<'a>(rows: &'a Value, provider: &str) -> &'a Value {
    let all = present(rows.as_array());
    present(all.iter().find(|row| row["provider"] == provider))
}

fn key_routes() -> Vec<(Method, &'static str)> {
    vec![
        (Method::GET, "/api/keys"),
        (Method::PUT, "/api/keys/anthropic"),
        (Method::DELETE, "/api/keys/anthropic"),
        (Method::POST, "/api/keys/anthropic/test"),
    ]
}

/// The rows of `GET /api/keys` for the desktop.
async fn list(daemon: &Daemon) -> Value {
    let desktop = Some(daemon.desktop.as_str());
    let (status, rows) = send(
        &daemon.base,
        desktop,
        (Method::GET, "/api/keys"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    rows
}

#[tokio::test]
async fn a_saved_key_shows_only_its_last_four_characters() {
    let daemon = Daemon::start().await;
    let rows = list(&daemon).await;
    assert_eq!(present(rows.as_array()).len(), 4);
    assert_eq!(row(&rows, "anthropic")["has_key"], false);

    let desktop = Some(daemon.desktop.as_str());
    let body = json!({ "key": format!("  {SECRET}  ") });
    let (status, saved) = send(
        &daemon.base,
        desktop,
        (Method::PUT, "/api/keys/anthropic"),
        body,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved, json!({ "ok": true }));

    let rows = list(&daemon).await;
    assert_eq!(row(&rows, "anthropic")["has_key"], true);
    assert_eq!(row(&rows, "anthropic")["hint"], "...abcd");
    assert!(!rows.to_string().contains("0123456789"));
}

#[tokio::test]
async fn deleting_a_saved_key_clears_its_row() {
    let daemon = Daemon::start().await;
    let desktop = Some(daemon.desktop.as_str());
    let body = json!({ "key": SECRET });
    send(
        &daemon.base,
        desktop,
        (Method::PUT, "/api/keys/anthropic"),
        body,
    )
    .await;

    let (status, _) = send(
        &daemon.base,
        desktop,
        (Method::DELETE, "/api/keys/anthropic"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let rows = list(&daemon).await;
    assert_eq!(row(&rows, "anthropic")["has_key"], false);
    assert_eq!(row(&rows, "anthropic")["hint"], Value::Null);
}

#[tokio::test]
async fn a_phone_gets_403_and_no_token_gets_401_on_every_key_route() {
    let daemon = Daemon::start().await;
    let body = json!({ "key": SECRET });
    for (method, path) in key_routes() {
        let label = format!("{method} {path}");
        let (status, _) = send(
            &daemon.base,
            Some(&daemon.phone),
            (method.clone(), path),
            body.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{label}");
        let (status, _) = send(&daemon.base, None, (method, path), body.clone()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{label}");
    }
}

#[tokio::test]
async fn a_key_with_a_space_or_no_text_is_refused_with_400() {
    let daemon = Daemon::start().await;
    let desktop = Some(daemon.desktop.as_str());
    for text in ["", "   ", "sk-ant has a space", "sk-ant\nline"] {
        let body = json!({ "key": text });
        let (status, reply) = send(
            &daemon.base,
            desktop,
            (Method::PUT, "/api/keys/anthropic"),
            body,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{text:?}");
        assert_eq!(reply["error"], "That does not look like a key.");
    }
}

#[tokio::test]
async fn an_unknown_provider_is_404_on_every_key_route_that_names_one() {
    let daemon = Daemon::start().await;
    let desktop = Some(daemon.desktop.as_str());
    let body = json!({ "key": SECRET });
    let (status, _) = send(&daemon.base, desktop, (Method::PUT, "/api/keys/nope"), body).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(
        &daemon.base,
        desktop,
        (Method::DELETE, "/api/keys/nope"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(
        &daemon.base,
        desktop,
        (Method::POST, "/api/keys/nope/test"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn testing_a_provider_with_no_saved_key_is_400_and_says_so() {
    let daemon = Daemon::start().await;
    let desktop = Some(daemon.desktop.as_str());
    let (status, reply) = send(
        &daemon.base,
        desktop,
        (Method::POST, "/api/keys/gemini/test"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(reply["error"], "No key saved yet.");
}
