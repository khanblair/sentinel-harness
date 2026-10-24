//! The dry run: a rehearsal on a sample project that ships with Sentinel. A second engine runs the
//! sample in a temp folder with the same agents, and this module turns its events into the six
//! stages the setup wizard shows. The user's repository is never opened here.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sentinel_core::api::RunRequest;
use sentinel_core::dry_run::{DryRunStage, DryRunStageRow, DryRunStatus, StageState};
use sentinel_core::{Cost, Event, EventRecord, HarnessState, PermissionMode, TaskStatus, ToolId};
use tokio::sync::broadcast::{self, error::RecvError};

use crate::controls::usd_text;
use crate::engine::{Engine, EngineError, EngineOptions, Result, lock};
use crate::secrets::MemoryStore;

/// A dry run that has not ended after this long is cut short. Its folder is still removed.
const FOLLOW_LIMIT: Duration = Duration::from_secs(600);
/// The dry run never spends more than this, whatever the main cap is.
const DRY_RUN_CAP: &str = "1.00";
const SAMPLE_PREFIX: &str = "sentinel-dry-run-";

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

/// What the dry run shows: whether it goes on, each stage's state, and the counts at the end.
#[derive(Debug, Default)]
struct Book {
    running: bool,
    stages: Vec<(DryRunStage, StageState)>,
    verified: Option<u32>,
    total: Option<u32>,
}

/// A stage that changed state. The engine emits one event for each.
type Change = (DryRunStage, StageState);

/// The one dry run the daemon keeps. A daemon has one dry run, so the state is not per engine.
static BOOK: OnceLock<Mutex<Book>> = OnceLock::new();

fn book() -> &'static Mutex<Book> {
    BOOK.get_or_init(|| Mutex::new(Book::default()))
}

impl Book {
    /// Start a dry run. Refused while one is going.
    fn claim(&mut self) -> bool {
        if self.running {
            return false;
        }
        *self = Book {
            running: true,
            ..Book::default()
        };
        true
    }

    fn state_of(&self, stage: DryRunStage) -> Option<StageState> {
        self.stages
            .iter()
            .find(|(s, _)| *s == stage)
            .map(|(_, state)| *state)
    }

    /// Set one stage. The change is returned only when the state differs.
    fn set(&mut self, step: DryRunStage, value: StageState) -> Option<Change> {
        match self.stages.iter_mut().find(|(s, _)| *s == step) {
            Some(row) if row.1 == value => return None,
            Some(row) => row.1 = value,
            None => self.stages.push((step, value)),
        }
        Some((step, value))
    }

    /// Mark every stage before `last` done, and `last` itself with `outcome`.
    fn reach(&mut self, last: DryRunStage, outcome: StageState) -> Vec<Change> {
        let before = DryRunStage::ALL.into_iter().take_while(|s| *s != last);
        before
            .chain(std::iter::once(last))
            .filter_map(|s| {
                let target = if s == last { outcome } else { StageState::Done };
                self.set(s, target)
            })
            .collect()
    }

    /// Start a stage, unless it already has a state. A stage never goes back to running.
    fn begin(&mut self, stage: DryRunStage) -> Vec<Change> {
        if self.state_of(stage).is_some() {
            return Vec::new();
        }
        self.reach(stage, StageState::Running)
    }

    /// Apply one event of the second engine to the stages.
    fn handle(&mut self, event: &Event) -> Vec<Change> {
        if let Event::RunFinished { verified, total } = event {
            self.verified = Some(*verified);
            self.total = Some(*total);
        }
        // Starting or ending a stage also settles the stages before it, so a missed event
        // never leaves an earlier stage open.
        if matches!(
            event,
            Event::TaskStatus {
                status: TaskStatus::Running,
                ..
            }
        ) {
            return self.begin(DryRunStage::AgentsWork);
        }
        match event.kind().as_str() {
            "PlanReady" => self.reach(DryRunStage::LockFiles, StageState::Done),
            "BarrierStart" => self.begin(DryRunStage::RunTests),
            "BarrierEnd" => self.reach(DryRunStage::RunTests, StageState::Done),
            "Verdict" => self.reach(DryRunStage::Review, StageState::Done),
            "ReportReady" => self.reach(DryRunStage::Report, StageState::Done),
            _ => Vec::new(),
        }
    }

