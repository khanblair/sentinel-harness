//! The `OpenRouter` worker: Sentinel's own agent loop for models reached through `OpenRouter`.
//!
//! One run is a loop of model calls. A reply may hold tool calls. Sentinel runs them, feeds the
//! results back, and stops on `finish`, a plain answer, the step limit, a kill, the task cost
//! ceiling, or the task time limit.

use std::time::Duration;

use futures::future::BoxFuture;
use reqwest::{Response, StatusCode};
use sentinel_core::{Cost, TaskId};
use serde_json::{Value, json};
use tokio::sync::watch;
use tokio::time;

use crate::agent_tools::{ToolCall, ToolEnv, ToolResult, run_tool, tool_definitions};
use crate::budget::BudgetEvent;
use crate::context::{ContextLimits, estimate_tokens, truncate_tool_output};
use crate::openrouter_config::redact;
pub use crate::openrouter_config::{OpenRouterConfig, OpenRouterError};
use crate::transcript::Transcript;
use crate::worker::{EndKind, OutputLine, RunControl, RunOutcome, RunSpec, Worker};

const CALL_TIMEOUT: Duration = Duration::from_secs(120);
/// Retries after the first attempt, for a 429 or a 5xx. Waits are the base, doubled each time.
const RETRIES: u32 = 3;
const OUTPUT_LINE_CHARS: usize = 400;
const TAIL_LINES: usize = 200;
/// Price used when a reply has no dollar figure: 0.000002 dollars per token.
const UNKNOWN_MICRO_PER_TOKEN: u64 = 2;
const SYSTEM_PROMPT: &str = "You are one of several coding agents working in the same folder. \
Do the task. You may write only the files listed in your scope. Read before you write. \
Run the tests when you are done. Call finish with one sentence when the task is done.";

/// Runs a task through an `OpenRouter` chat model and its own tool loop.
pub struct OpenRouterWorker {
    config: OpenRouterConfig,
    client: reqwest::Client,
}

impl OpenRouterWorker {
    /// Build the worker. Every HTTP call gets a 120 second timeout.
    pub fn new(config: OpenRouterConfig) -> Result<Self, OpenRouterError> {
        let client = reqwest::Client::builder().timeout(CALL_TIMEOUT).build()?;
        Ok(Self { config, client })
    }
}

impl Worker for OpenRouterWorker {
    fn run(&self, spec: RunSpec, ctrl: RunControl) -> BoxFuture<'_, RunOutcome> {
        Box::pin(run_task(self, spec, ctrl))
    }
}

async fn run_task(worker: &OpenRouterWorker, spec: RunSpec, ctrl: RunControl) -> RunOutcome {
    let limit = spec.timeout;
    let mut run = TaskRun::new(worker, spec, ctrl);
    let stop = time::timeout(limit, run.drive())
        .await
        .unwrap_or(Stop::TimedOut);
    run.finish(stop)
}

/// Why a run ended, before it is mapped to an `EndKind`.
enum Stop {
    Completed,
    Failed(String),
    StepLimit,
    TimedOut,
    Killed,
    CostCeiling,
}

/// Why one model call failed. A 429 or a 5xx may be retried.
enum CallError {
    Retryable(String),
    Failed(String),
    Killed,
}

impl CallError {
    fn into_stop(self) -> Stop {
        match self {
            Self::Retryable(text) | Self::Failed(text) => {
                Stop::Failed(format!("Model call failed: {text}."))
            }
            Self::Killed => Stop::Killed,
        }
    }
}

/// The state of one task attempt.
struct TaskRun<'a> {
    worker: &'a OpenRouterWorker,
    spec: RunSpec,
    ctrl: RunControl,
    env: ToolEnv,
    transcript: Transcript,
    limits: ContextLimits,
    cost: Cost,
    output_tail: Vec<String>,
}

impl<'a> TaskRun<'a> {
    fn new(worker: &'a OpenRouterWorker, spec: RunSpec, ctrl: RunControl) -> Self {
        let env = ToolEnv {
            workdir: spec.workdir.clone(),
            scope: spec.scope.clone(),
        };
        let transcript = Transcript::new(&system_prompt(&spec.scope), &spec.prompt);
        Self {
            worker,
            spec,
            ctrl,
            env,
            transcript,
            limits: ContextLimits::default(),
            cost: Cost::ZERO,
            output_tail: Vec::new(),
        }
    }

    async fn drive(&mut self) -> Stop {
        if *self.ctrl.kill.borrow() {
            return Stop::Killed;
        }
        for _ in 0..self.worker.config.max_steps {
            if let Some(stop) = self.step().await {
                return stop;
            }
        }
        Stop::StepLimit
    }

