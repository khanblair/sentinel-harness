//! The engine: one handle that owns the state of a Sentinel daemon. The HTTP layer, the CLI, and the
//! run loop all call it. Every change goes through `emit`, which stores the event, appends it to the
//! log file, and sends it to every connected device.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use sentinel_core::{
    AgentCost, ChatId, CostSnapshot, DeviceInfo, Event, EventRecord, HarnessState, RepoInfo,
    Snapshot, TailscaleState, ToolId,
};
use sentinel_plan::planner::Plan;
use sentinel_store::{Store, StoreError};
use time::OffsetDateTime;
use tokio::sync::broadcast;

use crate::agent_health::Health;
use crate::brain::BrainState;
use crate::cmd::CommandRunner;
use crate::config::{self, Config, ConfigError};
use crate::live::LiveRun;
use crate::onboarding::OnboardingError;
use crate::pairing::PairingManager;
use crate::secrets::SecretStore;

/// Events kept in a snapshot for the stream. Agent output lines are left out: the Terminal panel
/// reads them from `events_after`.
const SNAPSHOT_EVENTS: u32 = 500;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
    #[error("onboarding error: {0}")]
    Onboarding(#[from] OnboardingError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Refused(String),
    #[error("{0}")]
    Conflict(String),
}

pub type Result<T> = std::result::Result<T, EngineError>;

/// A scripted test double for the agent CLIs. Only tests and `sentinel selftest` set it: the product
/// always runs the real CLIs.
#[derive(Debug, Clone)]
pub struct FakeAgents {
    pub script: PathBuf,
    pub plans_dir: PathBuf,
}

pub struct EngineOptions {
    pub repo_root: PathBuf,
    pub home: PathBuf,
    pub runner: Arc<dyn CommandRunner>,
    pub secrets: Arc<dyn SecretStore>,
    /// The scripted test double (tests only). `None` in the product.
    pub fake_agents: Option<FakeAgents>,
    /// Keep state in memory instead of `.sentinel/state.db` (tests).
    pub in_memory: bool,
}

pub(crate) struct Inner {
    pub store: Store,
    pub repo_root: PathBuf,
    pub home: PathBuf,
    pub config: Mutex<Config>,
    pub bus: broadcast::Sender<EventRecord>,
    pub runner: Arc<dyn CommandRunner>,
    pub secrets: Arc<dyn SecretStore>,
    pub fake_agents: Option<FakeAgents>,
    pub pairing: Mutex<PairingManager>,
    pub repo: Mutex<Option<RepoInfo>>,
    pub tailscale: Mutex<TailscaleState>,
    pub live: Mutex<LiveRun>,
    /// The Brain vault and index, opened on first use.
    pub(crate) brain: Mutex<Option<Arc<BrainState>>>,
    /// True on an always-on host: no native dialogs.
    pub headless: std::sync::atomic::AtomicBool,
    /// Set when the owner asks the daemon to stop (`POST /api/quit`).
    pub quit: tokio::sync::Notify,
    /// When this daemon started.
    pub started: OffsetDateTime,
    /// The engine that knows the paired devices, when this project is not the primary one. One
    /// sign-in then works in every project.
    pub devices_from: Mutex<Option<Engine>>,
    /// The plan the planning step uses when no agent gives a usable one. Only a dry run sets it.
    pub fallback_plan: Mutex<Option<Plan>>,
    /// Agents paused after a failure a retry will not fix. Routing skips them for a while.
    pub health: Health,
    /// The roster of the next run, cheapest first. Empty when the routing picks the agents.
    pub roster: Mutex<Vec<ToolId>>,
}

#[derive(Clone)]
pub struct Engine {
    pub(crate) inner: Arc<Inner>,
}

impl Engine {
    pub fn open(options: EngineOptions) -> Result<Engine> {
        let store = open_store(&options)?;
        let config = config::load(&config::config_path(&options.repo_root))?;
        let (bus, _) = broadcast::channel(2048);
        let engine = Engine {
            inner: Arc::new(Inner {
                store,
                repo_root: options.repo_root,
                home: options.home,
                config: Mutex::new(config),
                bus,
                runner: options.runner,
                secrets: options.secrets,
                fake_agents: options.fake_agents,
                pairing: Mutex::new(PairingManager::new()),
                repo: Mutex::new(None),
                tailscale: Mutex::new(TailscaleState::NotInstalled),
                live: Mutex::new(LiveRun::default()),
                brain: Mutex::new(None),
                headless: std::sync::atomic::AtomicBool::new(false),
                quit: tokio::sync::Notify::new(),
                started: OffsetDateTime::now_utc(),
                devices_from: Mutex::new(None),
                fallback_plan: Mutex::new(None),
                health: Health::default(),
                roster: Mutex::new(Vec::new()),
            }),
        };
        engine.refresh_repo();
        engine.adopt_old_events()?;
        if let Err(error) = engine.prune_empty_chats() {
            eprintln!("Could not remove empty chats: {error}");
        }
        engine.ensure_a_chat()?;
        engine.note_crash_on_open();
        Ok(engine)
    }

