//! Test one agent CLI for real: one tiny headless prompt through the same worker a task uses, in an
//! empty folder, with a hard timeout. The owner reads one sentence that says whether it works.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use sentinel_core::api::{KeyTest, ToolTest};
use sentinel_core::{Cost, TaskId, ToolId, ToolInfo, ToolStatus};
use sentinel_workers::{Budget, EndKind, RunControl, RunOutcome, RunSpec, Worker};
use tokio::sync::mpsc;

use crate::agent_health::note_test_result;
use crate::engine::{Engine, EngineError, Result};
use crate::openrouter_tool::is_openrouter_adapter;

/// The longest a test may run. The worker kills the agent when it is reached.
pub(crate) const PROBE_TIMEOUT: Duration = Duration::from_secs(90);
const PROBE_PROMPT: &str = "Reply with the single word: ok";
const PROBE_TASK: &str = "AGENT-TEST";
/// A test spends a few tokens. This cap stops a stuck run long before it costs real money.
const PROBE_CAP_MICRO_USD: u64 = 1_000_000;
const OPENROUTER_PROVIDER: &str = "openrouter";
const MAX_LINE_CHARS: usize = 160;
const MASK: &str = "...";
const MIN_SECRET_CHARS: usize = 20;
const SECRET_MARKERS: [&str; 3] = ["key", "token", "bearer"];
const SEPARATORS: &[u8] = b" :=\"'";
const SIGN_IN_MARKERS: [&str; 6] = [
    "login",
    "sign in",
    "authenticate",
    "api key",
    "unauthorized",
    "401",
];
const MISSING: &str =
    "This agent was not found on this computer. Install it, then press Scan again.";
const NO_COMMAND: &str =
    "Sentinel has no command line for this agent yet. Add one in Settings, Agents.";
const TIMED_OUT: &str = "It did not answer in 90 seconds.";
const NO_REPLY: &str = "It stopped without a reply.";
const NO_REASON: &str = "It stopped without saying why.";
const NO_FOLDER: &str = "Sentinel could not make a folder to test in.";
const ALREADY_RUNNING: &str = "A test of this agent is already running.";

/// Tools with a test running now. A second test of the same tool is refused until the first ends.
static RUNNING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Holds one tool's place in `RUNNING` until the test ends, even when the test returns early.
struct TestClaim {
    tool: String,
}

impl TestClaim {
    fn take(tool: &ToolId) -> Option<Self> {
        let name = tool.to_string();
        let added = running().insert(name.clone());
        added.then_some(Self { tool: name })
    }
}

impl Drop for TestClaim {
    fn drop(&mut self) {
        running().remove(&self.tool);
    }
}

fn running() -> MutexGuard<'static, HashSet<String>> {
    RUNNING.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What one probe run ended with and how long it took.
pub(crate) struct ProbeResult {
    pub outcome: RunOutcome,
    pub elapsed: Duration,
}

impl Engine {
    /// Test one agent. `None` when no tool has that id.
    pub(crate) async fn test_agent(&self, id: &ToolId) -> Result<Option<ToolTest>> {
        let Some(tool) = self.store().list_tools()?.into_iter().find(|t| &t.id == id) else {
            return Ok(None);
        };
        if let Some(message) = unrunnable_sentence(&tool) {
            return Ok(Some(tool_test(&tool.id, false, message, Duration::ZERO)));
        }
        let _claim = TestClaim::take(&tool.id)
            .ok_or_else(|| EngineError::Conflict(ALREADY_RUNNING.to_owned()))?;
        let started = Instant::now();
        let (ok, message) = if is_openrouter_adapter(&tool.adapter) {
            openrouter_verdict(self.test_key(OPENROUTER_PROVIDER).await)
        } else {
            self.probe_verdict(&tool).await
        };
        note_test_result(&self.inner.health, &tool.id, ok, &message);
        Ok(Some(tool_test(&tool.id, ok, &message, started.elapsed())))
    }

    /// Run the probe through the tool's own worker, in a fresh folder that is a git repo, as a task
    /// would see it. Some agents refuse to work outside a repo.
    async fn probe_verdict(&self, tool: &ToolInfo) -> (bool, String) {
        let worker = match self.worker_for(tool) {
            Ok(worker) => worker,
            Err(error) => return (false, error.to_string()),
        };
        let Ok(folder) = tempfile::tempdir() else {
            return (false, NO_FOLDER.to_owned());
        };
        let _ = tokio::process::Command::new("git")
            .arg("init")
            .current_dir(folder.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
        let probe = run_probe(
            worker.as_ref(),
            &tool.id,
            folder.path().to_path_buf(),
            PROBE_TIMEOUT,
        )
        .await;
        verdict(&tool.name, &probe)
    }
}

/// The sentence for a tool that cannot be run at all. `None` when the probe should run.
fn unrunnable_sentence(tool: &ToolInfo) -> Option<&'static str> {
    match tool.status {
        ToolStatus::Missing => Some(MISSING),
        ToolStatus::NeedsCommand => Some(NO_COMMAND),
        ToolStatus::Ready | ToolStatus::NeedsLogin | ToolStatus::Failed => None,
    }
}

