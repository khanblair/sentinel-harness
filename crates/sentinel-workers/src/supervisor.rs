//! Runs one worker command: spawn, capture, cost, and stop on timeout, kill, or ceiling.

use std::io;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use sentinel_core::{Cost, TaskId, Tier, ToolId};
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::time::{self, Instant, MissedTickBehavior};

use crate::adapter_cli::{SpendSource, ceiling_applies};
use crate::budget::{Budget, BudgetEvent, estimate_by_time, guess_per_hour};
use crate::capture::{Capture, capture_streams};
use crate::worker::{CommandLine, EndKind, OutputLine, RunControl, RunOutcome, RunSpec};

const DRAIN_LIMIT: Duration = Duration::from_secs(2);
const ESTIMATE_TICK: Duration = Duration::from_secs(1);
/// A run on a time guess alone for longer than this gets one line that says so.
const GUESS_NOTICE_AFTER: Duration = Duration::from_secs(60);

/// Where the reported cost comes from, and the tier used for time estimates.
pub struct CostSource {
    pub tier: Tier,
    /// When set, a line that contains this text and then a dollar amount reports the run's
    /// total cost so far. For example `Total cost: $0.07`.
    pub reported_prefix: Option<String>,
    /// True for a model that costs nothing, such as a local model or a free tier. Time then adds
    /// no estimate, so the per-task ceiling never fires. A line that reports a cost still counts.
    pub free: bool,
}

/// Spawns `cmd`, captures its output, and stops it on the first stop condition.
pub async fn run_command(
    cmd: CommandLine,
    spec: &RunSpec,
    mut ctrl: RunControl,
    source: &CostSource,
) -> RunOutcome {
    let mut child = match launch(&cmd) {
        Ok(child) => child,
        Err(error) => return spawn_failure(&error),
    };
    let pid = child.id();
    feed_stdin(&mut child, cmd.stdin.as_deref()).await;
    let mut capture = capture_streams(&mut child, spec, ctrl.lines.clone());
    let mut spend = Spend::new(spec, &ctrl, source);

    let stop = run_until_stop(RunUntilStopParams {
        child: &mut child,
        capture: &mut capture,
        ctrl: &mut ctrl,
        spend: &mut spend,
        timeout: spec.timeout,
    })
    .await;
    terminate(&mut child, pid).await;
    capture
        .drain(DRAIN_LIMIT, |line| {
            spend.observe(line);
        })
        .await;
    let cost = spend.spent();
    build_outcome(stop, capture.into_tail(), cost)
}

#[derive(Debug, Clone, Copy)]
enum Stop {
    Exit(Option<i32>),
    TimedOut,
    Killed,
    CostCeiling,
}

struct RunContext<'a> {
    child: &'a mut Child,
    capture: &'a mut Capture,
    ctrl: &'a mut RunControl,
    spend: &'a mut Spend,
    timer: std::pin::Pin<Box<tokio::time::Sleep>>,
    ticker: tokio::time::Interval,
    watch_kill: bool,
    child_exited: bool,
    exit_code: Option<i32>,
}

struct RunContextParams<'a> {
    child: &'a mut Child,
    capture: &'a mut Capture,
    ctrl: &'a mut RunControl,
    spend: &'a mut Spend,
    timeout: Duration,
}

impl<'a> RunContextParams<'a> {
    fn build(self) -> RunContext<'a> {
        let timer = Box::pin(time::sleep(self.timeout));
        let mut ticker = time::interval_at(Instant::now() + ESTIMATE_TICK, ESTIMATE_TICK);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        RunContext {
            child: self.child,
            capture: self.capture,
            ctrl: self.ctrl,
            spend: self.spend,
            timer,
            ticker,
            watch_kill: true,
            child_exited: false,
            exit_code: None,
        }
    }
}

