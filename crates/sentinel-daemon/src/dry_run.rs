//! The dry run: a rehearsal on a sample project that ships with Sentinel. A second engine runs the
//! sample in a temp folder with the same agents, and this module turns its events into the six
//! stages the setup wizard shows. The user's repository is never opened here.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sentinel_core::api::RunRequest;
use sentinel_core::dry_run::{DryRunStage, DryRunStatus};
use sentinel_core::{Cost, Event, EventRecord, HarnessState, PermissionMode, ToolId};
use sentinel_plan::planner::Plan;
use tokio::sync::broadcast::{self, error::RecvError};

use crate::controls::usd_text;
use crate::dry_run_book::{Book, Change, book};
use crate::dry_run_preflight::check_agents;
use crate::engine::{Engine, EngineError, EngineOptions, Result, lock};
use crate::secrets::MemoryStore;

/// A dry run that has not ended after this long is cut short. Its folder is still removed.
const FOLLOW_LIMIT: Duration = Duration::from_secs(600);
/// After a dry run gives up, its second run gets this long to stop before the folder is removed.
const STOP_LIMIT: Duration = Duration::from_secs(30);
/// The dry run never spends more than this, whatever the main cap is.
const DRY_RUN_CAP: &str = "1.00";
const SAMPLE_PREFIX: &str = "sentinel-dry-run-";
/// The plan that ships with the sample. It covers the three tasks of `SAMPLE_BACKLOG`.
const SAMPLE_PLAN: &str = include_str!("../../../fixtures/sample-plan.json");

/// The sample repository, as it ships in the binary. One row per file, so the table stays
/// readable: rustfmt would split each row over four lines.
#[rustfmt::skip]
const SAMPLE_FILES: [(&str, &str); 8] = [
    ("Cargo.toml", include_str!("../../../fixtures/sample-repo/Cargo.toml")),
    ("Cargo.lock", include_str!("../../../fixtures/sample-repo/Cargo.lock")),
    (".gitignore", include_str!("../../../fixtures/sample-repo/.gitignore")),
    ("src/lib.rs", include_str!("../../../fixtures/sample-repo/src/lib.rs")),
    ("src/fees.rs", include_str!("../../../fixtures/sample-repo/src/fees.rs")),
    ("src/ledger.rs", include_str!("../../../fixtures/sample-repo/src/ledger.rs")),
    ("tests/fees.rs", include_str!("../../../fixtures/sample-repo/tests/fees.rs")),
    ("tests/ledger.rs", include_str!("../../../fixtures/sample-repo/tests/ledger.rs")),
];

/// Three tasks from `fixtures/sample-backlog.md`. The fake agents' plans match these ids.
const SAMPLE_BACKLOG: &str = "# fee-ledger backlog

## T-01 Add a health check function
A `health()` function returns `ok`. Done when a test proves it.

## T-02 Reject negative amounts
Posting a negative amount returns an error and changes nothing. Done when a test proves it.

## T-04 Add a daily transfer limit
An account may not move more than 100000 cents in one day. Done when a test proves the limit.
";

type Rx = broadcast::Receiver<EventRecord>;

/// How the wait for the second run ended.
#[derive(Debug, PartialEq, Eq)]
enum FollowEnd {
    Done,
    TimedOut,
    Hopeless,
}

/// A stage change on the book, emitted on the main engine.
fn change_book(main: &Engine, change: impl FnOnce(&mut Book) -> Vec<Change>) {
    let changes = change(&mut lock(book()));
    for (stage, state) in changes {
        main.emit_quiet(&Event::DryRunStage { stage, state });
    }
}

impl Engine {
    /// Start the dry run in the background. It is refused while a run is live or a dry run is
    /// going. The stages show on the stream and in `dry_run_status`.
    pub fn start_dry_run(&self) -> Result<()> {
        if self.is_run_live() {
            return Err(EngineError::Conflict(
                "A run is live. Finish it before the dry run.".to_owned(),
            ));
        }
        if !lock(book()).claim() {
            return Err(EngineError::Conflict(
                "A dry run is already going.".to_owned(),
            ));
        }
        let main = self.clone();
        tokio::spawn(run_dry(main));
        Ok(())
    }