fn openrouter_verdict(answer: Result<KeyTest>) -> (bool, String) {
    match answer {
        Ok(test) => (test.ok, test.message),
        Err(error) => (false, error.to_string()),
    }
}

fn tool_test(tool: &ToolId, ok: bool, message: &str, elapsed: Duration) -> ToolTest {
    ToolTest {
        tool: tool.to_string(),
        ok,
        message: message.to_owned(),
        millis: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
    }
}

/// Run the probe prompt through one worker and wait for its outcome. The worker builds the program
/// and its environment, so a probe and a task start the agent the same way.
pub(crate) async fn run_probe(
    worker: &dyn Worker,
    agent: &ToolId,
    workdir: PathBuf,
    timeout: Duration,
) -> ProbeResult {
    let (lines, _receiver) = mpsc::unbounded_channel();
    let (budget, kill) = Budget::new(Cost::from_micro_usd(PROBE_CAP_MICRO_USD, false));
    let control = RunControl {
        kill,
        lines,
        budget,
    };
    let spec = RunSpec {
        scope: Vec::new(),
        task: TaskId::new(PROBE_TASK),
        agent: agent.clone(),
        model: String::new(),
        prompt: PROBE_PROMPT.to_owned(),
        workdir,
        timeout,
    };
    let started = Instant::now();
    let outcome = worker.run(spec, control).await;
    ProbeResult {
        outcome,
        elapsed: started.elapsed(),
    }
}

/// The one sentence the owner reads for a probe. Never holds a key.
pub(crate) fn verdict(name: &str, probe: &ProbeResult) -> (bool, String) {
    match probe.outcome.end {
        EndKind::Completed => answered(probe),
        EndKind::TimedOut => (false, TIMED_OUT.to_owned()),
        EndKind::Failed | EndKind::Killed | EndKind::CostCeiling => {
            (false, failure(name, &probe.outcome.output_tail))
        }
    }
}

fn answered(probe: &ProbeResult) -> (bool, String) {
    if reply_text(&probe.outcome.output_tail).trim().is_empty() {
        return (false, NO_REPLY.to_owned());
    }
    let seconds = probe.elapsed.as_secs_f64();
    (true, format!("Works. It answered in {seconds:.1} s."))
}

fn failure(name: &str, tail: &[String]) -> String {
    let text = reply_text(tail);
    if mentions_sign_in(&text) {
        return format!("It is not signed in. Sign in to {name} in a terminal, then test again.");
    }
    first_line(&text)
}

fn mentions_sign_in(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    SIGN_IN_MARKERS.iter().any(|marker| lower.contains(marker))
}

/// The first line of the error text, with keys hidden and cut to a fixed length.
fn first_line(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or(NO_REASON);
    mask_secrets(line).chars().take(MAX_LINE_CHARS).collect()
}

/// The answer in a tool's output. A tool run with a JSON output format wraps its answer in an
/// object with a `result` text. Anything else is the plain output.
fn reply_text(tail: &[String]) -> String {
    let text = tail.join("\n");
    let wrapped = text
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line.trim()).ok())
        .find_map(|value| value.get("result")?.as_str().map(str::to_owned));
    wrapped.unwrap_or(text)
}

/// Replace a long run of key characters that follows `key`, `token`, or `Bearer` with `...`. An
/// agent's error text can carry the key it was given, and that text reaches the owner's screen.
pub(crate) fn mask_secrets(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut ends: Vec<usize> = SECRET_MARKERS
        .iter()
        .flat_map(|marker| {
            lower
                .match_indices(*marker)
                .map(move |(at, _)| at + marker.len())
        })
        .collect();
    ends.sort_unstable();
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for end in ends {
        if end < copied {
            continue;
        }
        let start = skip_while(bytes, end, |byte| SEPARATORS.contains(&byte));
        let stop = skip_while(bytes, start, is_key_byte);
        if stop - start >= MIN_SECRET_CHARS {
            out.push_str(&text[copied..start]);
            out.push_str(MASK);
            copied = stop;
        }
    }
    out.push_str(&text[copied..]);
    out
}