impl RunContext<'_> {
    async fn run(mut self) -> Stop {
        if *self.ctrl.kill.borrow() {
            return Stop::Killed;
        }
        loop {
            if let Some(stop) = self.select_once().await {
                return stop;
            }
            if self.child_exited {
                break;
            }
        }
        Stop::Exit(self.exit_code)
    }

    async fn select_once(&mut self) -> Option<Stop> {
        tokio::select! {
            status = self.child.wait(), if !self.child_exited => {
                self.child_exited = true;
                self.exit_code = status.ok().and_then(|exit| exit.code());
                None
            }
            () = &mut self.timer => Some(Stop::TimedOut),
            changed = self.ctrl.kill.changed(), if self.watch_kill => {
                self.watch_kill = changed.is_ok();
                if self.watch_kill && *self.ctrl.kill.borrow() {
                    Some(Stop::Killed)
                } else {
                    None
                }
            }
            Some(line) = self.capture.raw.recv() => {
                self.capture.push(line.clone());
                self.spend.observe(&line)
            }
            _ = self.ticker.tick() => {
                self.spend.tick();
                None
            }
        }
    }
}

struct RunUntilStopParams<'a> {
    child: &'a mut Child,
    capture: &'a mut Capture,
    ctrl: &'a mut RunControl,
    spend: &'a mut Spend,
    timeout: Duration,
}

async fn run_until_stop(params: RunUntilStopParams<'_>) -> Stop {
    RunContextParams {
        child: params.child,
        capture: params.capture,
        ctrl: params.ctrl,
        spend: params.spend,
        timeout: params.timeout,
    }
    .build()
    .run()
    .await
}

