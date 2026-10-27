//! The work behind each command.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use sentinel_core::api::{CheckRow, CheckStatus, RunRequest};
use sentinel_core::{Event, HarnessState, TaskStatus};
use sentinel_daemon::cmd::SystemRunner;
use sentinel_daemon::config::Config;
use sentinel_daemon::doctor::{current_os, install_check};
use sentinel_daemon::engine::{Engine, EngineOptions};
use sentinel_daemon::roster::{HACKATHON_AGENTS, parse_roster};
use sentinel_daemon::secrets::{EnvFallback, KeychainStore};
use sentinel_daemon::starter;
use sentinel_daemon::test_command::{detect, split_command};

use crate::doctor_host::host_check_rows;
use crate::writeup;

pub(crate) fn repo_or_current(repo: Option<PathBuf>) -> Result<PathBuf> {
    let path = match repo {
        Some(path) => path,
        None => std::env::current_dir().context("could not read the current folder")?,
    };
    path.canonicalize()
        .with_context(|| format!("{} is not a folder", path.display()))
}

pub(crate) fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

pub(crate) fn open_engine(repo: &Path) -> Result<Engine> {
    Engine::open(EngineOptions {
        repo_root: repo.to_path_buf(),
        home: home_dir(),
        runner: Arc::new(SystemRunner),
        secrets: Arc::new(EnvFallback::new(KeychainStore)),
        fake_agents: None,
        in_memory: false,
    })
    .context("could not open Sentinel state")
}

/// The flags of `sentinel run`.
pub(crate) struct RunArgs {
    pub backlog: String,
    pub repo: Option<PathBuf>,
    pub cap: Option<String>,
    pub test_cmd: Option<String>,
    pub agents: Option<String>,
}

pub(crate) async fn run(args: RunArgs) -> Result<ExitCode> {
    let repo = repo_or_current(args.repo)?;
    let engine = open_engine(&repo)?;
    if let Some(cap) = args.cap {
        engine.update_config(|c| c.cap_usd = cap)?;
    }
    let command = choose_test_command(&engine, &repo, args.test_cmd.as_deref())?;
    println!("test command: {}", command.join(" "));
    engine.update_config(|c| c.test_command = command)?;
    engine.update_config(|c| c.park_contradictions = true)?;
    engine.scan_tools().await?;
    if let Some(text) = args.agents.as_deref() {
        apply_agents(&engine, text)?;
    }
    let mut events = engine.subscribe();
    engine
        .start_run(RunRequest {
            backlog: Some(args.backlog),
            ..RunRequest::default()
        })
        .await?;
    loop {
        tokio::select! {
            Ok(record) = events.recv() => print_event(&record.event),
            () = tokio::time::sleep(Duration::from_millis(500)) => {}
        }
        if engine.harness_state() == HarnessState::Done {
            break;
        }
    }
    while let Ok(record) = events.try_recv() {
        print_event(&record.event);
    }
    print_summary(&engine, &repo)
}

/// The roster of the run, from `--agents`. Prints one line per agent, and a warning when the list is
/// shorter than the hackathon asks for.
fn apply_agents(engine: &Engine, text: &str) -> Result<()> {
    let specs = parse_roster(text).map_err(|error| anyhow::anyhow!("--agents: {error}"))?;
    let roster = engine.apply_roster(&specs)?;
    for line in engine.roster_lines(&roster)? {
        println!("{line}");
    }
    if roster.len() < HACKATHON_AGENTS {
        println!(
            "warning: the hackathon asks for {HACKATHON_AGENTS} agents; this roster has {}",
            roster.len()
        );
    }
    Ok(())
}

/// The test command for this run. An empty `--test-cmd` is a mistake, so it stops the run.
fn choose_test_command(engine: &Engine, repo: &Path, flag: Option<&str>) -> Result<Vec<String>> {
    if flag.is_some_and(|text| split_command(text).is_empty()) {
        bail!("--test-cmd needs a command");
    }
    let saved = engine.config().test_command;
    Ok(pick_command(flag, &saved, detect(repo)))
}

/// The order of choice: the flag, then the saved setting when it is not the default, then
/// what the project says it runs, then the default.
fn pick_command(
    flag: Option<&str>,
    saved: &[String],
    detected: Option<Vec<String>>,
) -> Vec<String> {
    if let Some(words) = flag.map(split_command).filter(|words| !words.is_empty()) {
        return words;
    }
    let default = Config::default().test_command;
    if !saved.is_empty() && saved != default.as_slice() {
        return saved.to_vec();
    }
    detected.unwrap_or(default)
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "the terminal shows only a few of the many event kinds"
)]
fn print_event(event: &Event) {
    match event {
        Event::WaveStarted { wave, tasks } => println!(
            "wave {wave}: {}",
            tasks
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Event::Verdict {
            task,
            status,
            reason,
        } => println!("{task}: {} - {reason}", status.label()),
        Event::WriteBlocked { path, restored, .. } => {
            println!("blocked a write to {path} (restored: {restored})");
        }
        Event::BarrierEnd { wave, passed } => println!(
            "barrier {wave}: {}",
            if *passed {
                "suite passed"
            } else {
                "suite failed"
            }
        ),
        Event::Message { text, .. } => println!("{text}"),
        Event::BudgetThreshold { percent } => println!("{percent}% of the cap is spent"),
        _ => {}
    }
}

fn print_summary(engine: &Engine, repo: &Path) -> Result<ExitCode> {
    let tasks = engine.store().list_tasks()?;
    let verified = tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Verified)
        .count();
    println!(
        "\n{verified} of {} tasks verified. Report: {}",
        tasks.len(),
        repo.join(".sentinel/report.md").display()
    );
    Ok(ExitCode::SUCCESS)
}

