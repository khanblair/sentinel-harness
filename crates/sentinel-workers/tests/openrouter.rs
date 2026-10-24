//! The `OpenRouter` worker against a fake API (`wiremock`), and the tool executor on its own.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use sentinel_core::{Cost, TaskId, ToolId};
use sentinel_workers::agent_tools::{ToolCall, ToolEnv, run_tool};
use sentinel_workers::budget::Budget;
use sentinel_workers::worker::{EndKind, OutputLine, RunControl, RunSpec, Worker};
use sentinel_workers::{OpenRouterConfig, OpenRouterWorker};
use serde_json::{Value, json};
use tokio::sync::{mpsc, watch};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const KEY: &str = "sk-or-test-0123456789";
const CHAT_PATH: &str = "/api/v1/chat/completions";

/// The run's control handles. The kill sender stays here so a test can fire the signal.
struct Control {
    run: RunControl,
    lines: mpsc::UnboundedReceiver<OutputLine>,
    kill: watch::Sender<bool>,
    budget: Arc<Budget>,
}

fn control() -> Control {
    let (budget, _budget_kill) = Budget::new(Cost::from_micro_usd(10_000_000, true));
    let (lines_tx, lines) = mpsc::unbounded_channel();
    let (kill, kill_rx) = watch::channel(false);
    Control {
        run: RunControl {
            kill: kill_rx,
            lines: lines_tx,
            budget: budget.clone(),
        },
        lines,
        kill,
        budget,
    }
}

fn drain(lines: &mut mpsc::UnboundedReceiver<OutputLine>) -> Vec<String> {
    let mut texts = Vec::new();
    while let Ok(line) = lines.try_recv() {
        texts.push(line.text);
    }
    texts
}

fn spec(workdir: &Path, scope: &[&str]) -> RunSpec {
    RunSpec {
        task: TaskId::new("O-1-test"),
        agent: ToolId::new("openrouter"),
        model: "test/model".to_string(),
        prompt: "Add the greeting.".to_string(),
        workdir: workdir.to_path_buf(),
        timeout: Duration::from_secs(60),
        scope: scope.iter().map(ToString::to_string).collect(),
    }
}

fn worker_for(server: &MockServer, max_steps: u32) -> OpenRouterWorker {
    let config = OpenRouterConfig {
        base_url: format!("{}/api/v1", server.uri()),
        api_key: KEY.to_string(),
        model: "test/model".to_string(),
        max_steps,
        retry_base: Duration::ZERO,
        ..OpenRouterConfig::default()
    };
    OpenRouterWorker::new(config).expect("the HTTP client builds")
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

async fn mount_step(server: &MockServer, response: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path(CHAT_PATH))
        .respond_with(response)
        .up_to_n_times(1)
        .mount(server)
        .await;
}

/// The body of the nth request the fake API received, as JSON.
async fn request_body(server: &MockServer, index: usize) -> Value {
    let requests = server
        .received_requests()
        .await
        .expect("requests are recorded");
    requests[index].body_json().expect("the body is JSON")
}

fn tool_call(name: &str, arguments: Value) -> ToolCall {
    ToolCall {
        id: "call_1".to_string(),
        name: name.to_string(),
        arguments,
    }
}

fn env(workdir: &Path, scope: &[&str]) -> ToolEnv {
    ToolEnv {
        workdir: workdir.to_path_buf(),
        scope: scope.iter().map(ToString::to_string).collect(),
    }
}

#[tokio::test]
async fn scripted_run_writes_the_file_and_completes_with_recorded_cost() {
    let dir = tempfile::tempdir().expect("temp folder");
    fs::create_dir_all(dir.path().join("src")).expect("src folder");
    fs::write(dir.path().join("src/lib.rs"), "old\n").expect("seed file");
    let server = MockServer::start().await;
    mount_step(
        &server,
        tool_reply("read_file", &json!({"path": "src/lib.rs"})),
    )
    .await;
    mount_step(
        &server,
        tool_reply(
            "write_file",
            &json!({"path": "src/lib.rs", "content": "new\n"}),
        ),
    )
    .await;
    mount_step(&server, tool_reply("finish", &json!({"summary": "Done."}))).await;

    let ctl = control();
    let outcome = worker_for(&server, 30)
        .run(spec(dir.path(), &["src/lib.rs"]), ctl.run)
        .await;

    assert_eq!(outcome.end, EndKind::Completed);
    assert_eq!(
        fs::read_to_string(dir.path().join("src/lib.rs")).expect("file exists"),
        "new\n"
    );
    let expected = Cost::parse_usd("0.0015").expect("valid amount");
    assert_eq!(outcome.cost, expected);
    assert_eq!(ctl.budget.spent(), expected);
}

