//! `sentinel selftest`: runs the real engine on a throwaway repository with scripted agents that
//! misbehave on purpose, then prints one PASS or FAIL line per scenario. It needs no real agent,
//! no network, and no money.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use sentinel_core::api::RunRequest;
use sentinel_core::{Event, HarnessState, TaskStatus};
use sentinel_daemon::cmd::SystemRunner;
use sentinel_daemon::engine::{Engine, EngineOptions, FakeAgents};
use sentinel_daemon::secrets::MemoryStore;

/// A scenario that runs longer than this is a FAIL, never a hang.
const SCENARIO_TIMEOUT: Duration = Duration::from_secs(60);
/// Large enough to hold every event of one scenario run.
const EVENT_LIMIT: u32 = 100_000;
const BACKLOG_FILE: &str = "backlog.md";
const TEST_SCRIPT: &str = "#!/bin/sh\nexit 0\n";
const BETA_ORIGINAL: &str = "beta original";

/// The files of the throwaway repository. A scenario's backlog is added on top of them.
/// `.gitignore` keeps the engine's `.sentinel` folder out of the clean-tree check.
const REPO_FILES: [(&str, &str); 6] = [
    ("src/alpha.txt", "alpha original\n"),
    ("src/beta.txt", "beta original\n"),
    ("src/gamma.txt", "gamma original\n"),
    ("tests/run.sh", TEST_SCRIPT),
    ("README.md", "# selftest repo\n"),
    (".gitignore", ".sentinel/\n"),
];

const FAKE_AGENT: &str = include_str!("../../../fixtures/selftest/fake-agent");

/// `Ok` carries the detail for the PASS line. `Err` carries the reason for the FAIL line.
type Check = Result<String, String>;

/// One bad agent: its backlog and scripted plans, and what the finished run must show.
struct Scenario {
    name: &'static str,
    backlog: &'static str,
    plan_json: &'static str,
    /// Each scripted task as (file name, text). The fake agent reads `T-NN.fake` from here.
    plans: &'static [(&'static str, &'static str)],
    expect: fn(&Outcome) -> Check,
}

/// What a finished run left behind. The expectations read only this, so they run without an engine.
struct Outcome {
    /// Each task and the status it ended with.
    statuses: Vec<(String, TaskStatus)>,
    /// The task and the reason of each verdict the run gave.
    reasons: Vec<(String, String)>,
    /// Every event the run stored, in order.
    events_seen: Vec<Event>,
    /// The path and text of each file of the repository after the run.
    files: Vec<(String, String)>,
}

impl Outcome {
    fn status(&self, task: &str) -> Option<TaskStatus> {
        self.statuses
            .iter()
            .find(|(id, _)| id == task)
            .map(|(_, status)| *status)
    }

    fn file(&self, path: &str) -> Option<&str> {
        self.files
            .iter()
            .find(|(name, _)| name == path)
            .map(|(_, text)| text.as_str())
    }

    fn saw(&self, wanted: impl Fn(&Event) -> bool) -> bool {
        self.events_seen.iter().any(wanted)
    }

    fn reason_contains(&self, task: &str, text: &str) -> bool {
        self.reasons
            .iter()
            .any(|(id, reason)| id == task && reason.contains(text))
    }

    fn wrote_blocked(&self, path: &str) -> bool {
        self.saw(|event| {
            matches!(event, Event::WriteBlocked { path: blocked, .. } if blocked == path)
        })
    }

    fn started(&self, task: &str) -> bool {
        self.saw(|event| {
            matches!(
                event,
                Event::TaskStatus { task: id, status: TaskStatus::Running }
                    if id.to_string() == task
            )
        })
    }
}

fn expect_collider(outcome: &Outcome) -> Check {
    if !outcome.wrote_blocked("src/beta.txt") {
        return Err("no write to src/beta.txt was blocked".to_owned());
    }
    match outcome.file("src/beta.txt") {
        Some(text) if text.trim_end() == BETA_ORIGINAL => {
            Ok("a write outside its files was blocked and restored".to_owned())
        }
        Some(text) => Err(format!("src/beta.txt still holds {:?}", text.trim_end())),
        None => Err("src/beta.txt is missing after the run".to_owned()),
    }
}