    /// The last dry run: its stages so far and its counts once it ends.
    pub fn dry_run_status(&self) -> DryRunStatus {
        lock(book()).status()
    }
}

/// The whole dry run. It always ends with `DryRunFinished`, also when the rehearsal fails.
async fn run_dry(main: Engine) {
    change_book(&main, |b| b.begin(DryRunStage::Plan));
    if let Err(error) = rehearse(&main).await {
        main.emit_quiet(&Event::Message {
            from: "sentinel".to_owned(),
            to: None,
            text: format!("The dry run could not finish: {error}"),
        });
    }
    change_book(&main, Book::close_open);
    let (verified, total) = lock(book()).end();
    main.emit_quiet(&Event::DryRunFinished { verified, total });
}

/// Write the sample, test its agents, run it on a second engine, and follow that run. The sample
/// folder is gone when this returns, so `DryRunFinished` comes after the cleanup.
async fn rehearse(main: &Engine) -> Result<()> {
    let sample = SampleProject::write()?;
    let second = open_second(main, sample.path()).await?;
    let preflight = check_agents(&second).await?;
    change_book(main, |b| b.finish_preflight(&preflight));
    if preflight.usable.is_empty() {
        return Ok(());
    }
    let mut events = second.subscribe();
    second.start_run(dry_run_request()).await?;
    follow(main, &second, &mut events).await;
    Ok(())
}

/// Map the second engine's events to stages. When the run is hopeless, or the wait runs out, the
/// second run is stopped and its last events are read, so its folder is not removed under a live
/// agent.
async fn follow(main: &Engine, second: &Engine, events: &mut Rx) {
    let end = pump(main, events).await;
    if end == FollowEnd::Done {
        return;
    }
    if end == FollowEnd::Hopeless {
        change_book(main, Book::give_up);
    }
    // A stop that finds no live run is fine: the run already ended.
    let _ = second.stop_run();
    wait_until_done(events).await;
}

/// Apply each event to the stages until the second run reports Done, the wait runs out, or every
/// task is lost.
async fn pump(main: &Engine, events: &mut Rx) -> FollowEnd {
    loop {
        let record = match tokio::time::timeout(FOLLOW_LIMIT, events.recv()).await {
            Ok(Ok(record)) => record,
            Ok(Err(RecvError::Lagged(_))) => continue,
            Ok(Err(RecvError::Closed)) | Err(_) => return FollowEnd::TimedOut,
        };
        if is_done(&record.event) {
            return FollowEnd::Done;
        }
        change_book(main, |b| b.handle(&record.event));
        if lock(book()).hopeless() {
            return FollowEnd::Hopeless;
        }
    }
}

/// Read events, without applying them, until the second run reports Done or the stop wait ends.
async fn wait_until_done(events: &mut Rx) {
    let _ = tokio::time::timeout(STOP_LIMIT, until_done(events)).await;
}

async fn until_done(events: &mut Rx) {
    loop {
        match events.recv().await {
            Ok(record) if is_done(&record.event) => return,
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return,
        }
    }
}

fn is_done(event: &Event) -> bool {
    *event
        == Event::HarnessState {
            state: HarnessState::Done,
        }
}

fn dry_run_request() -> RunRequest {
    RunRequest {
        backlog: None,
        task: None,
        chat: None,
        text: None,
        attachments: Vec::new(),
    }
}

/// A second engine on the sample folder. It uses the main engine's agents and routing, with its
/// own cap and mode. It never reads the main repository.
async fn open_second(main: &Engine, root: &Path) -> Result<Engine> {
    let second = Engine::open(EngineOptions {
        repo_root: root.to_path_buf(),
        home: main.home().to_path_buf(),
        runner: main.inner.runner.clone(),
        secrets: Arc::new(MemoryStore::new()),
        fake_agents: main.inner.fake_agents.clone(),
        in_memory: true,
    })?;
    copy_setup(main, &second).await?;
    second.set_fallback_plan(Some(sample_plan()?));
    Ok(second)
}