/// `sentinel starter`: make the starter project in a folder and print where it is.
pub(crate) fn starter(dir: Option<PathBuf>) -> Result<ExitCode> {
    let parent = dir.unwrap_or_else(home_dir);
    let folder =
        starter::create_starter_project(&parent).context("could not make the starter project")?;
    starter::set_starter_test_command(&folder).context("could not save the starter settings")?;
    println!("{}", folder.display());
    Ok(ExitCode::SUCCESS)
}

pub(crate) async fn scan(repo: Option<PathBuf>) -> Result<ExitCode> {
    let engine = open_engine(&repo_or_current(repo)?)?;
    for tool in engine.scan_tools().await? {
        println!(
            "{:<10} {:<12} {:<8} {}",
            tool.name,
            format!("{:?}", tool.status),
            tool.version,
            tool.path
        );
    }
    Ok(ExitCode::SUCCESS)
}

/// The install check, then the host checks when the folder is a repository. The exit code
/// follows the install check only.
pub(crate) async fn doctor(repo: Option<PathBuf>) -> Result<ExitCode> {
    let repo = repo_or_current(repo)?;
    let install = install_check(&SystemRunner, current_os()).await;
    print_rows(&install.rows);
    let failed = install.rows.iter().any(|row| row.status != CheckStatus::Ok);
    if repo.join(".git").exists() {
        println!("\nHost checks");
        let engine = open_engine(&repo)?;
        print_rows(&host_check_rows(&engine, &SystemRunner).await?);
    } else {
        println!("Host checks need a repository. Run this inside one.");
    }
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// One line per row: `ok` or `needs action`, the name, and the detail.
fn print_rows(rows: &[CheckRow]) {
    for row in rows {
        let mark = match row.status {
            CheckStatus::Ok => "ok",
            CheckStatus::NeedsAction | CheckStatus::Invalid => "needs action",
        };
        println!("{mark:<13} {:<24} {}", row.name, row.detail);
    }
}

pub(crate) fn report(repo: Option<PathBuf>, writeup: bool) -> Result<ExitCode> {
    let repo = repo_or_current(repo)?;
    let path = repo.join(".sentinel/report.md");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("no report yet at {}", path.display()))?;
    if writeup {
        let log = read_log(&repo.join(".sentinel/log.jsonl"))?;
        println!("{}", writeup::build(&text, &log));
    } else {
        println!("{text}");
    }
    Ok(ExitCode::SUCCESS)
}

/// Serve the MCP tools on stdin and stdout for an agent. Nothing else may print to stdout.
pub(crate) async fn mcp() -> Result<ExitCode> {
    let served = tokio::task::spawn_blocking(|| {
        sentinel_mcp::serve(
            std::io::stdin().lock(),
            std::io::stdout().lock(),
            sentinel_mcp::ServerConfig::from_env(),
        )
    })
    .await
    .context("the MCP server stopped unexpectedly")?;
    served.context("the MCP server could not read or write its stdio")?;
    Ok(ExitCode::SUCCESS)
}

/// A run that never wrote a log has no events, so a missing log reads as empty.
fn read_log(path: &Path) -> Result<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error).with_context(|| format!("could not read {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(parts: &[&str]) -> Vec<String> {
        parts.iter().map(ToString::to_string).collect()
    }

    fn default_command() -> Vec<String> {
        words(&["cargo", "test"])
    }

    #[test]
    fn the_flag_beats_a_saved_setting_and_detection() {
        let picked = pick_command(
            Some("npm test"),
            &words(&["make", "check"]),
            Some(words(&["go", "test", "./..."])),
        );
        assert_eq!(picked, words(&["npm", "test"]));
    }

    #[test]
    fn a_saved_setting_beats_detection_when_it_is_not_the_default() {
        let picked = pick_command(
            None,
            &words(&["make", "check"]),
            Some(words(&["go", "test", "./..."])),
        );
        assert_eq!(picked, words(&["make", "check"]));
    }

    #[test]
    fn detection_beats_the_saved_default() {
        let picked = pick_command(
            None,
            &default_command(),
            Some(words(&["go", "test", "./..."])),
        );
        assert_eq!(picked, words(&["go", "test", "./..."]));
    }

    #[test]
    fn the_default_is_used_when_nothing_else_applies() {
        assert_eq!(
            pick_command(None, &default_command(), None),
            default_command()
        );
    }

    #[test]
    fn a_blank_flag_falls_through_to_the_saved_setting() {
        let picked = pick_command(Some("   "), &words(&["make", "check"]), None);
        assert_eq!(picked, words(&["make", "check"]));
    }
}