fn expect_tamperer(outcome: &Outcome) -> Check {
    if outcome.status("T-01") == Some(TaskStatus::Verified) {
        return Err("T-01 was verified after it edited a protected file".to_owned());
    }
    if !outcome.reason_contains("T-01", "tamper") {
        return Err("no verdict on T-01 names the tamper".to_owned());
    }
    match outcome.file("tests/run.sh") {
        Some(TEST_SCRIPT) => {
            Ok("a protected test file was edited and the task was not verified".to_owned())
        }
        Some(_) => Err("tests/run.sh still holds the agent's edit".to_owned()),
        None => Err("tests/run.sh is missing after the run".to_owned()),
    }
}

fn expect_liar(outcome: &Outcome) -> Check {
    if outcome.status("T-01") == Some(TaskStatus::Verified) {
        return Err("T-01 was verified although it changed nothing".to_owned());
    }
    let checkpointed = outcome.saw(|event| {
        matches!(event, Event::Checkpoint { task, .. } if task.to_string() == "T-01")
    });
    if checkpointed {
        return Err("T-01 was checkpointed without any change".to_owned());
    }
    Ok("a claim of done with no change was not verified".to_owned())
}

/// The run reports the cap through a budget threshold, a task ceiling, or the 70 percent stop.
fn reported_cap(outcome: &Outcome) -> bool {
    outcome.saw(|event| {
        matches!(event, Event::BudgetThreshold { .. } | Event::TaskCeilingHit { .. })
            || matches!(event, Event::Message { text, .. } if text.contains("of the cap is spent"))
    })
}

fn expect_overspender(outcome: &Outcome) -> Check {
    if !reported_cap(outcome) {
        return Err("the run never reported that the cap was spent".to_owned());
    }
    if outcome.started("T-02") {
        return Err("T-02 started after the cap was spent".to_owned());
    }
    Ok("the run stopped on the cap and the next task never started".to_owned())
}

fn expect_crasher(outcome: &Outcome) -> Check {
    let failing = outcome.status("T-01");
    if !matches!(failing, Some(TaskStatus::Blocked | TaskStatus::Failed | TaskStatus::Skipped)) {
        return Err(format!("T-01 ended as {failing:?}, not blocked, failed, or skipped"));
    }
    let other = outcome.status("T-02");
    if other != Some(TaskStatus::Verified) {
        return Err(format!("T-02 ended as {other:?}, not verified"));
    }
    Ok("the failing task ended blocked and the other task still verified".to_owned())
}

/// Every scenario, in the order the command prints them.
fn scenarios() -> Vec<Scenario> {
    vec![collider(), tamperer(), liar(), overspender(), crasher()]
}

// Each agent misbehaves once, on its own backlog. T-NN.fake files are the scripts.

fn collider() -> Scenario {
    // UNSURE: the watcher may restore src/beta.txt during the wave, or only the barrier does. Both
    // report WriteBlocked for the path, so the check accepts either.
    Scenario {
        name: "collider",
        backlog: include_str!("../../../fixtures/selftest/collider/backlog.md"),
        plan_json: include_str!("../../../fixtures/selftest/collider/plan.json"),
        plans: &[
            ("T-01.fake", include_str!("../../../fixtures/selftest/collider/T-01.fake")),
            ("T-02.fake", include_str!("../../../fixtures/selftest/collider/T-02.fake")),
        ],
        expect: expect_collider,
    }
}

fn tamperer() -> Scenario {
    // UNSURE: the planner may refuse a scope that names a protected file. Then the run never
    // reaches the tamper check and this scenario reports FAIL with no tamper reason.
    Scenario {
        name: "tamperer",
        backlog: include_str!("../../../fixtures/selftest/tamperer/backlog.md"),
        plan_json: include_str!("../../../fixtures/selftest/tamperer/plan.json"),
        plans: &[(
            "T-01.fake",
            include_str!("../../../fixtures/selftest/tamperer/T-01.fake"),
        )],
        expect: expect_tamperer,
    }
}

fn liar() -> Scenario {
    Scenario {
        name: "liar",
        backlog: include_str!("../../../fixtures/selftest/liar/backlog.md"),
        plan_json: include_str!("../../../fixtures/selftest/liar/plan.json"),
        plans: &[("T-01.fake", include_str!("../../../fixtures/selftest/liar/T-01.fake"))],
        expect: expect_liar,
    }
}

