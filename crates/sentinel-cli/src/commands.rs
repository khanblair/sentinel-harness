//! The work behind each command.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use sentinel_core::api::{CheckRow, CheckStatus, RunRequest};
use sentinel_core::{Event, HarnessState, TaskStatus};
use sentinel_daemon::cmd::SystemRunner;
use sentinel_daemon::doctor::{current_os, install_check};
use sentinel_daemon::engine::{Engine, EngineOptions, FakeAgents};
use sentinel_daemon::secrets::{EnvFallback, KeychainStore};

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

/// The scripted agents. They live in `fixtures/fake-cli` of the Sentinel repository, found from the
/// current folder upward, or from `SENTINEL_FIXTURES`.
fn find_fake_agents() -> Result<FakeAgents> {
    let candidates: Vec<PathBuf> = std::env::var_os("SENTINEL_FIXTURES")
        .map(PathBuf::from)
        .into_iter()
        .chain(std::env::current_dir().ok().into_iter().flat_map(|d| {
            d.ancestors()
                .map(|a| a.join("fixtures"))
                .collect::<Vec<_>>()
        }))
        .collect();
    for base in candidates {
        let script = base.join("fake-cli/fake-agent");
        if script.is_file() {
            return Ok(FakeAgents {
                script,
                plans_dir: base.join("fake-cli/plans"),
            });
        }
    }
    bail!(
        "could not find fixtures/fake-cli. Run from the Sentinel repository, or set SENTINEL_FIXTURES."
    )
}

pub(crate) fn open_engine(repo: &Path, fake: bool) -> Result<Engine> {
    let fake_agents = if fake {
        Some(find_fake_agents()?)
    } else {
        None
    };
    Engine::open(EngineOptions {
        repo_root: repo.to_path_buf(),
        home: home_dir(),
        runner: Arc::new(SystemRunner),
        secrets: Arc::new(EnvFallback::new(KeychainStore)),
        fake_agents,
        in_memory: false,
    })
    .context("could not open Sentinel state")
}

pub(crate) async fn run(
    backlog: &str,
    repo: Option<PathBuf>,
    cap: Option<String>,
    fake: bool,
) -> Result<ExitCode> {
    let repo = repo_or_current(repo)?;
    let engine = open_engine(&repo, fake)?;
    if let Some(cap) = cap {
        engine.update_config(|c| c.cap_usd = cap)?;
    }
    engine.scan_tools().await?;
    let mut events = engine.subscribe();
    engine
        .start_run(RunRequest {
            backlog: Some(backlog.to_owned()),
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

pub(crate) async fn scan(repo: Option<PathBuf>) -> Result<ExitCode> {
    let engine = open_engine(&repo_or_current(repo)?, false)?;
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
        let engine = open_engine(&repo, false)?;
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
