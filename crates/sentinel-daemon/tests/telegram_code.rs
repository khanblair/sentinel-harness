//! The phone commands `/task`, `/diff`, and `/daily`, against a `wiremock` Bot API. The rig is a
//! trimmed copy of the one in `tests/telegram.rs`. Waits use bus events and channels, each with a
//! timeout, so no test sleeps.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

mod common;

use std::collections::VecDeque;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::{Method, StatusCode, header::AUTHORIZATION};
use sentinel_core::{Event, EventRecord, Task, TaskId, TaskStatus};
use sentinel_daemon::cmd::SystemRunner;
use sentinel_daemon::engine::{Engine, EngineOptions};
use sentinel_daemon::secrets::MemoryStore;
use sentinel_daemon::server;
use sentinel_store::CheckpointRecord;
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
}

impl Respond for Updates {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
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

/// Answers `sendMessage`, and passes each request body to the test.
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
    /// Kept alive so the mock keeps answering. Tests never read it.
    _server: MockServer,
    base: String,
    desktop: String,
    updates: Arc<Mutex<VecDeque<Value>>>,
    sent: mpsc::UnboundedReceiver<Value>,
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
    let updates = Arc::new(Mutex::new(VecDeque::new()));
    let poller = Updates {
        queue: Arc::clone(&updates),
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
        _server: server,
        base,
        desktop,
        updates,
        sent: sent_rx,
        events,
        dir,
    }
}

/// An engine on a temporary folder, with the Bot API pointed at the mock server.
fn open_engine(repo: &Path, secrets: &MemoryStore, bot_api: String) -> Engine {
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
        Some(json!({ "token": TOKEN, "user_id": user })),
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

fn first_button_data(body: &Value) -> String {
    body["reply_markup"]["inline_keyboard"][0][0]["callback_data"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// A repository with one commit that names a task in its message. The commit message must not
/// reach the chat.
fn commit_repo(repo: &Path) -> String {
    common::git(repo, &["init", "-q"]);
    common::git(repo, &["config", "user.email", "test@example.com"]);
    common::git(repo, &["config", "user.name", "Test"]);
    std::fs::write(repo.join("lib.rs"), "fn a() {}\n").unwrap();
    common::git(repo, &["add", "lib.rs"]);
    common::git(repo, &["commit", "-q", "-m", "T-01 Add a rate limit"]);
    let output = Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[tokio::test]
async fn task_confirm_appends_the_next_id_and_keeps_the_old_bytes() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    set_levels(&rig, &["notify", "code"]).await;
    let backlog = rig.dir.path().join("backlog.md");
    std::fs::write(&backlog, "# Backlog\n\n## T-01 Old task\nBody.\n").unwrap();

    push(
        &rig,
        owner_text(
            OWNER,
            "/task Add a rate limit\nRefuse 30 transfers a minute.",
        ),
    );
    let card = next_sent(&mut rig).await;
    assert!(text_of(&card).contains("Add a rate limit"), "{card}");
    let token = first_button_data(&card);

    push(&rig, owner_press(OWNER, &token));
    let reply = next_sent(&mut rig).await;
    assert!(text_of(&reply).contains("Added as T-02"), "{reply}");

    let written = std::fs::read_to_string(&backlog).unwrap();
    assert!(
        written.starts_with("# Backlog\n\n## T-01 Old task\nBody.\n"),
        "{written}"
    );
    assert!(
        written.ends_with(
            "## T-02 Add a rate limit\n\nAdd a rate limit\nRefuse 30 transfers a minute.\n"
        ),
        "{written}"
    );
}

#[tokio::test]
async fn the_token_appears_in_no_reply_to_a_task() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    set_levels(&rig, &["notify", "code"]).await;
    push(&rig, owner_text(OWNER, "/task Add logging"));
    let card = next_sent(&mut rig).await;
    let token = first_button_data(&card);
    assert!(!text_of(&card).contains(&token), "{card}");

    push(&rig, owner_press(OWNER, &token));
    let reply = next_sent(&mut rig).await;
    assert!(!text_of(&reply).contains(&token), "{reply}");
}

#[tokio::test]
async fn a_task_at_notify_level_is_refused_and_writes_nothing() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    set_levels(&rig, &["notify", "reports"]).await;
    push(&rig, owner_text(OWNER, "/task Add logging"));
    let reply = next_sent(&mut rig).await;
    assert!(text_of(&reply).contains("not on for this chat"), "{reply}");
    assert!(!rig.dir.path().join("backlog.md").exists());
}

#[tokio::test]
async fn a_diff_after_a_checkpoint_names_the_task_and_the_file_but_not_the_commit_message() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    set_levels(&rig, &["notify", "code"]).await;
    let repo = rig.dir.path().to_path_buf();
    let commit = commit_repo(&repo);
    rig.engine
        .store()
        .upsert_task(&Task::new(TaskId::new("T-01"), "Add a rate limit", "Body."))
        .unwrap();
    let checkpoint = CheckpointRecord {
        task: TaskId::new("T-01"),
        commit,
        paths: vec!["lib.rs".to_owned()],
        at: rig.engine.now(),
    };
    rig.engine.store().insert_checkpoint(&checkpoint).unwrap();

    push(&rig, owner_text(OWNER, "/diff T-01"));
    let reply = text_of(&next_sent(&mut rig).await).to_owned();
    assert!(reply.contains("T-01"), "{reply}");
    assert!(reply.contains("lib.rs"), "{reply}");
    assert!(reply.contains("+fn a() {}"), "{reply}");
    assert!(!reply.contains("Add a rate limit"), "{reply}");
}