    /// One model call and the tool calls it asks for. Returns a stop when the run is over.
    async fn step(&mut self) -> Option<Stop> {
        self.transcript
            .compact_if_needed(self.worker.config.window_tokens, &self.limits);
        let reply = match self.call_model().await {
            Ok(reply) => reply,
            Err(stop) => return Some(stop),
        };
        if let Some(stop) = self.charge(&reply) {
            return Some(stop);
        }
        let message = reply["choices"][0]["message"].clone();
        self.record_reply(&message);
        let calls = message["tool_calls"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if calls.is_empty() {
            return Some(Stop::Completed);
        }
        self.run_calls(&calls).await
    }

    /// Send the request, retrying a 429 or a 5xx with doubling waits.
    async fn call_model(&mut self) -> Result<Value, Stop> {
        let mut attempt = 0;
        loop {
            match self.post_once().await {
                Err(CallError::Retryable(_)) if attempt < RETRIES => {
                    self.wait_before_retry(attempt).await?;
                    attempt += 1;
                }
                Ok(reply) => return Ok(reply),
                Err(error) => return Err(error.into_stop()),
            }
        }
    }

    async fn wait_before_retry(&mut self, attempt: u32) -> Result<(), Stop> {
        let delay = self.worker.config.retry_base * 2_u32.pow(attempt);
        tokio::select! {
            () = time::sleep(delay) => Ok(()),
            () = killed(&mut self.ctrl.kill) => Err(Stop::Killed),
        }
    }

    async fn post_once(&mut self) -> Result<Value, CallError> {
        let worker = self.worker;
        let request = worker
            .client
            .post(chat_url(&worker.config.base_url))
            .bearer_auth(&worker.config.api_key)
            .json(&self.request_body())
            .send();
        let sent = tokio::select! {
            sent = request => sent,
            () = killed(&mut self.ctrl.kill) => return Err(CallError::Killed),
        };
        let response = sent.map_err(|error| CallError::Failed(error.to_string()))?;
        read_reply(response).await
    }

    fn request_body(&self) -> Value {
        json!({
            "model": self.worker.config.model,
            "messages": self.transcript.messages(),
            "tools": tool_definitions(),
            "usage": {"include": true},
        })
    }

    /// Count the reply's cost. Returns a stop when the task ceiling or the kill signal fired.
    fn charge(&mut self, reply: &Value) -> Option<Stop> {
        let cost = reply_cost(reply, self.transcript.used_tokens());
        self.cost = self.cost + cost;
        let events = self.ctrl.budget.record(&self.spec.task, cost);
        if events
            .iter()
            .any(|event| is_ceiling(event, &self.spec.task))
        {
            return Some(Stop::CostCeiling);
        }
        if *self.ctrl.kill.borrow() {
            return Some(Stop::Killed);
        }
        None
    }

    fn record_reply(&mut self, message: &Value) {
        let text = message["content"].as_str().unwrap_or_default();
        if !text.is_empty() {
            self.emit(text);
        }
        self.transcript.start_step(message.clone());
    }

    /// Run the calls of one reply in order. A `finish` ends the run.
    async fn run_calls(&mut self, calls: &[Value]) -> Option<Stop> {
        for entry in calls {
            let call = ToolCall::from_reply(entry);
            self.emit(&call.label());
            let result = match self.execute(call.clone()).await {
                Ok(result) => result,
                Err(stop) => return Some(stop),
            };
            self.push_result(&call.id, &result);
            if result.finished {
                return Some(Stop::Completed);
            }
        }
        None
    }

    /// Run one tool on a blocking thread, so a slow command does not hold the runtime. A kill
    /// stops the wait at once.
    async fn execute(&mut self, call: ToolCall) -> Result<ToolResult, Stop> {
        let env = self.env.clone();
        let work = tokio::task::spawn_blocking(move || run_tool(&call, &env));
        tokio::select! {
            joined = work => joined.map_err(|error| Stop::Failed(format!("A tool crashed: {error}."))),
            () = killed(&mut self.ctrl.kill) => Err(Stop::Killed),
        }
    }

    fn push_result(&mut self, call_id: &str, result: &ToolResult) {
        let content = truncate_tool_output(&result.text, &self.limits);
        self.transcript
            .push_result(json!({"role": "tool", "tool_call_id": call_id, "content": content}));
    }

    /// Send one line to the stream and keep it in the tail. The API key is hidden first.
    fn emit(&mut self, text: &str) {
        let clean: String = redact(text, &self.worker.config.api_key)
            .chars()
            .take(OUTPUT_LINE_CHARS)
            .collect();
        if self.output_tail.len() == TAIL_LINES {
            self.output_tail.remove(0);
        }
        self.output_tail.push(clean.clone());
        let line = OutputLine {
            task: self.spec.task.clone(),
            agent: self.spec.agent.clone(),
            text: clean,
        };
        let _ = self.ctrl.lines.send(line);
    }

    fn finish(mut self, stop: Stop) -> RunOutcome {
        let (end, last_line) = self.outcome_for(stop);
        if let Some(line) = last_line {
            self.emit(&line);
        }
        RunOutcome {
            end,
            exit_code: None,
            output_tail: self.output_tail,
            cost: self.cost,
        }
    }

    fn outcome_for(&self, stop: Stop) -> (EndKind, Option<String>) {
        match stop {
            Stop::Completed => (EndKind::Completed, None),
            Stop::Failed(reason) => (EndKind::Failed, Some(reason)),
            Stop::StepLimit => (
                EndKind::Failed,
                Some(format!(
                    "Stopped: step limit of {} reached.",
                    self.worker.config.max_steps
                )),
            ),
            Stop::TimedOut => (
                EndKind::TimedOut,
                Some("Stopped: time limit reached.".to_string()),
            ),
            Stop::Killed => (EndKind::Killed, Some("Stopped: killed.".to_string())),
            Stop::CostCeiling => (
                EndKind::CostCeiling,
                Some("Stopped: task cost ceiling reached.".to_string()),
            ),
        }
    }
}

/// Read a reply. A 429 or a 5xx comes back as retryable. Anything else that is not a 2xx fails.
async fn read_reply(response: Response) -> Result<Value, CallError> {
    let status = response.status();
    if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        return Err(CallError::Retryable(format!("HTTP {}", status.as_u16())));
    }
    let body = response
        .text()
        .await
        .map_err(|error| CallError::Failed(error.to_string()))?;
    if !status.is_success() {
        return Err(CallError::Failed(format!("HTTP {}", status.as_u16())));
    }
    let reply: Value = serde_json::from_str(&body)
        .map_err(|error| CallError::Failed(format!("the reply was not JSON: {error}")))?;
    if reply["choices"][0]["message"].is_null() {
        return Err(CallError::Failed("the reply had no message".to_string()));
    }
    Ok(reply)
}