fn overspender() -> Scenario {
    // UNSURE: which cap event comes first depends on the timing of the cost line and the kill
    // signal. The check accepts any of the three forms the engine can emit.
    Scenario {
        name: "overspender",
        backlog: include_str!("../../../fixtures/selftest/overspender/backlog.md"),
        plan_json: include_str!("../../../fixtures/selftest/overspender/plan.json"),
        plans: &[
            ("T-01.fake", include_str!("../../../fixtures/selftest/overspender/T-01.fake")),
            ("T-02.fake", include_str!("../../../fixtures/selftest/overspender/T-02.fake")),
        ],
        expect: expect_overspender,
    }
}

fn crasher() -> Scenario {
    Scenario {
        name: "crasher",
        backlog: include_str!("../../../fixtures/selftest/crasher/backlog.md"),
        plan_json: include_str!("../../../fixtures/selftest/crasher/plan.json"),
        plans: &[
            ("T-01.fake", include_str!("../../../fixtures/selftest/crasher/T-01.fake")),
            ("T-02.fake", include_str!("../../../fixtures/selftest/crasher/T-02.fake")),
        ],
        expect: expect_crasher,
    }
}

/// `sentinel selftest`: run every scenario, or the one named by `only`, and print a line each.
pub(crate) async fn run(only: Option<&str>) -> anyhow::Result<ExitCode> {
    let chosen = choose(only)?;
    let total = chosen.len();
    let mut passed = 0;
    for scenario in chosen {
        let name = scenario.name;
        // A panic in the engine ends this task, not the command. It becomes a FAIL line.
        let check = match tokio::spawn(async move { check_scenario(&scenario).await }).await {
            Ok(check) => check,
            Err(error) => Err(format!("panicked: {error}")),
        };
        if print_line(name, check) {
            passed += 1;
        }
    }
    println!("{passed} of {total} scenarios passed");
    Ok(if passed == total { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

/// Runs one scenario and turns every outcome into a check. A timeout is a FAIL, never a hang.
async fn check_scenario(scenario: &Scenario) -> Check {
    match tokio::time::timeout(SCENARIO_TIMEOUT, run_scenario(scenario)).await {
        Ok(Ok(outcome)) => (scenario.expect)(&outcome),
        Ok(Err(error)) => Err(format!("{error:#}")),
        Err(_) => Err(format!("timed out after {} seconds", SCENARIO_TIMEOUT.as_secs())),
    }
}

/// Prints the PASS or FAIL line and says whether the scenario passed.
fn print_line(name: &str, check: Check) -> bool {
    match check {
        Ok(detail) => {
            println!("PASS {name}: {detail}");
            true
        }
        Err(reason) => {
            println!("FAIL {name}: {reason}");
            false
        }
    }
}

fn choose(only: Option<&str>) -> anyhow::Result<Vec<Scenario>> {
    let all = scenarios();
    let Some(name) = only else {
        return Ok(all);
    };
    let names: Vec<&str> = all.iter().map(|scenario| scenario.name).collect();
    if !names.contains(&name) {
        bail!(
            "no selftest scenario named {name}. The names are: {}",
            names.join(", ")
        );
    }
    Ok(all
        .into_iter()
        .filter(|scenario| scenario.name == name)
        .collect())
}

/// One scenario on its own repository and engine. Its folder is removed when it ends.
async fn run_scenario(scenario: &Scenario) -> anyhow::Result<Outcome> {
    let scratch = Scratch::new(scenario.name)?;
    let repo = scratch.0.join("repo");
    let plans = scratch.0.join("plans");
    let script = scratch.0.join("fake-agent");
    write_repo(&repo, scenario.backlog)?;
    write_plans(&plans, scenario)?;
    write_script(&script)?;
    let engine = selftest_engine(&repo, script, plans)?;
    engine.scan_tools().await?;
    engine
        .start_run(RunRequest {
            backlog: Some(BACKLOG_FILE.to_owned()),
            ..RunRequest::default()
        })
        .await?;
    wait_until_done(&engine).await;
    collect_outcome(&engine, &repo)
}

/// An engine with the scripted agents, the cap, and short waits for answers no one gives.
fn selftest_engine(repo: &Path, script: PathBuf, plans: PathBuf) -> anyhow::Result<Engine> {
    let engine = Engine::open(EngineOptions {
        repo_root: repo.to_path_buf(),
        home: repo.to_path_buf(),
        runner: Arc::new(SystemRunner),
        secrets: Arc::new(MemoryStore::new()),
        fake_agents: Some(FakeAgents {
            script,
            plans_dir: plans,
        }),
        in_memory: true,
    })?;
    engine.update_config(|c| {
        "1.00".clone_into(&mut c.cap_usd);
        c.test_command = vec!["sh".to_owned(), "tests/run.sh".to_owned()];
        c.protected_paths = vec!["tests/".to_owned()];
        c.task_timeout_seconds = 20;
        c.max_parallel = 3;
        c.decision_timeout_seconds = 2;
        c.approval_timeout_seconds = 2;
    })?;
    Ok(engine)
}

async fn wait_until_done(engine: &Engine) {
    while engine.harness_state() != HarnessState::Done {
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

fn collect_outcome(engine: &Engine, repo: &Path) -> anyhow::Result<Outcome> {
    let statuses = engine
        .store()
        .list_tasks()?
        .into_iter()
        .map(|task| (task.id.to_string(), task.status))
        .collect();
    let events_seen: Vec<Event> = engine
        .events_after(0, EVENT_LIMIT)?
        .into_iter()
        .map(|record| record.event)
        .collect();
    let reasons = events_seen.iter().filter_map(verdict_reason).collect();
    let files = REPO_FILES
        .iter()
        .map(|(path, _)| read_repo_file(repo, path))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(Outcome {
        statuses,
        reasons,
        events_seen,
        files,
    })
}

fn verdict_reason(event: &Event) -> Option<(String, String)> {
    let Event::Verdict { task, reason, .. } = event else {
        return None;
    };
    Some((task.to_string(), reason.clone()))
}

fn read_repo_file(repo: &Path, path: &str) -> anyhow::Result<(String, String)> {
    let text = fs::read_to_string(repo.join(path))
        .with_context(|| format!("could not read {path}"))?;
    Ok((path.to_owned(), text))
}

/// The throwaway repository: the base files and the backlog, committed on `main`.
fn write_repo(repo: &Path, backlog: &str) -> anyhow::Result<()> {
    for (path, text) in REPO_FILES {
        write_file(&repo.join(path), text)?;
    }
    write_file(&repo.join(BACKLOG_FILE), backlog)?;
    git(repo, &["init", "-b", "main"])?;
    git(repo, &["config", "user.name", "Sentinel selftest"])?;
    git(repo, &["config", "user.email", "selftest@example.com"])?;
    git(repo, &["add", "-A"])?;
    git(repo, &["commit", "-m", "start"])
}

/// The scripted plans sit outside the repository, so they never show up as changes.
fn write_plans(plans: &Path, scenario: &Scenario) -> anyhow::Result<()> {
    write_file(&plans.join("plan.json"), scenario.plan_json)?;
    for (name, text) in scenario.plans {
        write_file(&plans.join(name), text)?;
    }
    Ok(())
}

fn write_script(path: &Path) -> anyhow::Result<()> {
    write_file(path, FAKE_AGENT)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .context("could not make the fake agent runnable")
}

fn write_file(path: &Path, text: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text).with_context(|| format!("could not write {}", path.display()))
}

fn git(repo: &Path, args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .context("could not start git")?;
    if output.status.success() {
        return Ok(());
    }
    bail!(
        "git {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

/// A folder under the system temp folder, removed when the scenario ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> anyhow::Result<Self> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "sentinel-selftest-{name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path)
            .with_context(|| format!("could not make {}", path.display()))?;
        Ok(Self(fs::canonicalize(&path)?))
    }

}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hand-built outcome. Tests add events or reasons when a check needs them.
    fn outcome(statuses: &[(&str, TaskStatus)], files: &[(&str, &str)]) -> Outcome {
        Outcome {
            statuses: statuses
                .iter()
                .map(|(id, status)| ((*id).to_owned(), *status))
                .collect(),
            reasons: Vec::new(),
            events_seen: Vec::new(),
            files: files
                .iter()
                .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
                .collect(),
        }
    }

    fn blocked_beta() -> Event {
        Event::WriteBlocked {
            task: None,
            path: "src/beta.txt".to_owned(),
            restored: true,
        }
    }

    fn cap_event() -> Event {
        Event::BudgetThreshold { percent: 95 }
    }

    #[test]
    fn collider_passes_when_the_write_is_blocked_and_beta_is_restored() {
        let mut good = outcome(&[], &[("src/beta.txt", "beta original\n")]);
        good.events_seen.push(blocked_beta());
        assert!(expect_collider(&good).is_ok());
    }

    #[test]
    fn collider_fails_when_beta_keeps_the_write_of_the_other_task() {
        let mut bad = outcome(&[], &[("src/beta.txt", "beta changed by T-01\n")]);
        bad.events_seen.push(blocked_beta());
        assert!(expect_collider(&bad).is_err());
    }

    #[test]
    fn collider_fails_when_no_write_was_blocked() {
        let quiet = outcome(&[], &[("src/beta.txt", "beta original\n")]);
        assert!(expect_collider(&quiet).is_err());
    }

    #[test]
    fn tamperer_passes_when_the_edit_is_caught_and_the_script_restored() {
        let mut good = outcome(
            &[("T-01", TaskStatus::Blocked)],
            &[("tests/run.sh", TEST_SCRIPT)],
        );
        good.reasons.push((
            "T-01".to_owned(),
            "tampered with a protected file: tests/run.sh".to_owned(),
        ));
        assert!(expect_tamperer(&good).is_ok());
    }

    #[test]
    fn tamperer_fails_when_the_task_is_verified() {
        let mut bad = outcome(
            &[("T-01", TaskStatus::Verified)],
            &[("tests/run.sh", TEST_SCRIPT)],
        );
        bad.reasons
            .push(("T-01".to_owned(), "tampered with a protected file".to_owned()));
        assert!(expect_tamperer(&bad).is_err());
    }

    #[test]
    fn tamperer_fails_when_the_edited_script_stays() {
        let mut bad = outcome(
            &[("T-01", TaskStatus::Blocked)],
            &[("tests/run.sh", "#!/bin/sh\nexit 0\n# edited\n")],
        );
        bad.reasons
            .push(("T-01".to_owned(), "tampered with a protected file".to_owned()));
        assert!(expect_tamperer(&bad).is_err());
    }

    #[test]
    fn liar_passes_when_a_claim_of_done_is_not_verified() {
        let good = outcome(&[("T-01", TaskStatus::Blocked)], &[]);
        assert!(expect_liar(&good).is_ok());
    }

    #[test]
    fn liar_fails_when_the_claim_of_done_is_verified() {
        let bad = outcome(&[("T-01", TaskStatus::Verified)], &[]);
        assert!(expect_liar(&bad).is_err());
    }

    #[test]
    fn overspender_passes_when_the_cap_stops_the_run_before_the_next_task() {
        let mut good = outcome(&[("T-02", TaskStatus::Ready)], &[]);
        good.events_seen.push(cap_event());
        assert!(expect_overspender(&good).is_ok());
    }

    #[test]
    fn overspender_fails_when_the_next_task_starts_after_the_cap() {
        let mut bad = outcome(&[("T-02", TaskStatus::Running)], &[]);
        bad.events_seen.push(cap_event());
        bad.events_seen.push(Event::TaskStatus {
            task: sentinel_core::TaskId::new("T-02"),
            status: TaskStatus::Running,
        });
        assert!(expect_overspender(&bad).is_err());
    }

    #[test]
    fn crasher_passes_when_the_failing_task_ends_blocked_and_the_other_verifies() {
        let good = outcome(
            &[("T-01", TaskStatus::Blocked), ("T-02", TaskStatus::Verified)],
            &[],
        );
        assert!(expect_crasher(&good).is_ok());
    }

    #[test]
    fn crasher_fails_when_the_other_task_does_not_verify() {
        let bad = outcome(
            &[("T-01", TaskStatus::Blocked), ("T-02", TaskStatus::Blocked)],
            &[],
        );
        assert!(expect_crasher(&bad).is_err());
    }

    #[test]
    fn the_table_holds_the_five_scenarios_in_order() {
        let names: Vec<&str> = scenarios().iter().map(|scenario| scenario.name).collect();
        assert_eq!(
            names,
            ["collider", "tamperer", "liar", "overspender", "crasher"]
        );
    }

    #[test]
    fn an_unknown_only_name_is_an_error() {
        assert!(choose(Some("nope")).is_err());
    }

    #[test]
    fn only_picks_the_named_scenario() {
        assert_eq!(choose(Some("liar")).map(|picked| picked.len()).ok(), Some(1));
    }
}