#[tokio::test]
async fn a_diff_for_an_unknown_task_says_it_does_not_know_the_task() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    set_levels(&rig, &["notify", "code"]).await;
    push(&rig, owner_text(OWNER, "/diff T-99"));
    let reply = next_sent(&mut rig).await;
    assert!(
        text_of(&reply).contains("I do not know task T-99"),
        "{reply}"
    );
}

#[tokio::test]
async fn daily_holds_the_verified_count_and_shows_the_title() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    set_levels(&rig, &["notify", "reports"]).await;
    rig.engine.store().upsert_task(&verified_task()).unwrap();
    rig.engine
        .emit(&Event::Verdict {
            task: TaskId::new("T-01"),
            status: TaskStatus::Verified,
            reason: "All four checks passed.".to_owned(),
        })
        .unwrap();

    push(&rig, owner_text(OWNER, "/daily"));
    let reply = text_of(&next_sent(&mut rig).await).to_owned();
    assert!(reply.contains("Verified today: 1. In total: 1."), "{reply}");
    assert!(reply.contains("T-01 Add a rate limit"), "{reply}");
}

fn verified_task() -> Task {
    let mut task = Task::new(TaskId::new("T-01"), "Add a rate limit", "Body.");
    task.status = TaskStatus::Verified;
    task
}

#[tokio::test]
async fn daily_hides_task_titles_in_hackathon_mode() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    set_levels(&rig, &["notify", "reports"]).await;
    rig.engine
        .update_config(|c| c.hackathon_mode = true)
        .unwrap();
    rig.engine.store().upsert_task(&verified_task()).unwrap();
    rig.engine
        .emit(&Event::Verdict {
            task: TaskId::new("T-01"),
            status: TaskStatus::Verified,
            reason: "All four checks passed.".to_owned(),
        })
        .unwrap();

    push(&rig, owner_text(OWNER, "/daily"));
    let reply = text_of(&next_sent(&mut rig).await).to_owned();
    assert!(reply.contains("Verified today: 1."), "{reply}");
    assert!(!reply.contains("Add a rate limit"), "{reply}");
}

#[tokio::test]
async fn the_event_bus_records_the_task_add_as_a_device_action() {
    let mut rig = rig().await;
    connect_and_pair(&mut rig, OWNER).await;
    set_levels(&rig, &["notify", "code"]).await;
    push(&rig, owner_text(OWNER, "/task Add logging"));
    let card = next_sent(&mut rig).await;
    let token = first_button_data(&card);
    push(&rig, owner_press(OWNER, &token));
    expect_event(
        &mut rig.events,
        |e| matches!(e, Event::DeviceAction { action, .. } if action.starts_with("added T-01")),
    )
    .await;
}
