//! Clearing data by scope through the hub: a phone is refused, the desktop clears chats and the
//! Brain, a refused scope list changes nothing, and "all" ends with the setup wizard not done,
//! one empty chat, no paired phone, and the desktop still signed in.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

mod common;

use std::fs;
use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{Method, Request, StatusCode};
use sentinel_core::api::DataScope;
use sentinel_core::routines::{NewRoutine, RoutineKind};
use sentinel_core::{BrainMode, DeviceId, DeviceKind, HarnessState};
use sentinel_daemon::cmd::SystemRunner;
use sentinel_daemon::config::Config;
use sentinel_daemon::controls::Caller;
use sentinel_daemon::engine::{Engine, EngineOptions};
use sentinel_daemon::hashing::{hash_token, new_token};
use sentinel_daemon::hub::{Hub, hub_router};
use sentinel_daemon::secrets::{MemoryStore, SecretStore};
use sentinel_daemon::telegram::TOKEN_KEY;
use sentinel_store::Device;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use common::{fake_engine, sample_repo};

/// The hub over one engine, as the app runs it.
fn app(engine: &Engine) -> Router {
    hub_router(Hub::new(engine.clone(), None))
}

/// Sends one POST with an optional bearer token and returns the status and the JSON body.
async fn post(
    router: &Router,
    path: &str,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header(CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    let request = request.body(Body::from(body.to_string())).unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

/// Pairs a phone by writing its device row, and returns its token.
fn add_phone(engine: &Engine) -> String {
    let token = new_token();
    engine
        .store()
        .insert_device(&Device {
            id: DeviceId::new("phone-under-test"),
            name: "Test phone".to_owned(),
            kind: DeviceKind::Phone,
            token_hash: hash_token(&token),
            created_at: engine.now(),
            revoked_at: None,
        })
        .unwrap();
    token
}

/// A project with a desktop token, one named chat, and an opened Brain. The Brain stays read only,
/// so no background seed writes notes while the test clears them.
fn project_with_data() -> (TempDir, Engine, String) {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    let desktop = engine.ensure_desktop_token().unwrap();
    engine
        .create_chat(Some("Rate limit"), &Caller::pc("You"))
        .unwrap();
    engine
        .update_config(|config| config.brain_mode = BrainMode::ReadOnly)
        .unwrap();
    engine.brain().unwrap();
    (repo, engine, desktop)
}

#[tokio::test]
async fn a_phone_is_refused_by_every_clear_route() {
    let (_repo, engine, _desktop) = project_with_data();
    let phone = add_phone(&engine);
    let router = app(&engine);
    for path in ["/api/data/project", "/api/data/app", "/api/data/all"] {
        let body = json!({ "scopes": ["chats"] });
        let (status, _) = post(&router, path, Some(&phone), body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path} let a phone through");
    }
}

#[tokio::test]
async fn the_desktop_clears_chats_and_the_brain_of_the_project() {
    let (repo, engine, desktop) = project_with_data();
    let router = app(&engine);
    assert!(repo.path().join(".sentinel/brain").is_dir());

    let body = json!({ "scopes": ["chats", "brain"] });
    let (status, reply) = post(&router, "/api/data/project", Some(&desktop), body).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(reply, json!({ "ok": true }));
    let chats = engine.list_chats().unwrap();
    assert_eq!(chats.len(), 1, "one empty chat should remain");
    assert_ne!(chats[0].title, "Rate limit");
    assert!(!repo.path().join(".sentinel/brain").exists());
}

#[tokio::test]
async fn a_scope_list_with_an_app_scope_changes_nothing() {
    let (repo, engine, desktop) = project_with_data();
    let router = app(&engine);

    let body = json!({ "scopes": ["brain", "keys"] });
    let (status, _) = post(&router, "/api/data/project", Some(&desktop), body).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(repo.path().join(".sentinel/brain").is_dir());
    assert!(
        engine
            .list_chats()
            .unwrap()
            .iter()
            .any(|c| c.title == "Rate limit")
    );
}

#[tokio::test]
async fn clearing_all_ends_with_one_empty_chat_no_phone_and_the_desktop_signed_in() {
    let (_repo, engine, desktop) = project_with_data();
    let phone = add_phone(&engine);
    engine.onboarding_complete().unwrap();
    let router = app(&engine);

    let (status, reply) = post(&router, "/api/data/all", Some(&desktop), json!({})).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(reply, json!({ "ok": true }));
    let chats = engine.list_chats().unwrap();
    assert_eq!(chats.len(), 1);
    assert_ne!(chats[0].title, "Rate limit");
    assert!(!engine.onboarding().unwrap().complete);
    let paired = engine
        .store()
        .list_devices()
        .unwrap()
        .into_iter()
        .filter(|d| d.kind != DeviceKind::Desktop && d.revoked_at.is_none())
        .count();
    assert_eq!(paired, 0);
    assert!(engine.authenticate(&phone).is_none());
    assert!(engine.authenticate(&desktop).is_some());
    let tools = engine.store().list_tools().unwrap();
    assert!(tools.iter().any(|tool| tool.status.is_ready()));
}

#[test]
fn clearing_history_removes_the_report_and_the_live_run_mark() {
    let (repo, engine, _desktop) = project_with_data();
    engine
        .store()
        .set_setting("run.live", "2023-11-14T22:13:20Z")
        .unwrap();
    let report = repo.path().join(".sentinel/report.md");
    fs::write(&report, "an old report").unwrap();

    engine.clear_project_data(&[DataScope::History]).unwrap();

    assert!(!report.exists());
    assert_eq!(engine.store().get_setting("run.live").unwrap(), None);
    assert_eq!(engine.harness_state(), HarnessState::Idle);
    assert!(
        engine
            .list_chats()
            .unwrap()
            .iter()
            .any(|c| c.title == "Rate limit")
    );
}

#[test]
fn clearing_settings_restores_the_defaults_and_keeps_the_repo_folder() {
    let (repo, engine, _desktop) = project_with_data();
    let folder = repo.path().display().to_string();
    engine
        .update_config(|config| {
            config.repo_path = Some(folder.clone());
            config.backlog = "elsewhere.md".to_owned();
        })
        .unwrap();
    engine
        .store()
        .set_setting("tools.defaults_applied", "1")
        .unwrap();

    engine.clear_project_data(&[DataScope::Settings]).unwrap();

    let config = engine.config();
    assert_eq!(config.backlog, Config::default().backlog);
    assert_eq!(config.repo_path, Some(folder));
    assert_eq!(
        engine
            .store()
            .get_setting("tools.defaults_applied")
            .unwrap(),
        None
    );
    assert!(repo.path().join(".sentinel/config.toml").is_file());
}

#[test]
fn clearing_devices_revokes_the_phone_and_keeps_this_computer() {
    let (_repo, engine, desktop) = project_with_data();
    let phone = add_phone(&engine);

    engine.clear_app_data(&[DataScope::Devices]).unwrap();

    assert!(engine.authenticate(&phone).is_none());
    assert!(engine.authenticate(&desktop).is_some());
}

#[test]
fn clearing_keys_removes_the_openrouter_key_and_the_telegram_token() {
    let repo = sample_repo();
    let secrets = Arc::new(MemoryStore::new());
    let engine = engine_with_secrets(repo.path(), Arc::clone(&secrets));
    engine.set_openrouter_key("sk-or-test-key").unwrap();
    secrets.set(TOKEN_KEY, "123:test-token").unwrap();

    engine.clear_app_data(&[DataScope::Keys]).unwrap();

    assert_eq!(secrets.get("openrouter").unwrap(), None);
    assert_eq!(secrets.get(TOKEN_KEY).unwrap(), None);
}

#[test]
fn clearing_telegram_turns_it_off_and_forgets_the_pairing() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    engine
        .update_config(|config| config.telegram.enabled = true)
        .unwrap();
    engine
        .store()
        .set_setting("telegram.paired", "true")
        .unwrap();

    engine.clear_app_data(&[DataScope::Telegram]).unwrap();

    assert!(!engine.config().telegram.enabled);
    assert_eq!(engine.store().get_setting("telegram.paired").unwrap(), None);
}

#[test]
fn clearing_routines_removes_every_routine_from_the_store() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    let standup = NewRoutine {
        name: "Standup".to_owned(),
        kind: RoutineKind::Reminder,
        minute_of_day: 540,
        days: 31,
        to_telegram: false,
        text: "Plan".to_owned(),
        backlog: String::new(),
    };
    engine.create_routine(&standup, &Caller::pc("You")).unwrap();

    engine.clear_app_data(&[DataScope::Routines]).unwrap();

    let left = engine.store().list_routines().unwrap();
    assert_eq!(left.len(), 0, "a routine survived the clear");
}

#[test]
fn clearing_onboarding_starts_the_setup_wizard_again() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    engine.onboarding_complete().unwrap();

    engine.clear_app_data(&[DataScope::Onboarding]).unwrap();

    assert!(!engine.onboarding().unwrap().complete);
}

#[test]
fn a_refused_app_call_removes_nothing_in_its_list() {
    let (_repo, engine, _desktop) = project_with_data();
    let phone = add_phone(&engine);

    let refused = engine.clear_app_data(&[DataScope::Devices, DataScope::Brain]);

    assert!(refused.is_err());
    assert!(engine.authenticate(&phone).is_some());
}

/// An engine that keeps its secrets in a store the test can read back.
fn engine_with_secrets(repo: &Path, secrets: Arc<MemoryStore>) -> Engine {
    Engine::open(EngineOptions {
        repo_root: repo.to_path_buf(),
        home: repo.to_path_buf(),
        runner: Arc::new(SystemRunner),
        secrets,
        fake_agents: None,
        in_memory: true,
    })
    .unwrap()
}
