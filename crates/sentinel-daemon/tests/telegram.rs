//! Telegram inside the daemon, against a `wiremock` Bot API. The tests drive the real routes with a
//! desktop token, and the real poller and sender run in the background. Waits use bus events and
//! channels fed by the mock, each with a timeout, so no test sleeps.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::{Method, StatusCode, header::AUTHORIZATION};
use sentinel_core::{
    Approval, ApprovalId, ApprovalKind, ApprovalState, ConnectionState, Event, EventRecord,
    PermissionMode,
};
use sentinel_daemon::cmd::SystemRunner;
use sentinel_daemon::engine::{Engine, EngineOptions};
use sentinel_daemon::secrets::{MemoryStore, SecretStore};
use sentinel_daemon::server;
use sentinel_telegram::command::{ParseProblem, parse};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};
use tokio::time::timeout;
use wiremock::matchers::path_regex;
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const TOKEN: &str = "123456789:AAFakeTokenForTestsOnlyxxxxxxxxxxxx";
const OWNER: i64 = 42;
const WAIT: Duration = Duration::from_secs(5);
const ALL_EVENTS: [&str; 9] = [
    "approval_needed",
    "decision_needed",
    "budget_70",
    "budget_95",
    "task_blocked",
    "wave_finished",
    "run_finished",
    "brain_proposals",
    "errors",
];

static NEXT_UPDATE: AtomicI64 = AtomicI64::new(1);

/// Serves the queued updates one per poll. An empty queue answers after a short delay.
struct Updates {
    queue: Arc<Mutex<VecDeque<Value>>>,
    polls: mpsc::UnboundedSender<()>,
}

impl Respond for Updates {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        let _ = self.polls.send(());
        let next = self
            .queue
            .lock()
            .ok()
            .and_then(|mut queue| queue.pop_front());
        match next {
            Some(update) => ResponseTemplate::new(200).set_body_json(json!({
                "ok": true,
                "result": [update],
            })),
            None => ResponseTemplate::new(200)
                .set_body_json(json!({ "ok": true, "result": [] }))
                .set_delay(Duration::from_millis(20)),
        }
    }
}

/// Answers `sendMessage` and `editMessageText`, and passes each request body to the test.
struct Sent {
    bodies: mpsc::UnboundedSender<Value>,
}

impl Respond for Sent {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        let _ = self.bodies.send(body);
        ResponseTemplate::new(200).set_body_json(json!({
            "ok": true,
            "result": { "message_id": 77, "date": 0, "chat": { "id": OWNER, "type": "private" } },
        }))
    }
}

struct Rig {
    engine: Engine,
    secrets: MemoryStore,
    /// Kept alive so the mock keeps answering. Tests never read it.
    _server: MockServer,
    base: String,
    desktop: String,
    updates: Arc<Mutex<VecDeque<Value>>>,
    sent: mpsc::UnboundedReceiver<Value>,
    polls: mpsc::UnboundedReceiver<()>,
    events: broadcast::Receiver<EventRecord>,
    dir: tempfile::TempDir,
}

/// The fake Bot API: `getMe` answers the bot's name, and the other calls go to the channels.
async fn mount_bot_api(server: &MockServer, sent: mpsc::UnboundedSender<Value>, poller: Updates) {
    let getme =
        json!({ "ok": true, "result": { "id": 999, "is_bot": true, "username": "sentinel_bot" } });
    Mock::given(path_regex(r"/bot[^/]+/getMe"))
        .respond_with(ResponseTemplate::new(200).set_body_json(getme))
        .mount(server)
        .await;
    Mock::given(path_regex(r"/bot[^/]+/sendMessage"))
        .respond_with(Sent { bodies: sent })
        .mount(server)
        .await;
    Mock::given(path_regex(r"/bot[^/]+/getUpdates"))
        .respond_with(poller)
        .mount(server)
        .await;
}

async fn rig() -> Rig {
    let server = MockServer::start().await;
    let (sent_tx, sent_rx) = mpsc::unbounded_channel();
    let (poll_tx, poll_rx) = mpsc::unbounded_channel();
    let updates = Arc::new(Mutex::new(VecDeque::new()));
    let poller = Updates {
        queue: Arc::clone(&updates),
        polls: poll_tx,
    };
    mount_bot_api(&server, sent_tx, poller).await;

    let dir = tempfile::tempdir().unwrap();
    let secrets = MemoryStore::new();
    let engine = open_engine(dir.path(), &secrets, server.uri());
    let events = engine.subscribe();
    let base = serve_daemon(engine.clone()).await;
    let desktop = engine.ensure_desktop_token().unwrap();
    Rig {
        engine,
        secrets,
        _server: server,
        base,
        desktop,
        updates,
        sent: sent_rx,
        polls: poll_rx,
        events,
        dir,
    }
}