#[tokio::test]
async fn write_outside_scope_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().expect("temp folder");
    let server = MockServer::start().await;
    mount_step(
        &server,
        tool_reply(
            "write_file",
            &json!({"path": "src/other.rs", "content": "x"}),
        ),
    )
    .await;
    mount_step(&server, tool_reply("finish", &json!({"summary": "Done."}))).await;

    let ctl = control();
    worker_for(&server, 30)
        .run(spec(dir.path(), &["src/allowed.rs"]), ctl.run)
        .await;

    assert!(!dir.path().join("src/other.rs").exists());
    let second = request_body(&server, 1).await;
    assert!(
        second
            .to_string()
            .contains("src/other.rs is not reserved for this task. Write only: src/allowed.rs.")
    );
}

#[tokio::test]
async fn rate_limit_then_success_is_retried() {
    let dir = tempfile::tempdir().expect("temp folder");
    let server = MockServer::start().await;
    mount_step(&server, ResponseTemplate::new(429)).await;
    mount_step(&server, tool_reply("finish", &json!({"summary": "Done."}))).await;

    let ctl = control();
    let outcome = worker_for(&server, 30)
        .run(spec(dir.path(), &[]), ctl.run)
        .await;

    assert_eq!(outcome.end, EndKind::Completed);
    let requests = server.received_requests().await.unwrap_or_default();
    assert_eq!(requests.len(), 2, "one 429 and one retry");
}

#[tokio::test]
async fn step_limit_ends_the_run_as_failed_with_a_clear_last_line() {
    let dir = tempfile::tempdir().expect("temp folder");
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(CHAT_PATH))
        .respond_with(tool_reply("list_dir", &json!({"path": "."})))
        .mount(&server)
        .await;

    let ctl = control();
    let outcome = worker_for(&server, 2)
        .run(spec(dir.path(), &[]), ctl.run)
        .await;

    assert_eq!(outcome.end, EndKind::Failed);
    assert_eq!(
        outcome.output_tail.last().map(String::as_str),
        Some("Stopped: step limit of 2 reached.")
    );
}

#[tokio::test]
async fn kill_signal_during_a_call_stops_the_run_as_killed() {
    let dir = tempfile::tempdir().expect("temp folder");
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(CHAT_PATH))
        .respond_with(
            tool_reply("finish", &json!({"summary": "Late."})).set_delay(Duration::from_secs(30)),
        )
        .mount(&server)
        .await;

    let ctl = control();
    let worker = worker_for(&server, 30);
    let waiting = async {
        while server
            .received_requests()
            .await
            .is_none_or(|requests| requests.is_empty())
        {
            tokio::task::yield_now().await;
        }
        let _ = ctl.kill.send(true);
    };
    let (outcome, ()) = tokio::join!(worker.run(spec(dir.path(), &[]), ctl.run), waiting);

    assert_eq!(outcome.end, EndKind::Killed);
}

#[tokio::test]
async fn api_key_never_appears_in_output_tail_stream_or_debug_text() {
    let dir = tempfile::tempdir().expect("temp folder");
    let server = MockServer::start().await;
    let leaky = ResponseTemplate::new(200).set_body_json(json!({
        "choices": [{"message": {
            "role": "assistant",
            "content": format!("My key is {KEY}, finishing."),
            "tool_calls": [{"id": "call_1", "type": "function",
                "function": {"name": "finish", "arguments": "{\"summary\":\"ok\"}"}}],
        }}],
        "usage": {"total_tokens": 10, "cost": 0.0001},
    }));
    mount_step(&server, leaky).await;

    let config = OpenRouterConfig {
        base_url: format!("{}/api/v1", server.uri()),
        api_key: KEY.to_string(),
        retry_base: Duration::ZERO,
        ..OpenRouterConfig::default()
    };
    let debug_text = format!("{config:?}");
    let worker = OpenRouterWorker::new(config).expect("the HTTP client builds");
    let mut ctl = control();
    let outcome = worker.run(spec(dir.path(), &[]), ctl.run).await;

    assert!(!debug_text.contains(KEY));
    assert!(outcome.output_tail.iter().all(|line| !line.contains(KEY)));
    assert!(drain(&mut ctl.lines).iter().all(|text| !text.contains(KEY)));
    assert!(
        outcome
            .output_tail
            .iter()
            .any(|line| line.contains("[hidden]"))
    );
}