fn skip_while(bytes: &[u8], from: usize, keep_going: fn(u8) -> bool) -> usize {
    let mut at = from;
    while at < bytes.len() && keep_going(bytes[at]) {
        at += 1;
    }
    at
}

fn is_key_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use futures::future::BoxFuture;
    use sentinel_core::{Tier, ToolId};
    use sentinel_workers::supervisor::{CostSource, run_command};
    use sentinel_workers::{CommandLine, RunControl, RunOutcome, RunSpec, Worker};

    use super::*;

    /// Runs a script with `sh`, the way a CLI worker runs its program.
    struct ScriptWorker {
        script: PathBuf,
    }

    impl Worker for ScriptWorker {
        fn run(&self, spec: RunSpec, ctrl: RunControl) -> BoxFuture<'_, RunOutcome> {
            Box::pin(async move {
                let command = CommandLine {
                    program: "sh".to_owned(),
                    args: vec![self.script.to_string_lossy().into_owned()],
                    env: Vec::new(),
                    stdin: None,
                    workdir: spec.workdir.clone(),
                };
                let source = CostSource {
                    tier: Tier::Cheap,
                    reported_prefix: None,
                    free: true,
                };
                run_command(command, &spec, ctrl, &source).await
            })
        }
    }

    /// Write one script into a fresh folder and run the probe through it.
    async fn probe_script(body: &str, timeout: Duration) -> (ProbeResult, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("agent.sh");
        std::fs::write(&script, body).unwrap();
        let worker = ScriptWorker { script };
        let agent = ToolId::new("test-agent");
        let probe = run_probe(&worker, &agent, dir.path().to_path_buf(), timeout).await;
        (probe, dir)
    }

    #[tokio::test]
    async fn a_working_agent_is_reported_with_its_time() {
        let (probe, _dir) = probe_script("echo ok\n", Duration::from_secs(5)).await;
        let (ok, message) = verdict("Test Agent", &probe);
        assert!(ok);
        assert!(message.starts_with("Works. It answered in "), "{message}");
        assert!(message.ends_with(" s."), "{message}");
    }

    #[tokio::test]
    async fn an_agent_that_asks_for_a_login_says_it_is_not_signed_in() {
        let (probe, _dir) =
            probe_script("echo 'please login' >&2\nexit 1\n", Duration::from_secs(5)).await;
        assert_eq!(
            verdict("Test Agent", &probe),
            (
                false,
                "It is not signed in. Sign in to Test Agent in a terminal, then test again."
                    .to_owned()
            )
        );
    }

    #[tokio::test]
    async fn a_silent_agent_is_stopped_at_the_timeout() {
        let (probe, _dir) = probe_script("sleep 5\n", Duration::from_secs(1)).await;
        assert_eq!(probe.outcome.end, EndKind::TimedOut);
        assert!(
            probe.elapsed < Duration::from_secs(4),
            "{:?}",
            probe.elapsed
        );
        assert_eq!(verdict("Test Agent", &probe), (false, TIMED_OUT.to_owned()));
    }

    #[tokio::test]
    async fn a_failure_shows_its_first_line_with_the_key_hidden() {
        let body = "echo 'bad token sk-abcdefghijklmnopqrstuvwx1234' >&2\nexit 2\n";
        let (probe, _dir) = probe_script(body, Duration::from_secs(5)).await;
        assert_eq!(
            verdict("Test Agent", &probe),
            (false, "bad token ...".to_owned())
        );
    }

    #[tokio::test]
    async fn an_exit_without_any_reply_is_not_a_working_agent() {
        let (probe, _dir) = probe_script("exit 0\n", Duration::from_secs(5)).await;
        assert_eq!(verdict("Test Agent", &probe), (false, NO_REPLY.to_owned()));
    }

    #[test]
    fn a_long_key_after_a_bearer_or_key_word_is_hidden() {
        assert_eq!(
            mask_secrets("Authorization: Bearer abcdefghijklmnopqrstuvwxyz"),
            "Authorization: Bearer ..."
        );
        assert_eq!(
            mask_secrets("api_key=ABCDEFGHIJKLMNOPQRST1234 was refused"),
            "api_key=... was refused"
        );
    }

    #[test]
    fn a_short_word_after_a_marker_stays_as_it_is() {
        assert_eq!(mask_secrets("token expired"), "token expired");
        assert_eq!(mask_secrets("no key set"), "no key set");
    }

    #[test]
    fn a_second_test_of_the_same_agent_waits_for_the_first_to_end() {
        let agent = ToolId::new("claim-test-agent");
        let first = TestClaim::take(&agent);
        assert!(first.is_some());
        assert!(TestClaim::take(&agent).is_none());
        drop(first);
        assert!(TestClaim::take(&agent).is_some());
    }
}
