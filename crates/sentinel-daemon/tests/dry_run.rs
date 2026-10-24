//! The dry run on the built-in sample project, over the real HTTP routes. The fake agents run the
//! three sample tasks. The user's repository must come out exactly as it went in, and the sample
//! folder must be gone when the dry run ends.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use reqwest::StatusCode;
use reqwest::header::AUTHORIZATION;
use sentinel_core::dry_run::{DryRunStage, StageState};
use sentinel_core::{Event, EventRecord};
use sentinel_daemon::cmd::{CmdOutput, CommandRunner, SystemRunner};
use sentinel_daemon::engine::Engine;
use sentinel_daemon::server;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::broadcast::{self, error::RecvError};

mod common;
use common::{engine_with_runner, fake_engine, sample_repo};

const RUNNING_STATUS: &str =
    r#"{"BackendState":"Running","Self":{"TailscaleIPs":["100.64.0.5"],"HostName":"test-pc"}}"#;
const SAMPLE_PREFIX: &str = "sentinel-dry-run-";

/// Answers `tailscale` as a running tailnet, so a phone can pair. Other programs run for real.
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

/// Serves the router on a free loopback port and returns its base URL.
async fn serve(engine: Engine) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, server::router(engine)).await;
    });
    format!("http://{addr}")
}

async fn send(base: &str, method: reqwest::Method, path: &str, token: &str) -> (StatusCode, Value) {
    let response = reqwest::Client::new()
        .request(method, format!("{base}{path}"))
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

async fn post_empty(base: &str, path: &str, token: &str) -> (StatusCode, Value) {
    send(base, reqwest::Method::POST, path, token).await
}

/// Pair a phone through the real routes and return its token.
async fn pair_phone(base: &str, desktop: &str) -> String {
    let client = reqwest::Client::new();
    let started: Value = client
        .post(format!("{base}/api/pair/start"))
        .header(AUTHORIZATION, format!("Bearer {desktop}"))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let code = started["code"].as_str().unwrap().to_owned();
    let submitted: Value = client
        .post(format!("{base}/api/pair"))
        .json(&json!({ "code": code, "device_name": "Blair phone" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = submitted["id"].as_str().unwrap().to_owned();
    approve_pairing(base, desktop, &id).await
}

/// The desktop approves the pending phone, and the phone's token comes back.
async fn approve_pairing(base: &str, desktop: &str, id: &str) -> String {
    let client = reqwest::Client::new();
    let approved = client
        .post(format!("{base}/api/pair/{id}/approve"))
        .header(AUTHORIZATION, format!("Bearer {desktop}"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(approved.status(), StatusCode::OK);
    let outcome: Value = client
        .get(format!("{base}/api/pair/{id}/outcome"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(outcome["state"], "approved", "{outcome}");
    outcome["token"].as_str().unwrap().to_owned()
}

/// The files of a repository, relative to its root. The `.git` and `.sentinel` folders are left
/// out: the first is git's own state, the second is the daemon's.
fn worktree_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect_files(root, root, &mut found);
    found.sort();
    found
}

fn collect_files(root: &Path, dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name == ".git" || name == ".sentinel" {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, found);
        } else {
            found.push(path.strip_prefix(root).unwrap().to_path_buf());
        }
    }
}

fn git_status(root: &Path) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The sample folders the dry run makes in the system temp folder, by name.
fn sample_folders() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(std::env::temp_dir())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(SAMPLE_PREFIX))
        .collect();
    names.sort();
    names
}

/// Read the main bus until the dry run reports it finished. Returns the stages that reached Done,
/// in the order they did, and the counts. Fails the test if it takes too long.
async fn until_finished(
    events: &mut broadcast::Receiver<EventRecord>,
) -> (Vec<DryRunStage>, u32, u32) {
    let wait = async {
        let mut done = Vec::new();
        loop {
            let record = match events.recv().await {
                Ok(record) => record,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("the event bus closed before the dry run ended"),
            };
            if let Event::DryRunStage {
                stage,
                state: StageState::Done,
            } = record.event
            {
                done.push(stage);
            }
            if let Event::DryRunFinished { verified, total } = record.event {
                return (done, verified, total);
            }
        }
    };
    within_limit(wait).await
}

async fn within_limit<T>(work: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(240), work)
        .await
        .expect("the dry run did not finish in time")
}

#[tokio::test]
async fn a_dry_run_walks_six_stages_and_leaves_the_repository_alone() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    let token = engine.ensure_desktop_token().unwrap();
    let base = serve(engine.clone()).await;
    let mut events = engine.subscribe();
    let files_before = worktree_files(repo.path());
    let status_before = git_status(repo.path());
    let folders_before = sample_folders();

    let (started, body) = post_empty(&base, "/api/dry-run", &token).await;
    assert_eq!(started, StatusCode::ACCEPTED, "{body}");
    assert_eq!(body, json!({ "started": true }));
    let (again, body) = post_empty(&base, "/api/dry-run", &token).await;
    assert_eq!(again, StatusCode::CONFLICT, "{body}");

    let (done, verified, total) = until_finished(&mut events).await;
    assert_eq!(done, DryRunStage::ALL.to_vec());
    assert_eq!((verified, total), (3, 3));

    assert_eq!(worktree_files(repo.path()), files_before);
    assert_eq!(git_status(repo.path()), status_before);
    assert_eq!(sample_folders(), folders_before);

    let (read, status) = send(&base, reqwest::Method::GET, "/api/dry-run", &token).await;
    assert_eq!(read, StatusCode::OK);
    assert_eq!(status["running"], false);
    assert_eq!(status["stages"].as_array().unwrap().len(), 6);
    assert_eq!(status["verified"], 3);
}

#[tokio::test]
async fn a_phone_cannot_start_a_dry_run() {
    let repo = sample_repo();
    let engine = engine_with_runner(repo.path(), Arc::new(TailnetRunner));
    let desktop = engine.ensure_desktop_token().unwrap();
    let base = serve(engine.clone()).await;
    let phone = pair_phone(&base, &desktop).await;

    let (refused, body) = post_empty(&base, "/api/dry-run", &phone).await;
    assert_eq!(refused, StatusCode::FORBIDDEN, "{body}");
}
