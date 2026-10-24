//! The work behind each command.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use sentinel_core::api::{CheckStatus, RunRequest};
use sentinel_core::{Event, HarnessState, TaskStatus};
use sentinel_daemon::cmd::SystemRunner;
use sentinel_daemon::engine::{Engine, EngineOptions, FakeAgents};
use sentinel_daemon::secrets::{EnvFallback, KeychainStore};

pub(crate) fn repo_or_current(repo: Option<PathBuf>) -> Result<PathBuf> {
    let path = match repo {
        Some(path) => path,
        None => std::env::current_dir().context("could not read the current folder")?,
    };
    path.canonicalize()
        .with_context(|| format!("{} is not a folder", path.display()))
}

fn home() -> PathBuf {
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
        home: home(),
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
            task: None,
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

pub(crate) async fn doctor() -> Result<ExitCode> {
    let engine = open_engine(&repo_or_current(None)?, false)?;
    let report = engine.doctor().await;
    let mut failed = false;
    for row in report.rows {
        let mark = if row.status == CheckStatus::Ok {
            "ok "
        } else {
            "FIX"
        };
        failed |= row.status != CheckStatus::Ok;
        println!("{mark} {:<24} {}", row.name, row.detail);
    }
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

pub(crate) fn report(repo: Option<PathBuf>) -> Result<ExitCode> {
    let path = repo_or_current(repo)?.join(".sentinel/report.md");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("no report yet at {}", path.display()))?;
    println!("{text}");
    Ok(ExitCode::SUCCESS)
}