fn launch(cmd: &CommandLine) -> io::Result<Child> {
    let mut command = Command::new(&cmd.program);
    command
        .args(&cmd.args)
        .envs(cmd.env.iter().map(|(key, value)| (key, value)))
        .current_dir(&cmd.workdir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    command.spawn()
}

fn spawn_failure(error: &io::Error) -> RunOutcome {
    RunOutcome {
        end: EndKind::Failed,
        exit_code: None,
        output_tail: vec![error.to_string()],
        cost: Cost::ZERO,
    }
}

async fn feed_stdin(child: &mut Child, text: Option<&str>) {
    let Some(mut stdin) = child.stdin.take() else {
        return;
    };
    if let Some(text) = text {
        let _ = stdin.write_all(text.as_bytes()).await;
    }
    let _ = stdin.shutdown().await;
}

struct Spend {
    task: TaskId,
    agent: ToolId,
    budget: Arc<Budget>,
    lines: mpsc::UnboundedSender<OutputLine>,
    tier: Tier,
    prefix: Option<String>,
    source: SpendSource,
    /// Reported spend. It counts toward the task ceiling.
    recorded: Cost,
    /// Time guesses. They count toward the cap, never toward the task ceiling.
    guessed: Cost,
    guess_elapsed: Duration,
    guess_noticed: bool,
}

impl Spend {
    fn new(spec: &RunSpec, ctrl: &RunControl, source: &CostSource) -> Self {
        let source_kind = if source.free {
            SpendSource::Free
        } else {
            SpendSource::TimeGuess
        };
        Self {
            task: spec.task.clone(),
            agent: spec.agent.clone(),
            budget: ctrl.budget.clone(),
            lines: ctrl.lines.clone(),
            tier: source.tier,
            prefix: source.reported_prefix.clone(),
            source: source_kind,
            recorded: Cost::ZERO,
            guessed: Cost::ZERO,
            guess_elapsed: Duration::ZERO,
            guess_noticed: false,
        }
    }

    /// Everything the run has cost so far, reported or guessed.
    fn spent(&self) -> Cost {
        self.recorded + self.guessed
    }

    fn observe(&mut self, line: &str) -> Option<Stop> {
        let total = self.reported_amount(line)?;
        let delta = total.saturating_sub(self.recorded);
        self.source = SpendSource::Reported;
        self.charge(delta)
    }

    fn reported_amount(&self, line: &str) -> Option<Cost> {
        let prefix = self.prefix.as_deref()?;
        let rest = line.split_once(prefix)?.1;
        Cost::parse_usd(cut_decimals(leading_amount(rest)?)).ok()
    }

    /// Adds the time guess while the run has no real cost. A free or reported run adds nothing.
    fn tick(&mut self) {
        if self.source != SpendSource::TimeGuess {
            return;
        }
        self.guess_elapsed += ESTIMATE_TICK;
        if !self.guess_noticed && self.guess_elapsed > GUESS_NOTICE_AFTER {
            self.notice_guess();
        }
        let guess = estimate_by_time(self.tier, ESTIMATE_TICK);
        self.guessed = self.guessed + guess;
        self.budget.record_guess(guess);
    }

    fn notice_guess(&mut self) {
        self.guess_noticed = true;
        let text = format!(
            "{} reports no cost, so Sentinel counts a guess of {} an hour. The task timeout still applies.",
            self.agent.as_str(),
            guess_per_hour(self.tier),
        );
        let _ = self.lines.send(OutputLine {
            task: self.task.clone(),
            agent: self.agent.clone(),
            text,
        });
    }

    fn charge(&mut self, cost: Cost) -> Option<Stop> {
        let events = self.budget.record(&self.task, cost);
        self.recorded = self.recorded + cost;
        let ceiling_hit = events.iter().any(|event| ceiling_for(event, &self.task));
        (ceiling_applies(self.source) && ceiling_hit).then_some(Stop::CostCeiling)
    }
}

/// At most six decimals, the finest unit of a cost. A tool that reports `0.00630725` still counts.
fn cut_decimals(amount: &str) -> &str {
    match amount.split_once('.') {
        Some((whole, fraction)) if fraction.len() > 6 => &amount[..whole.len() + 1 + 6],
        _ => amount,
    }
}

fn leading_amount(text: &str) -> Option<&str> {
    let text = text.trim_start().trim_start_matches('$');
    let mut seen_dot = false;
    let end = text
        .char_indices()
        .find_map(|(index, character)| {
            let allowed = character.is_ascii_digit() || (character == '.' && !seen_dot);
            seen_dot |= character == '.';
            (!allowed).then_some(index)
        })
        .unwrap_or(text.len());
    let amount = &text[..end];
    amount
        .bytes()
        .any(|byte| byte.is_ascii_digit())
        .then_some(amount)
}

fn ceiling_for(event: &BudgetEvent, task: &TaskId) -> bool {
    matches!(event, BudgetEvent::CeilingHit { task: hit, .. } if hit == task)
}

fn build_outcome(stop: Stop, tail: Vec<String>, cost: Cost) -> RunOutcome {
    let (end, exit_code) = match stop {
        Stop::Exit(Some(0)) => (EndKind::Completed, Some(0)),
        Stop::Exit(code) => (EndKind::Failed, code),
        Stop::TimedOut => (EndKind::TimedOut, None),
        Stop::Killed => (EndKind::Killed, None),
        Stop::CostCeiling => (EndKind::CostCeiling, None),
    };
    RunOutcome {
        end,
        exit_code,
        output_tail: tail,
        cost,
    }
}

async fn terminate(child: &mut Child, pid: Option<u32>) {
    kill_tree(pid);
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[cfg(unix)]
fn kill_tree(pid: Option<u32>) {
    let Some(pid) = pid else {
        return;
    };
    let _ = std::process::Command::new("kill")
        .arg("-KILL")
        .arg("--")
        .arg(format!("-{pid}"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(windows)]
fn kill_tree(pid: Option<u32>) {
    let Some(pid) = pid else {
        return;
    };
    let _ = std::process::Command::new("taskkill")
        .args(["/T", "/F", "/PID"])
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{MAX_LINE_CHARS, TAIL_LINES};
    use crate::worker::OutputLine;
    use std::path::Path;
    use tokio::sync::{mpsc, watch};

    fn usd(text: &str) -> Cost {
        Cost::parse_usd(text).expect("amount")
    }

    fn spec(timeout: Duration) -> RunSpec {
        RunSpec {
            scope: Vec::new(),
            task: TaskId::new("T-01"),
            agent: sentinel_core::ToolId::new("sh"),
            model: String::new(),
            prompt: String::new(),
            workdir: std::path::PathBuf::from("."),
            timeout,
        }
    }

    fn control(cap: &str) -> (RunControl, mpsc::UnboundedReceiver<OutputLine>) {
        let (budget, kill) = Budget::new(usd(cap));
        let (lines, received) = mpsc::unbounded_channel();
        (
            RunControl {
                kill,
                lines,
                budget,
            },
            received,
        )
    }

    fn sh(script: &str) -> CommandLine {
        CommandLine {
            program: "sh".into(),
            args: vec!["-c".into(), script.into()],
            env: Vec::new(),
            stdin: None,
            workdir: std::path::PathBuf::from("."),
        }
    }

    fn cost_source(prefix: Option<&str>) -> CostSource {
        CostSource {
            tier: Tier::Cheap,
            reported_prefix: prefix.map(str::to_owned),
            free: false,
        }
    }

    fn read_pid(path: &Path) -> u32 {
        std::fs::read_to_string(path)
            .expect("pid file")
            .trim()
            .parse()
            .expect("pid")
    }

    fn process_alive(pid: u32) -> bool {
        std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .status()
            .is_ok_and(|status| status.success())
    }

    async fn wait_until_gone(pid: u32) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while process_alive(pid) {
            assert!(Instant::now() < deadline, "pid {pid} is still alive");
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test]
    async fn three_lines_are_captured_and_sent() {
        let (ctrl, mut received) = control("10.00");
        let out = run_command(
            sh("echo one; echo two; echo three"),
            &spec(Duration::from_secs(5)),
            ctrl,
            &cost_source(None),
        )
        .await;
        assert_eq!(out.end, EndKind::Completed);
        assert_eq!(out.output_tail, ["one", "two", "three"]);
        let mut sent = Vec::new();
        while let Ok(line) = received.try_recv() {
            sent.push(line.text);
        }
        assert_eq!(sent, ["one", "two", "three"]);
    }

    #[tokio::test]
    async fn exit_code_three_is_failed() {
        let (ctrl, _received) = control("10.00");
        let out = run_command(
            sh("exit 3"),
            &spec(Duration::from_secs(5)),
            ctrl,
            &cost_source(None),
        )
        .await;
        assert_eq!(out.end, EndKind::Failed);
        assert_eq!(out.exit_code, Some(3));
    }

    #[tokio::test]
    async fn missing_program_fails_with_the_io_error() {
        let (ctrl, _received) = control("10.00");
        let mut cmd = sh("true");
        cmd.program = "sentinel-no-such-program-p3a".into();
        let out = run_command(cmd, &spec(Duration::from_secs(5)), ctrl, &cost_source(None)).await;
        assert_eq!(out.end, EndKind::Failed);
        assert_eq!(out.exit_code, None);
        assert_eq!(out.output_tail.len(), 1);
        assert!(out.output_tail[0].to_lowercase().contains("no such file"));
    }

    #[tokio::test]
    async fn timeout_kills_the_child() {
        let dir = tempfile::tempdir().expect("dir");
        let pid_file = dir.path().join("pid");
        let mut cmd = sh("echo $$ > \"$PID_FILE\"; sleep 30");
        cmd.env
            .push(("PID_FILE".into(), pid_file.display().to_string()));
        let (ctrl, _received) = control("10.00");
        let out = run_command(cmd, &spec(Duration::from_secs(1)), ctrl, &cost_source(None)).await;
        assert_eq!(out.end, EndKind::TimedOut);
        wait_until_gone(read_pid(&pid_file)).await;
    }

    #[tokio::test]
    async fn timeout_kills_the_grandchild() {
        let dir = tempfile::tempdir().expect("dir");
        let pid_file = dir.path().join("grandchild");
        let mut cmd = sh("sleep 30 & echo $! > \"$PID_FILE\"; wait");
        cmd.env
            .push(("PID_FILE".into(), pid_file.display().to_string()));
        let (ctrl, _received) = control("10.00");
        let out = run_command(cmd, &spec(Duration::from_secs(1)), ctrl, &cost_source(None)).await;
        assert_eq!(out.end, EndKind::TimedOut);
        wait_until_gone(read_pid(&pid_file)).await;
    }

    #[tokio::test]
    async fn kill_signal_stops_the_child() {
        let (budget, _) = Budget::new(usd("10.00"));
        let (kill_tx, kill_rx) = watch::channel(false);
        let (lines, _received) = mpsc::unbounded_channel();
        let ctrl = RunControl {
            kill: kill_rx,
            lines,
            budget,
        };
        let sender = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            let _ = kill_tx.send(true);
        });
        let run_spec = spec(Duration::from_secs(30));
        let source = cost_source(None);
        let run = run_command(sh("sleep 30"), &run_spec, ctrl, &source);
        let out = time::timeout(Duration::from_secs(3), run)
            .await
            .expect("killed in time");
        sender.await.expect("sender");
        assert_eq!(out.end, EndKind::Killed);
    }

    #[tokio::test]
    async fn budget_95_percent_kills_the_child() {
        let (ctrl, _received) = control("0.10");
        let budget = ctrl.budget.clone();
        let out = run_command(
            sh("echo 'Total cost: $0.096'; sleep 30"),
            &spec(Duration::from_secs(30)),
            ctrl,
            &cost_source(Some("Total cost: $")),
        )
        .await;
        assert_eq!(out.end, EndKind::Killed);
        assert_eq!(budget.spent(), usd("0.096"));
    }

    #[tokio::test]
    async fn reported_cost_replaces_the_total() {
        let (ctrl, _received) = control("10.00");
        let budget = ctrl.budget.clone();
        let out = run_command(
            sh("echo 'Total cost: $0.02'; echo 'Total cost: $0.07'"),
            &spec(Duration::from_secs(5)),
            ctrl,
            &cost_source(Some("Total cost: $")),
        )
        .await;
        assert_eq!(out.cost, usd("0.07"));
        assert!(!out.cost.is_estimated());
        assert_eq!(budget.spent(), usd("0.07"));
    }

    #[tokio::test]
    async fn a_json_cost_with_many_decimals_is_read() {
        let (ctrl, _received) = control("10.00");
        let out = run_command(
            sh(r#"echo '{"type":"result","total_cost_usd":0.00630725,"usage":{}}'"#),
            &spec(Duration::from_secs(5)),
            ctrl,
            &cost_source(Some("\"total_cost_usd\":")),
        )
        .await;
        assert_eq!(out.cost, usd("0.006307"));
        assert!(!out.cost.is_estimated());
    }

    #[tokio::test]
    async fn ceiling_ends_the_run() {
        let task = TaskId::new("T-01");
        let (ctrl, _received) = control("10.00");
        ctrl.budget.set_ceiling(&task, usd("0.05"));
        let out = run_command(
            sh("echo 'Total cost: $0.08'; sleep 30"),
            &spec(Duration::from_secs(30)),
            ctrl,
            &cost_source(Some("Total cost: $")),
        )
        .await;
        assert_eq!(out.end, EndKind::CostCeiling);
    }

    #[tokio::test]
    async fn stdin_is_written_to_the_child() {
        let mut cmd = sh("cat");
        cmd.stdin = Some("hello\n".into());
        let (ctrl, _received) = control("10.00");
        let out = run_command(cmd, &spec(Duration::from_secs(5)), ctrl, &cost_source(None)).await;
        assert_eq!(out.end, EndKind::Completed);
        assert_eq!(out.output_tail, ["hello"]);
    }

    #[tokio::test]
    async fn flood_output_keeps_only_the_last_200_lines() {
        let (ctrl, _received) = control("10.00");
        let script = "i=0; while [ $i -lt 100000 ]; do echo \"line $i\"; i=$((i+1)); done";
        let out = run_command(
            sh(script),
            &spec(Duration::from_secs(30)),
            ctrl,
            &cost_source(None),
        )
        .await;
        assert_eq!(out.end, EndKind::Completed);
        assert_eq!(out.output_tail.len(), TAIL_LINES);
        assert_eq!(
            out.output_tail.last().map(String::as_str),
            Some("line 99999")
        );
    }

    #[tokio::test]
    async fn a_long_line_is_cut_for_the_stream_and_kept_whole_in_the_tail() {
        let (ctrl, mut received) = control("10.00");
        let out = run_command(
            sh("printf '%10000s' '' | tr ' ' x; echo"),
            &spec(Duration::from_secs(5)),
            ctrl,
            &cost_source(None),
        )
        .await;
        assert_eq!(out.output_tail.len(), 1);
        assert_eq!(out.output_tail[0].chars().count(), 10_000);
        let shown = received.recv().await.expect("the stream got the line");
        assert_eq!(shown.text.chars().count(), MAX_LINE_CHARS);
    }

    fn spend_on(ctrl: &RunControl, source: &CostSource) -> Spend {
        Spend::new(&spec(Duration::from_secs(60)), ctrl, source)
    }

    fn free_source() -> CostSource {
        CostSource {
            tier: Tier::Cheap,
            reported_prefix: None,
            free: true,
        }
    }

    #[test]
    fn a_free_run_never_accrues_time_cost() {
        let (ctrl, _received) = control("5.00");
        let mut spend = spend_on(&ctrl, &free_source());
        for _ in 0..30 {
            spend.tick();
        }
        assert_eq!(spend.spent(), Cost::ZERO);
        assert_eq!(ctrl.budget.spent(), Cost::ZERO);
    }

    #[test]
    fn a_time_guess_over_the_task_ceiling_emits_no_ceiling_event() {
        let task = TaskId::new("T-01");
        let (ctrl, _received) = control("10.00");
        ctrl.budget.set_ceiling(&task, usd("0.05"));
        let mut spend = spend_on(&ctrl, &cost_source(None));
        // 400 seconds at 150 micro-dollars a second is $0.06, over the $0.05 ceiling.
        for _ in 0..400 {
            spend.tick();
        }
        assert!(ctrl.budget.spent() > usd("0.05"));
        assert_eq!(ctrl.budget.drain_events().len(), 0);
        assert_eq!(spend.source, SpendSource::TimeGuess);
    }

    #[test]
    fn a_real_cost_after_a_guess_is_judged_on_its_own() {
        let task = TaskId::new("T-01");
        let (ctrl, _received) = control("10.00");
        ctrl.budget.set_ceiling(&task, usd("0.05"));
        let mut spend = spend_on(&ctrl, &cost_source(Some("Total cost: $")));
        for _ in 0..400 {
            spend.tick();
        }
        assert!(spend.observe("Total cost: $0.01").is_none());
        assert!(spend.spent() > usd("0.05"));
    }

    #[test]
    fn a_reported_run_over_the_task_ceiling_is_stopped() {
        let task = TaskId::new("T-01");
        let (ctrl, _received) = control("10.00");
        ctrl.budget.set_ceiling(&task, usd("0.05"));
        let mut spend = spend_on(&ctrl, &cost_source(Some("Total cost: $")));
        assert!(matches!(
            spend.observe("Total cost: $0.08"),
            Some(Stop::CostCeiling)
        ));
    }

    #[test]
    fn a_real_cost_line_moves_the_source_from_guess_to_reported() {
        let (ctrl, _received) = control("10.00");
        let mut spend = spend_on(&ctrl, &cost_source(Some("Total cost: $")));
        spend.tick();
        assert_eq!(spend.source, SpendSource::TimeGuess);
        assert!(spend.observe("Total cost: $0.02").is_none());
        assert_eq!(spend.source, SpendSource::Reported);
        let before = spend.spent();
        spend.tick();
        assert_eq!(spend.spent(), before);
    }

    #[test]
    fn a_guess_over_a_minute_sends_one_notice_line() {
        let (ctrl, mut received) = control("10.00");
        let mut spend = spend_on(&ctrl, &cost_source(None));
        for _ in 0..90 {
            spend.tick();
        }
        let mut lines = Vec::new();
        while let Ok(line) = received.try_recv() {
            lines.push(line.text);
        }
        assert_eq!(
            lines,
            [
                "sh reports no cost, so Sentinel counts a guess of $0.54 an hour. The task timeout still applies."
            ]
        );
    }
}
