//! Helpers for the Brain API tests: an engine on a temporary repository, a listener, requests with a
//! device token, a paired phone, and fixtures written with `sentinel_brain`.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::future::BoxFuture;
use reqwest::{Method, StatusCode, header::AUTHORIZATION};
use sentinel_brain::Vault;
use sentinel_brain::lesson::new_lesson;
use sentinel_brain::note::{Author, Note};
use sentinel_brain::proposal::{Proposal, ProposalState, write_proposal};
use sentinel_brain::replace_generated;
use sentinel_brain::schedule::Date;
use sentinel_core::{NoteId, NoteType, ProposalKind};
use sentinel_daemon::cmd::{CmdOutput, CommandRunner, SystemRunner};
use sentinel_daemon::engine::{Engine, EngineOptions, FakeAgents};
use sentinel_daemon::secrets::MemoryStore;
use sentinel_daemon::server;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::common::repo_root;

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

pub fn engine_in(repo: &Path) -> Engine {
    let fixtures = repo_root().join("fixtures/fake-cli");
    Engine::open(EngineOptions {
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
    .unwrap()
}

/// Serves the router on a free loopback port and returns its base URL.
pub async fn start(engine: Engine) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, server::router(engine)).await;
    });
    format!("http://{addr}")
}

/// One request with a device token. Returns the status and the JSON body (`null` when none).
pub async fn call(
    base: &str,
    token: &str,
    (method, path): (Method, &str),
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new()
        .request(method, format!("{base}{path}"))
        .header(AUTHORIZATION, format!("Bearer {token}"));
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

/// Pair a phone through the real routes and return its token.
pub async fn pair_phone(base: &str, desktop: &str) -> String {
    let started = call(
        base,
        desktop,
        (Method::POST, "/api/pair/start"),
        Some(json!({})),
    )
    .await
    .1;
    let code = started["code"].as_str().unwrap().to_owned();
    let submitted = reqwest::Client::new()
        .post(format!("{base}/api/pair"))
        .json(&json!({ "code": code, "device_name": "Blair phone" }))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let id = submitted["id"].as_str().unwrap().to_owned();
    let approved = call(
        base,
        desktop,
        (Method::POST, &format!("/api/pair/{id}/approve")),
        Some(json!({})),
    )
    .await;
    assert_eq!(approved.0, StatusCode::OK);
    let outcome = reqwest::get(format!("{base}/api/pair/{id}/outcome"))
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(outcome["state"], "approved", "{outcome}");
    outcome["token"].as_str().unwrap().to_owned()
}

pub fn vault_root(dir: &Path) -> PathBuf {
    dir.join(".sentinel/brain")
}

pub fn seed_lesson(dir: &Path) {
    let vault = Vault::open(vault_root(dir)).unwrap();
    let date = Date::new(2026, 1, 1).unwrap();
    vault
        .write(
            &new_lesson("convention", "tests", "run the tests first", date),
            None,
        )
        .unwrap();
}

/// A note that Sentinel wrote long ago, at the strength floor, so a care run proposes to retire it.
pub fn seed_old_run_note(dir: &Path) {
    let vault = Vault::open(vault_root(dir)).unwrap();
    let note = Note {
        id: NoteId::new("old-run"),
        note_type: NoteType::Run,
        title: "Old run".to_owned(),
        tags: Vec::new(),
        by: Author::Run,
        strength: 0.05,
        confirmed: Some("2025-01-01".to_owned()),
        pinned: false,
        lesson: None,
        body: String::new(),
        extra: Vec::new(),
    };
    vault.write(&note, None).unwrap();
}

/// An open update proposal that replaces the generated part of `target`.
pub fn seed_update_proposal(dir: &Path, id: &str, target: &str, text: &str) {
    let proposed = Note {
        id: NoteId::new(target),
        note_type: NoteType::Convention,
        title: target.to_owned(),
        tags: Vec::new(),
        by: Author::Care,
        strength: 0.5,
        confirmed: None,
        pinned: false,
        lesson: None,
        body: replace_generated("", text),
        extra: Vec::new(),
    };
    let proposal = Proposal {
        id: NoteId::new(id),
        kind: ProposalKind::Update,
        target: Some(NoteId::new(target)),
        why: "Adds a suggestion.".to_owned(),
        note: Some(proposed),
        state: ProposalState::Open,
    };
    write_proposal(&vault_root(dir), &proposal).unwrap();
}

pub async fn post_note(base: &str, token: &str, title: &str, body: &str) -> Value {
    let (status, detail) = call(
        base,
        token,
        (Method::POST, "/api/brain/notes"),
        Some(json!({ "note_type": "convention", "title": title, "body": body })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    detail
}

pub async fn search(base: &str, token: &str, query: &str) -> Value {
    call(
        base,
        token,
        (Method::GET, &format!("/api/brain/notes?q={query}")),
        None,
    )
    .await
    .1
}

pub fn ids(list: &Value) -> Vec<String> {
    list["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|note| note["id"].as_str().unwrap().to_owned())
        .collect()
}