    pub fn store(&self) -> &Store {
        &self.inner.store
    }

    pub fn repo_root(&self) -> &Path {
        &self.inner.repo_root
    }

    pub fn home(&self) -> &Path {
        &self.inner.home
    }

    /// True when a test double stands in for the agent CLIs.
    pub fn is_fake(&self) -> bool {
        self.inner.fake_agents.is_some()
    }

    /// The plan the planning step falls back to when no agent gives a usable plan.
    pub fn set_fallback_plan(&self, plan: Option<Plan>) {
        *lock(&self.inner.fallback_plan) = plan;
    }

    pub(crate) fn fallback_plan(&self) -> Option<Plan> {
        lock(&self.inner.fallback_plan).clone()
    }

    /// The roster of the next run, cheapest first. Empty when the routing picks the agents.
    pub(crate) fn roster(&self) -> Vec<ToolId> {
        lock(&self.inner.roster).clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventRecord> {
        self.inner.bus.subscribe()
    }

    pub fn config(&self) -> Config {
        lock(&self.inner.config).clone()
    }

    /// Change the config and save it to `.sentinel/config.toml`.
    pub fn update_config(&self, change: impl FnOnce(&mut Config)) -> Result<Config> {
        let mut guard = lock(&self.inner.config);
        change(&mut guard);
        let saved = guard.clone();
        drop(guard);
        config::save(&config::config_path(&self.inner.repo_root), &saved)?;
        Ok(saved)
    }

    pub fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    /// Store an event, append it to `.sentinel/log.jsonl`, and send it to every device.
    pub fn emit(&self, event: &Event) -> Result<EventRecord> {
        // Everything a run says belongs to the chat that started it.
        let chat = self.live_chat();
        self.emit_in(chat.as_ref(), event)
    }

    /// Like `emit`, for an event that belongs to a given chat (or to none).
    pub fn emit_in(&self, chat: Option<&ChatId>, event: &Event) -> Result<EventRecord> {
        let record = self.inner.store.append_event_in(chat, event, self.now())?;
        self.append_log(&record);
        // Nobody listening is fine: the event is already stored.
        let _ = self.inner.bus.send(record.clone());
        Ok(record)
    }

    /// Like `emit`, for callers that cannot act on a store failure.
    pub fn emit_quiet(&self, event: &Event) {
        if let Err(error) = self.emit(event) {
            tracing_error(&error);
        }
    }

    fn append_log(&self, record: &EventRecord) {
        let path = self.inner.repo_root.join(".sentinel/log.jsonl");
        let Ok(line) = serde_json::to_string(record) else {
            return;
        };
        let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
            return;
        };
        // The log is a record, not state of record: a failed append never stops a run.
        let _ = writeln!(file, "{line}");
    }

    pub fn events_after(&self, after_id: i64, limit: u32) -> Result<Vec<EventRecord>> {
        Ok(self.inner.store.events_after(after_id, limit)?)
    }

    /// Recompute the repository facts shown in the snapshot.
    pub fn refresh_repo(&self) {
        let root = self.repo_path();
        let check = crate::folders::check_repo(&root);
        let backlog = root.join(self.config().backlog);
        let backlog_tasks = fs::read_to_string(&backlog).map_or(0, |text| {
            text.lines().filter(|l| l.starts_with("## ")).count()
        });
        let status = |name: &str| check.rows.iter().find(|r| r.name == name).map(|r| r.status);
        let repo_info = RepoInfo {
            name: root
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
            path: root.to_string_lossy().into_owned(),
            is_git: status("Git repository") == Some(sentinel_core::api::CheckStatus::Ok),
            has_backlog: status("backlog.md") == Some(sentinel_core::api::CheckStatus::Ok),
            clean: status("Clean working tree") == Some(sentinel_core::api::CheckStatus::Ok),
            backlog_tasks: u32::try_from(backlog_tasks).unwrap_or(u32::MAX),
        };
        *lock(&self.inner.repo) = Some(repo_info);
    }