#[tokio::test]
async fn large_tool_output_reaches_the_model_cut_to_the_head_and_tail() {
    let dir = tempfile::tempdir().expect("temp folder");
    fs::create_dir_all(dir.path().join("src")).expect("src folder");
    let big: Vec<String> = (1..=500).map(|index| format!("line {index}")).collect();
    fs::write(dir.path().join("src/big.rs"), big.join("\n")).expect("seed file");
    let server = MockServer::start().await;
    mount_step(
        &server,
        tool_reply("read_file", &json!({"path": "src/big.rs"})),
    )
    .await;
    mount_step(&server, tool_reply("finish", &json!({"summary": "Done."}))).await;

    let ctl = control();
    worker_for(&server, 30)
        .run(spec(dir.path(), &[]), ctl.run)
        .await;

    let second = request_body(&server, 1).await;
    let messages = second["messages"].as_array().expect("messages array");
    let tool = messages
        .iter()
        .find(|message| message["role"] == "tool")
        .expect("a tool message");
    let content = tool["content"].as_str().expect("tool content");
    assert_eq!(content.lines().count(), 81, "40 head, 1 marker, 40 tail");
    assert!(content.contains("[... 420 lines cut ...]"));
}

#[test]
fn run_command_refuses_rm_rf() {
    let dir = tempfile::tempdir().expect("temp folder");
    let call = tool_call("run_command", json!({"command": "rm -rf /"}));
    let result = run_tool(&call, &env(dir.path(), &[]));
    assert_eq!(result.text, "That command is not allowed.");
    assert!(!result.finished);
}

#[test]
fn run_command_refuses_cargo_fmt_without_check() {
    let dir = tempfile::tempdir().expect("temp folder");
    let call = tool_call("run_command", json!({"command": "cargo fmt"}));
    let result = run_tool(&call, &env(dir.path(), &[]));
    assert_eq!(result.text, "That command is not allowed.");
}

#[test]
fn run_command_runs_an_allowed_cargo_command() {
    let dir = tempfile::tempdir().expect("temp folder");
    let call = tool_call("run_command", json!({"command": "cargo check --quiet"}));
    let result = run_tool(&call, &env(dir.path(), &[]));
    assert!(
        result.text.starts_with("exit code "),
        "got: {}",
        result.text
    );
}

#[test]
fn path_with_parent_segments_is_refused_for_read_and_write() {
    let dir = tempfile::tempdir().expect("temp folder");
    let read = tool_call("read_file", json!({"path": "../secret.txt"}));
    let write = tool_call("write_file", json!({"path": "../x.txt", "content": "x"}));
    assert_eq!(
        run_tool(&read, &env(dir.path(), &[])).text,
        "That path is not allowed."
    );
    assert_eq!(
        run_tool(&write, &env(dir.path(), &["../x.txt"])).text,
        "That path is not allowed."
    );
}

#[test]
fn reading_git_and_sentinel_folders_is_refused() {
    let dir = tempfile::tempdir().expect("temp folder");
    let call = tool_call("read_file", json!({"path": ".git/config"}));
    assert_eq!(
        run_tool(&call, &env(dir.path(), &[])).text,
        "That path is not allowed."
    );
}

#[cfg(unix)]
#[test]
fn write_through_a_symlink_outside_the_workdir_is_refused() {
    use std::os::unix::fs::symlink;

    let workdir = tempfile::tempdir().expect("temp folder");
    let outside = tempfile::tempdir().expect("temp folder");
    symlink(outside.path(), workdir.path().join("link")).expect("symlink");
    let call = tool_call("write_file", json!({"path": "link/x.txt", "content": "x"}));
    let result = run_tool(&call, &env(workdir.path(), &["link/x.txt"]));
    assert_eq!(result.text, "That path is not allowed.");
    assert!(!outside.path().join("x.txt").exists());
}

#[test]
fn finish_marks_the_result_as_finished() {
    let dir = tempfile::tempdir().expect("temp folder");
    let call = tool_call("finish", json!({"summary": "Greeting added."}));
    let result = run_tool(&call, &env(dir.path(), &[]));
    assert!(result.finished);
    assert_eq!(result.text, "Finished: Greeting added.");
}