/// Resolves once the kill signal is set. If every sender is gone without a signal, it never
/// resolves, so a dropped sender cannot stop a run by accident.
async fn killed(kill: &mut watch::Receiver<bool>) {
    loop {
        if *kill.borrow_and_update() {
            return;
        }
        if kill.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

fn is_ceiling(event: &BudgetEvent, task: &TaskId) -> bool {
    matches!(event, BudgetEvent::CeilingHit { task: hit, .. } if hit == task)
}

fn chat_url(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim_end_matches('/'))
}

fn system_prompt(scope: &[String]) -> String {
    let reserved = if scope.is_empty() {
        "none".to_string()
    } else {
        scope.join("\n")
    };
    format!("{SYSTEM_PROMPT}\n\nScope:\n{reserved}")
}

/// The cost of one reply. `usage.cost` is exact when present. Otherwise the token count gives a
/// guess, from the reported total or from the input and the reply sizes.
fn reply_cost(reply: &Value, input_tokens: u32) -> Cost {
    let usage = &reply["usage"];
    if let Some(cost) = usage["cost"].as_f64().and_then(exact_cost) {
        return cost;
    }
    let tokens = usage["total_tokens"].as_u64().unwrap_or_else(|| {
        u64::from(input_tokens) + u64::from(estimate_tokens(&reply.to_string()))
    });
    Cost::from_micro_usd(tokens.saturating_mul(UNKNOWN_MICRO_PER_TOKEN), true)
}

/// Dollars from a float reply field, to the micro-dollar. Negative or non-finite values are
/// not a cost.
fn exact_cost(dollars: f64) -> Option<Cost> {
    if !dollars.is_finite() || dollars < 0.0 {
        return None;
    }
    Cost::parse_usd(&format!("{dollars:.6}")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_hides_the_key_and_keeps_other_text() {
        assert_eq!(
            redact("key sk-1 and sk-1", "sk-1"),
            "key [hidden] and [hidden]"
        );
        assert_eq!(redact("no key here", ""), "no key here");
    }

    #[test]
    fn debug_output_hides_the_api_key() {
        let config = OpenRouterConfig {
            api_key: "sk-or-secret".to_string(),
            ..OpenRouterConfig::default()
        };
        let text = format!("{config:?}");
        assert!(!text.contains("sk-or-secret"));
        assert!(text.contains("api_key: \"[hidden]\""));
    }

    #[test]
    fn exact_cost_reads_dollars_without_floats() {
        assert_eq!(exact_cost(0.0005), Cost::parse_usd("0.0005").ok());
        assert_eq!(exact_cost(1.5e-5), Cost::parse_usd("0.000015").ok());
        assert_eq!(exact_cost(-1.0), None);
    }

    #[test]
    fn reply_without_cost_is_estimated_per_token() {
        let reply = json!({"usage": {"total_tokens": 1000}});
        let cost = reply_cost(&reply, 0);
        assert_eq!(cost.micro_usd(), 2000);
        assert!(cost.is_estimated());
    }

    #[test]
    fn chat_url_has_no_double_slash() {
        assert_eq!(
            chat_url("https://example.test/api/v1/"),
            "https://example.test/api/v1/chat/completions"
        );
    }
}