/// The plan the sample ships with. The planning step uses it when no agent gives a usable plan.
fn sample_plan() -> Result<Plan> {
    serde_json::from_str(SAMPLE_PLAN).map_err(|e| EngineError::Invalid(e.to_string()))
}

async fn copy_setup(main: &Engine, second: &Engine) -> Result<()> {
    if main.store().list_tools()?.is_empty() {
        main.scan_tools().await?;
    }
    let enabled: Vec<ToolId> = main
        .store()
        .list_tools()?
        .into_iter()
        .filter(|tool| tool.enabled)
        .map(|tool| tool.id)
        .collect();
    second.scan_tools().await?;
    for tool in second.store().list_tools()? {
        let on = enabled.contains(&tool.id);
        second.toggle_tool(&tool.id, on)?;
    }
    second.set_routing(main.config().routing)?;
    let cap = dry_run_cap(main)?;
    second.update_config(|c| {
        c.cap_usd = usd_text(cap);
        c.permission_mode = PermissionMode::AutoInsideReserved;
    })?;
    Ok(())
}

/// The smaller of the main cap and the dry run's own limit.
fn dry_run_cap(main: &Engine) -> Result<Cost> {
    let limit = Cost::parse_usd(DRY_RUN_CAP).map_err(|e| EngineError::Invalid(e.to_string()))?;
    let main_cap = main.cap(&main.config());
    Ok(if main_cap < limit { main_cap } else { limit })
}

/// A temp folder holding the sample project. The folder is removed when this is dropped.
struct SampleProject {
    root: PathBuf,
}

impl SampleProject {
    /// Make the folder and commit the sample on `main`. The guard exists before the writes, so a
    /// failed write still removes what was made.
    fn write() -> Result<Self> {
        let project = Self {
            root: std::env::temp_dir().join(sample_folder_name()),
        };
        project.fill()?;
        Ok(project)
    }

    fn fill(&self) -> Result<()> {
        for (path, text) in SAMPLE_FILES {
            write_file(&self.root.join(path), text)?;
        }
        write_file(&self.root.join("backlog.md"), SAMPLE_BACKLOG)?;
        git(&self.root, &["init", "-b", "main"])?;
        git(&self.root, &["config", "user.name", "Sentinel"])?;
        git(
            &self.root,
            &["config", "user.email", "dry-run@sentinel.invalid"],
        )?;
        git(&self.root, &["add", "-A"])?;
        git(
            &self.root,
            &[
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "Sample project",
            ],
        )
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for SampleProject {
    fn drop(&mut self) {
        // Nothing useful to do when removal fails: the run is over either way.
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A folder name no other dry run or test uses: the process id and the clock in nanoseconds.
fn sample_folder_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    format!("{SAMPLE_PREFIX}{}-{nanos}", std::process::id())
}

fn write_file(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)?;
    Ok(())
}

fn git(dir: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git").current_dir(dir).args(args).output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(EngineError::Invalid(format!(
        "git {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

#[cfg(test)]
mod tests {
    use sentinel_plan::backlog::parse_backlog;
    use sentinel_plan::planner::PlanRequest;

    use super::*;
    use crate::agents::restrict_plan;

    #[test]
    fn the_sample_backlog_has_three_tasks() {
        assert_eq!(SAMPLE_BACKLOG.matches("\n## ").count(), 3);
    }

    #[test]
    fn the_sample_plan_gives_every_sample_task_a_scope() {
        let items = parse_backlog(SAMPLE_BACKLOG).expect("the sample backlog parses");
        let request = PlanRequest {
            items,
            repo_files: Vec::new(),
        };
        let plan = restrict_plan(sample_plan().expect("the sample plan parses"), &request)
            .expect("the sample plan fits the sample backlog");
        assert_eq!(plan.tasks.len(), 3);
        assert!(plan.tasks.iter().all(|task| !task.scope.is_empty()));
    }
}
