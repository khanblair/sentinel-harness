//! The agent test route over HTTP. The answers that need no agent program are checked here: an
//! unknown agent, a missing one, and one with no command line. A real run is covered by the unit
//! tests in `src/agent_test.rs`, which use scripted programs.

mod common;

use std::fmt::Debug;
use std::path::Path;
use std::sync::Arc;

use reqwest::{Client, StatusCode};
use sentinel_core::api::ToolTest;
use sentinel_core::{Tier, ToolId, ToolInfo, ToolStatus};
use sentinel_daemon::engine::Engine;
use sentinel_daemon::server;
use tokio::net::TcpListener;

use common::TailnetRunner;

const MISSING: &str =
    "This agent was not found on this computer. Install it, then press Scan again.";
const NO_COMMAND: &str =
    "Sentinel has no command line for this agent yet. Add one in Settings, Agents.";

/// One daemon on a free port, with its desktop token.
struct Daemon {
    base: String,
    desktop: String,
    engine: Engine,
    _dir: tempfile::TempDir,
}

impl Daemon {
    async fn start() -> Self {
        let dir = must(tempfile::tempdir());
        let engine = engine_in(dir.path());
        let desktop = must(engine.ensure_desktop_token());
        let base = serve(engine.clone()).await;
        Self {
            base,
            desktop,
            engine,
            _dir: dir,
        }
    }

    /// Ask the daemon to test one agent, as the desktop, and return the status and body.
    async fn test_tool(&self, id: &str) -> (StatusCode, serde_json::Value) {
        let response = must(
            Client::new()
                .post(format!("{}/api/tools/{id}/test", self.base))
                .bearer_auth(&self.desktop)
                .send()
                .await,
        );
        let status = response.status();
        let body = must(response.json().await);
        (status, body)
    }

    /// Put one agent in the store with the given status and no command line of its own.
    fn add_tool(&self, id: &str, status: ToolStatus) {
        let tool = ToolInfo {
            id: ToolId::new(id),
            name: format!("Agent {id}"),
            path: String::new(),
            version: String::new(),
            adapter: id.to_owned(),
            status,
            tier: Tier::Cheap,
            enabled: false,
        };
        must(self.engine.store().upsert_tool(&tool));
    }
}

/// The value of a result that a test needs. An error fails the test with its text.
fn must<T, E: Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{error:?}"),
    }
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

#[tokio::test]
async fn an_unknown_agent_answers_not_found() {
    let daemon = Daemon::start().await;
    let (status, body) = daemon.test_tool("nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "No such agent.");
}

#[tokio::test]
async fn a_missing_agent_is_told_to_install_it() {
    let daemon = Daemon::start().await;
    daemon.add_tool("gone", ToolStatus::Missing);
    let (status, body) = daemon.test_tool("gone").await;
    assert_eq!(status, StatusCode::OK);
    let answer: ToolTest = must(serde_json::from_value(body));
    assert_eq!(
        answer,
        ToolTest {
            tool: "gone".to_owned(),
            ok: false,
            message: MISSING.to_owned(),
            millis: 0,
        }
    );
}

#[tokio::test]
async fn an_agent_without_a_command_line_is_told_to_add_one() {
    let daemon = Daemon::start().await;
    daemon.add_tool("bare", ToolStatus::NeedsCommand);
    let (status, body) = daemon.test_tool("bare").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], false);
    assert_eq!(body["message"], NO_COMMAND);
}