    /// A run that ended with a stage still open failed that stage.
    fn close_open(&mut self) -> Vec<Change> {
        let open: Vec<DryRunStage> = self
            .stages
            .iter()
            .filter(|(_, state)| *state == StageState::Running)
            .map(|(stage, _)| *stage)
            .collect();
        open.into_iter()
            .filter_map(|stage| self.set(stage, StageState::Failed))
            .collect()
    }

    /// The dry run is over. Returns the verified and total counts.
    fn end(&mut self) -> (u32, u32) {
        self.running = false;
        (self.verified.unwrap_or(0), self.total.unwrap_or(0))
    }

    fn status(&self) -> DryRunStatus {
        let stages = DryRunStage::ALL
            .into_iter()
            .filter_map(|stage| {
                self.state_of(stage)
                    .map(|state| DryRunStageRow { stage, state })
            })
            .collect();
        DryRunStatus {
            running: self.running,
            stages,
            verified: self.verified,
            total: self.total,
        }
    }
}

/// Apply a change to the book, then emit each changed stage on the main engine.
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

/// Write the sample, run it on a second engine, and follow that run. The sample folder is gone
/// when this returns, so `DryRunFinished` comes after the cleanup.
async fn rehearse(main: &Engine) -> Result<()> {
    let sample = SampleProject::write()?;
    let second = open_second(main, sample.path()).await?;
    let mut events = second.subscribe();
    second.start_run(dry_run_request()).await?;
    follow(main, &mut events).await;
    Ok(())
}

/// Map the second engine's events to stages until its run reports Done, or the wait runs out.
async fn follow(main: &Engine, events: &mut broadcast::Receiver<EventRecord>) {
    loop {
        let record = match tokio::time::timeout(FOLLOW_LIMIT, events.recv()).await {
            Ok(Ok(record)) => record,
            Ok(Err(RecvError::Lagged(_))) => continue,
            Ok(Err(RecvError::Closed)) | Err(_) => return,
        };
        if record.event
            == (Event::HarnessState {
                state: HarnessState::Done,
            })
        {
            return;
        }
        change_book(main, |b| b.handle(&record.event));
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
    Ok(second)
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
    use super::*;

    fn fresh() -> Book {
        Book::default()
    }

    #[test]
    fn a_second_claim_is_refused_until_the_dry_run_ends() {
        let mut book = fresh();
        assert!(book.claim());
        assert!(!book.claim());
        book.end();
        assert!(book.claim());
    }

    #[test]
    fn plan_ready_finishes_plan_and_lock_files() {
        let mut book = fresh();
        book.claim();
        book.begin(DryRunStage::Plan);
        let changes = book.handle(&Event::PlanReady {
            tasks: 3,
            contradictions: 0,
            already_done: 0,
        });
        assert_eq!(
            changes,
            vec![
                (DryRunStage::Plan, StageState::Done),
                (DryRunStage::LockFiles, StageState::Done),
            ]
        );
    }

    #[test]
    fn the_first_running_task_starts_agents_work_once() {
        let mut book = fresh();
        book.claim();
        let running = Event::TaskStatus {
            task: sentinel_core::TaskId::new("T-01"),
            status: TaskStatus::Running,
        };
        assert_eq!(
            book.handle(&running),
            vec![
                (DryRunStage::Plan, StageState::Done),
                (DryRunStage::LockFiles, StageState::Done),
                (DryRunStage::AgentsWork, StageState::Running),
            ]
        );
        assert_eq!(book.handle(&running), Vec::new());
    }

    #[test]
    fn a_stage_that_is_done_does_not_restart() {
        let mut book = fresh();
        book.claim();
        book.reach(DryRunStage::Review, StageState::Done);
        assert_eq!(book.begin(DryRunStage::AgentsWork), Vec::new());
    }

    #[test]
    fn a_run_that_ends_early_fails_its_open_stage() {
        let mut book = fresh();
        book.claim();
        book.begin(DryRunStage::Plan);
        let changes = book.close_open();
        assert_eq!(changes, vec![(DryRunStage::Plan, StageState::Failed)]);
    }

    #[test]
    fn the_sample_backlog_has_three_tasks() {
        assert_eq!(SAMPLE_BACKLOG.matches("\n## ").count(), 3);
    }
}