    /// The repository the engine works on: the configured path, else the folder it started in.
    pub fn repo_path(&self) -> PathBuf {
        self.config()
            .repo_path
            .map_or_else(|| self.inner.repo_root.clone(), PathBuf::from)
    }

    pub fn snapshot(&self) -> Result<Snapshot> {
        self.snapshot_for(None)
    }

    /// The snapshot with the stream events of one chat. With no chat given, the chat that owns
    /// the live run, else the chat changed last.
    pub fn snapshot_for(&self, chat: Option<&ChatId>) -> Result<Snapshot> {
        let store = &self.inner.store;
        let config = self.config();
        let live = self.live_summary();
        let mut snapshot = Snapshot::empty();
        snapshot.harness = live.state;
        snapshot.run = live.run;
        snapshot.repo.clone_from(&lock(&self.inner.repo));
        snapshot.tasks = store.list_tasks()?;
        snapshot.dependencies = store.list_dependencies()?;
        snapshot.reservations = store.list_reservations(&[
            sentinel_core::ReservationState::Granted,
            sentinel_core::ReservationState::Waiting,
        ])?;
        snapshot.tools = store.list_tools()?;
        snapshot.routing = config.routing.clone();
        snapshot.permission_mode = config.permission_mode;
        snapshot.capabilities = store.list_capabilities()?;
        snapshot.brain_mode = config.brain_mode;
        snapshot.cost = CostSnapshot {
            total: self.total_cost(live.spent)?,
            cap: self.cap(&config),
        };
        snapshot.per_agent_cost = self.per_agent_cost()?;
        snapshot.approvals = store.list_approvals(&[])?.into_iter().take(50).collect();
        snapshot.decisions = store.list_decisions(&[])?;
        snapshot.devices = self.device_infos()?;
        snapshot.tailscale = *lock(&self.inner.tailscale);
        snapshot.onboarding = self.onboarding()?;
        snapshot.hackathon_mode = config.hackathon_mode;
        snapshot.settings = crate::settings_view::settings_for_ui(&config);
        snapshot.chats = self.list_chats()?;
        self.fill_events(&mut snapshot, chat)?;
        Ok(snapshot)
    }

    fn fill_events(&self, snapshot: &mut Snapshot, chat: Option<&ChatId>) -> Result<()> {
        let open = chat.cloned().or_else(|| {
            let live = snapshot.run.as_ref().map(|run| run.chat.clone());
            live.or_else(|| {
                snapshot
                    .chats
                    .iter()
                    .find(|c| !c.archived)
                    .map(|c| c.id.clone())
            })
        });
        snapshot.last_event_id = self.inner.store.last_event_id()?;
        snapshot.events = match open {
            Some(id) => self
                .inner
                .store
                .recent_chat_stream_events(&id, SNAPSHOT_EVENTS)?,
            None => self.inner.store.recent_stream_events(SNAPSHOT_EVENTS)?,
        };
        Ok(())
    }

    fn device_infos(&self) -> Result<Vec<DeviceInfo>> {
        Ok(self
            .inner
            .store
            .list_devices()?
            .into_iter()
            .map(|d| DeviceInfo {
                id: d.id,
                name: d.name,
                kind: d.kind,
                created_at: d.created_at,
                revoked: d.revoked_at.is_some(),
            })
            .collect())
    }

    fn per_agent_cost(&self) -> Result<Vec<AgentCost>> {
        let mut totals: Vec<(ToolId, sentinel_core::Cost)> = Vec::new();
        for run in self.inner.store.list_runs()? {
            match totals.iter_mut().find(|(tool, _)| *tool == run.tool) {
                Some((_, cost)) => *cost = *cost + run.cost,
                None => totals.push((run.tool, run.cost)),
            }
        }
        Ok(totals
            .into_iter()
            .map(|(agent, cost)| AgentCost { agent, cost })
            .collect())
    }

    pub fn harness_state(&self) -> HarnessState {
        self.live_summary().state
    }
}

fn tracing_error(error: &EngineError) {
    eprintln!("sentinel: could not store an event: {error}");
}

/// Lock a mutex and keep going if another thread panicked while holding it: every guarded value is
/// replaced whole, so it is never half written.
/// The state store of a project: in memory for tests, else `.sentinel/state.db` in the folder.
fn open_store(options: &EngineOptions) -> Result<Store> {
    let sentinel_dir = options.repo_root.join(".sentinel");
    fs::create_dir_all(&sentinel_dir)?;
    if options.in_memory {
        Ok(Store::open_in_memory()?)
    } else {
        Ok(Store::open(&sentinel_dir.join("state.db"))?)
    }
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
