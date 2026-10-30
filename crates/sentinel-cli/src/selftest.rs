//! `sentinel selftest`: runs the real engine on a throwaway repository with scripted agents that
//! misbehave on purpose, then prints one PASS or FAIL line per scenario. It needs no real agent,
//! no network, and no money.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Context, bail};
use sentinel_core::api::RunRequest;
use sentinel_core::{Event, HarnessState};
use sentinel_daemon::engine::{Engine, EngineOptions, FakeAgents};

use crate::selftest_cases::{Check, Outcome, Scenario, scenarios};
use crate::selftest_repo::{
    BACKLOG_FILE, REPO_FILES, Scratch, write_plans, write_repo, write_script,
};

/// A scenario that runs longer than this is a FAIL, never a hang.
const SCENARIO_TIMEOUT: Duration = Duration::from_secs(60);
/// Large enough to hold every event of one scenario run.
const EVENT_LIMIT: u32 = 100_000;

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
    Ok(if passed == total {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Runs one scenario and turns every outcome into a check. A timeout is a FAIL, never a hang.
async fn check_scenario(scenario: &Scenario) -> Check {
    match tokio::time::timeout(SCENARIO_TIMEOUT, run_scenario(scenario)).await {
        Ok(Ok(outcome)) => (scenario.expect)(&outcome)
            .map_err(|reason| format!("{reason}\n{}", describe(&outcome))),
        Ok(Err(error)) => Err(format!("{error:#}")),
        Err(_) => Err(format!(
            "timed out after {} seconds",
            SCENARIO_TIMEOUT.as_secs()
        )),
    }
}

/// What the run did, in a few indented lines, so a FAIL shows why.
fn describe(outcome: &Outcome) -> String {
    let statuses: Vec<String> = outcome
        .statuses
        .iter()
        .map(|(task, status)| format!("{task}={}", status.label()))
        .collect();
    let reasons: Vec<String> = outcome
        .reasons
        .iter()
        .map(|(task, reason)| format!("{task}: {reason}"))
        .collect();
    let blocked: Vec<&str> = outcome
        .events_seen
        .iter()
        .filter_map(|event| {
            let Event::WriteBlocked { path, .. } = event else {
                return None;
            };
            Some(path.as_str())
        })
        .collect();
    let notes: Vec<&str> = outcome
        .events_seen
        .iter()
        .filter_map(|event| {
            let Event::Message { from, text, .. } = event else {
                return None;
            };
            (from == "sentinel").then_some(text.as_str())
        })
        .collect();
    format!(
        "  tasks: {}\n  verdicts: {}\n  writes blocked: {}\n  notes: {}",
        statuses.join(", "),
        reasons.join(" | "),
        blocked.join(", "),
        notes.join(" | ")
    )
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
    let fake = FakeAgents {
        script,
        plans_dir: plans,
    };
    let engine = Engine::open(EngineOptions::scripted(repo, fake))?;
    engine.update_config(|c| {
        "1.00".clone_into(&mut c.cap_usd);
        c.test_command = vec!["sh".to_owned(), "tests/run.sh".to_owned()];
        // The existing test is protected. A new test file in `tests/` is allowed, as in a real run.
        c.protected_paths = vec!["tests/run.sh".to_owned()];
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
    let text =
        fs::read_to_string(repo.join(path)).with_context(|| format!("could not read {path}"))?;
    Ok((path.to_owned(), text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selftest_cases::{
        TEST_SCRIPT, expect_collider, expect_crasher, expect_liar, expect_overspender,
        expect_tamperer,
    };
    use sentinel_core::TaskStatus;

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

    fn blocked_gamma() -> Event {
        Event::WriteBlocked {
            task: None,
            path: "src/gamma.txt".to_owned(),
            restored: true,
        }
    }

    fn cap_event() -> Event {
        Event::BudgetThreshold { percent: 95 }
    }

    #[test]
    fn collider_passes_when_the_write_is_blocked_and_gamma_is_restored() {
        let mut good = outcome(&[], &[("src/gamma.txt", "gamma original\n")]);
        good.events_seen.push(blocked_gamma());
        assert!(expect_collider(&good).is_ok());
    }

    #[test]
    fn collider_fails_when_gamma_keeps_the_write() {
        let mut bad = outcome(&[], &[("src/gamma.txt", "gamma changed by T-01\n")]);
        bad.events_seen.push(blocked_gamma());
        assert!(expect_collider(&bad).is_err());
    }

    #[test]
    fn collider_fails_when_no_write_was_blocked() {
        let quiet = outcome(&[], &[("src/gamma.txt", "gamma original\n")]);
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
        bad.reasons.push((
            "T-01".to_owned(),
            "tampered with a protected file".to_owned(),
        ));
        assert!(expect_tamperer(&bad).is_err());
    }

    #[test]
    fn tamperer_fails_when_the_edited_script_stays() {
        let mut bad = outcome(
            &[("T-01", TaskStatus::Blocked)],
            &[("tests/run.sh", "#!/bin/sh\nexit 0\n# edited\n")],
        );
        bad.reasons.push((
            "T-01".to_owned(),
            "tampered with a protected file".to_owned(),
        ));
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
            &[
                ("T-01", TaskStatus::Blocked),
                ("T-02", TaskStatus::Verified),
            ],
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
    fn the_table_holds_the_seven_scenarios_in_order() {
        let names: Vec<&str> = scenarios().iter().map(|scenario| scenario.name).collect();
        assert_eq!(
            names,
            [
                "collider",
                "tamperer",
                "liar",
                "overspender",
                "crasher",
                "misscoper",
                "breaker"
            ]
        );
    }

    #[test]
    fn an_unknown_only_name_is_an_error() {
        assert!(choose(Some("nope")).is_err());
    }

    #[test]
    fn only_picks_the_named_scenario() {
        assert_eq!(
            choose(Some("liar")).map(|picked| picked.len()).ok(),
            Some(1)
        );
    }

    /// Every scenario runs on the real engine. A planner or guard change that breaks one fails here
    /// first, not on the machine of someone who runs `sentinel selftest`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn every_scenario_passes_on_the_real_engine() {
        for scenario in scenarios() {
            let check = check_scenario(&scenario).await;
            assert!(check.is_ok(), "{}: {check:?}", scenario.name);
        }
    }
}