/// An engine on a temporary folder, with the Bot API pointed at the mock server.
fn open_engine(repo: &std::path::Path, secrets: &MemoryStore, bot_api: String) -> Engine {
    let engine = Engine::open(EngineOptions {
        repo_root: repo.to_path_buf(),
        home: repo.to_path_buf(),
        runner: Arc::new(SystemRunner),
        secrets: Arc::new(secrets.clone()),
        fake_agents: None,
        in_memory: true,
    })
    .unwrap();
    engine
        .update_config(|c| c.telegram.base_url = bot_api)
        .unwrap();
    engine
}

/// Serves the daemon router on a free loopback port and returns its base URL.
async fn serve_daemon(engine: Engine) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, server::router(engine)).await;
    });
    format!("http://{addr}")
}

async fn call(rig: &Rig, method: Method, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new()
        .request(method, format!("{}{path}", rig.base))
        .header(AUTHORIZATION, format!("Bearer {}", rig.desktop));
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

fn update_id() -> i64 {
    NEXT_UPDATE.fetch_add(1, Ordering::SeqCst)
}

fn owner_text(user: i64, text: &str) -> Value {
    json!({
        "update_id": update_id(),
        "message": {
            "message_id": 1,
            "from": { "id": user, "is_bot": false, "first_name": "Owner" },
            "chat": { "id": user, "type": "private" },
            "text": text,
        },
    })
}

fn owner_press(user: i64, data: &str) -> Value {
    let card = json!({
        "message_id": 9,
        "from": { "id": 999, "is_bot": true, "first_name": "Sentinel" },
        "chat": { "id": user, "type": "private" },
        "text": "card",
    });
    let query = json!({
        "id": "callback-1",
        "from": { "id": user, "is_bot": false, "first_name": "Owner" },
        "data": data,
        "message": card,
    });
    json!({ "update_id": update_id(), "callback_query": query })
}

fn push(rig: &Rig, update: Value) {
    rig.updates.lock().unwrap().push_back(update);
}

async fn expect_event(
    events: &mut broadcast::Receiver<EventRecord>,
    wanted: impl Fn(&Event) -> bool,
) -> EventRecord {
    let found = timeout(WAIT, async {
        loop {
            if let Ok(record) = events.recv().await
                && wanted(&record.event)
            {
                return record;
            }
        }
    });
    found.await.expect("the event did not arrive in time")
}

async fn next_sent(rig: &mut Rig) -> Value {
    timeout(WAIT, rig.sent.recv())
        .await
        .expect("no message was sent in time")
        .expect("the sender closed")
}

fn text_of(body: &Value) -> &str {
    body["text"].as_str().unwrap_or_default()
}

fn connect_body(user: i64) -> Value {
    json!({ "token": TOKEN, "user_id": user })
}

fn settings_body(levels: &[&str]) -> Value {
    json!({
        "levels": levels,
        "events": ALL_EVENTS,
        "detail": "detail",
        "quiet_hours": null,
    })
}

/// Connects the owner and answers their `/start`. Returns once the chat is paired.
async fn connect_and_pair(rig: &mut Rig, user: i64) {
    push(rig, owner_text(user, "/start"));
    let (status, body) = call(
        rig,
        Method::POST,
        "/api/telegram/connect",
        Some(connect_body(user)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    expect_event(&mut rig.events, |e| {
        matches!(e, Event::TelegramPaired { .. })
    })
    .await;
    let help = next_sent(rig).await;
    assert!(text_of(&help).contains("/help"), "{help}");
}

async fn set_levels(rig: &Rig, levels: &[&str]) {
    let (status, body) = call(
        rig,
        Method::PUT,
        "/api/telegram",
        Some(settings_body(levels)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

fn pending_approval() -> Approval {
    Approval {
        id: ApprovalId::new("approval-1"),
        kind: ApprovalKind::Extension,
        task: None,
        agent: None,
        path: None,
        state: ApprovalState::Pending,
        decided_by: None,
    }
}

/// Stores the approval, then announces it the way the run loop does.
fn request_approval(rig: &Rig) {
    let approval = pending_approval();
    rig.engine
        .store()
        .insert_approval(&approval, rig.engine.now())
        .unwrap();
    rig.engine
        .emit(&Event::ApprovalRequested { approval })
        .unwrap();
}

fn keyboard_labels(body: &Value) -> Vec<String> {
    body["reply_markup"]["inline_keyboard"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|row| row.as_array().into_iter().flatten())
        .filter_map(|button| button["text"].as_str().map(str::to_owned))
        .collect()
}

fn first_button_data(body: &Value) -> String {
    body["reply_markup"]["inline_keyboard"][0][0]["callback_data"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn connecting_saves_the_token_and_the_owner_pairs_with_start() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    assert_eq!(
        rig.secrets.get("telegram.bot_token").unwrap().as_deref(),
        Some(TOKEN)
    );
    let (status, state) = call(&rig, Method::GET, "/api/telegram", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(state["connected"], json!(true), "{state}");
    assert_eq!(state["bot_username"], json!("sentinel_bot"));
}

#[tokio::test]
async fn a_message_from_another_user_is_ignored_and_logged_without_its_text() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    push(&rig, owner_text(7, "/status"));
    let record = expect_event(&mut rig.events, |e| {
        matches!(
            e,
            Event::TelegramReceived {
                accepted: false,
                ..
            }
        )
    })
    .await;
    assert!(
        matches!(record.event, Event::TelegramReceived { ref command, .. } if command == "/status")
    );
    // Updates are handled in order. A reply to user 7 would be the next message sent.
    push(&rig, owner_text(OWNER, "/help"));
    let reply = next_sent(&mut rig).await;
    assert_eq!(reply["chat_id"], json!(OWNER), "{reply}");
    assert!(text_of(&reply).contains("/help"), "{reply}");
}

#[tokio::test]
async fn status_is_refused_at_notify_and_answered_at_control() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    push(&rig, owner_text(OWNER, "/status"));
    let refused = next_sent(&mut rig).await;
    assert!(
        text_of(&refused).contains("not on for this chat"),
        "{refused}"
    );

    set_levels(&rig, &["notify", "control"]).await;
    push(&rig, owner_text(OWNER, "/status"));
    let answered = next_sent(&mut rig).await;
    assert!(text_of(&answered).contains("Sentinel is"), "{answered}");
}

#[tokio::test]
async fn a_cap_above_the_current_cap_is_refused() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    set_levels(&rig, &["notify", "control"]).await;
    push(&rig, owner_text(OWNER, "/cap 999.00"));
    let reply = next_sent(&mut rig).await;
    assert!(text_of(&reply).contains("can only be lowered"), "{reply}");
}

#[tokio::test]
async fn a_run_is_refused_in_bypass_permissions() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    set_levels(&rig, &["notify", "code"]).await;
    rig.engine
        .update_config(|c| c.permission_mode = PermissionMode::Bypass)
        .unwrap();
    push(&rig, owner_text(OWNER, "/run"));
    let reply = next_sent(&mut rig).await;
    assert!(text_of(&reply).contains("Bypass"), "{reply}");
}

#[tokio::test]
async fn an_approval_request_is_sent_with_allow_and_deny_buttons() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    set_levels(&rig, &["notify", "control"]).await;
    request_approval(&rig);
    let card = next_sent(&mut rig).await;
    assert_eq!(keyboard_labels(&card), vec!["Allow", "Deny"], "{card}");
}

#[tokio::test]
async fn a_button_token_works_once_and_a_second_press_is_refused() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    set_levels(&rig, &["notify", "control"]).await;
    request_approval(&rig);
    let card = next_sent(&mut rig).await;
    let token = first_button_data(&card);

    push(&rig, owner_press(OWNER, &token));
    let first = next_sent(&mut rig).await;
    assert_eq!(text_of(&first), "Allowed.", "{first}");
    let stored = rig
        .engine
        .store()
        .get_approval(&ApprovalId::new("approval-1"))
        .unwrap()
        .unwrap();
    assert_eq!(stored.state, ApprovalState::Allowed);
    assert_eq!(stored.decided_by.as_deref(), Some("telegram"));

    push(&rig, owner_press(OWNER, &token));
    let second = next_sent(&mut rig).await;
    assert!(text_of(&second).contains("already used"), "{second}");
}

#[tokio::test]
async fn revoking_the_chat_stops_the_poller() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    while rig.polls.try_recv().is_ok() {}

    let (status, body) = call(
        &rig,
        Method::DELETE,
        &format!("/api/telegram/chats/{OWNER}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    expect_event(&mut rig.events, |e| {
        matches!(e, Event::ConnectionChanged { name, state: ConnectionState::Missing } if name == "telegram")
    })
    .await;
    while rig.polls.try_recv().is_ok() {}

    let quiet = timeout(Duration::from_millis(500), rig.polls.recv()).await;
    assert!(
        quiet.is_err(),
        "the poller asked for updates after the revoke"
    );
}

#[tokio::test]
async fn the_token_appears_in_no_response_no_event_and_no_log() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    let (_, state) = call(&rig, Method::GET, "/api/telegram", None).await;
    let (_, saved) = call(
        &rig,
        Method::POST,
        "/api/telegram/token",
        Some(json!({ "token": TOKEN })),
    )
    .await;
    assert!(!state.to_string().contains(TOKEN));
    assert!(!saved.to_string().contains(TOKEN));

    let events = rig.engine.events_after(0, 1000).unwrap();
    let dump: String = events
        .iter()
        .map(|record| serde_json::to_string(&record.event).unwrap())
        .collect();
    assert!(!dump.contains(TOKEN), "a stored event holds the token");
    let log =
        std::fs::read_to_string(rig.dir.path().join(".sentinel/log.jsonl")).unwrap_or_default();
    assert!(!log.contains(TOKEN), "the event log holds the token");
}

#[test]
fn shell_and_bypass_are_not_commands() {
    assert!(matches!(parse("/shell ls"), Err(ParseProblem::Unknown(_))));
    assert!(matches!(parse("/bypass"), Err(ParseProblem::Unknown(_))));
}
