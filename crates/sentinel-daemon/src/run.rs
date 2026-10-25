//! Starting, steering, and finishing a run. The work itself lives in `plan_phase`, `wave`,
//! `attempt`, and `verify_phase`.

use std::sync::{Arc, Mutex};

use sentinel_core::api::RunRequest;
use sentinel_core::{ChatId, Event, HarnessState, Intent, RunId, RunInfo, TaskId, ToolId};
use sentinel_gate::ApprovalBook;
use sentinel_reserve::Scheduler;
use sentinel_tree::{Git, GuardOptions, start_guard};
use sentinel_verify::verifier::DiffReviewer;
use sentinel_workers::Budget;
use sentinel_workers::context::ContextLimits;
use time::OffsetDateTime;
use tokio::sync::{mpsc, watch};

use crate::close_out::finish;
use crate::config::Config;
use crate::controls::Caller;
use crate::engine::{Engine, EngineError, Result, lock};
use crate::intent::{Typing, permission_for};
use crate::plan_phase::first_wave;
use crate::pumps::{budget_pump, forward_guard_events};
use crate::resume::restored_git;
use crate::reviewer::{AgentReviewer, ScopeReviewer};
use crate::run_ctx::{RunCtx, RunMemory, RunPolicy};
use crate::run_source::RunSource;
use crate::wave::{WaveEnd, run_wave};

/// A run longer than this many waves is a bug, not a big backlog.
const MAX_WAVES: u32 = 200;

impl Engine {
    /// Start a run in the background. Returns as soon as it is started: the stream and the
    /// snapshot show the rest.
    /// `None` when the message needed no run: its reply is in the chat.
    pub async fn start_run(&self, request: RunRequest) -> Result<Option<RunInfo>> {
        self.start_run_as(request, &Caller::pc("You")).await
    }

    /// Like `start_run`, naming who asked, so the stream shows their words under their name.
    pub async fn start_run_as(
        &self,
        mut request: RunRequest,
        caller: &Caller,
    ) -> Result<Option<RunInfo>> {
        self.check_can_start(request.chat.as_ref()).await?;
        let resume = self.interrupted_since()?;
        let source = match resume {
            Some(_) => RunSource::resumed(self.interrupted_chat()?),
            None => self.resolve_source(&request)?,
        };
        let Typing::Run(typed) = self.type_step(&mut request, &source, caller).await? else {
            return Ok(None);
        };
        let (ctx, stop_tx) = self.prepare_run(source, resume, request.intent)?;
        let run_info = lock(&self.inner.live)
            .run
            .clone()
            .ok_or_else(|| EngineError::Invalid("run not registered".to_owned()))?;
        if resume.is_none() {
            self.mark_run_live(run_info.started_at, &run_info.chat)?;
            self.announce_request(&ctx.source, caller)?;
            self.announce_intent(&ctx.source.chat, typed.as_ref())?;
            // A message run says it started only once its planner has given a plan.
            if ctx.source.words.is_none() {
                self.announce_run_started()?;
            }
        }
        self.set_state(if resume.is_some() {
            HarnessState::Resuming
        } else {
            HarnessState::Running
        });
        self.launch(ctx, request, stop_tx)?;
        Ok(Some(run_info))
    }

    /// Start the write guard, the budget pump, and the run loop in the background.
    fn launch(
        &self,
        ctx: Arc<RunCtx>,
        request: RunRequest,
        stop_tx: watch::Sender<bool>,
    ) -> Result<()> {
        let (guard_tx, guard_rx) = mpsc::unbounded_channel();
        let policy = Arc::new(RunPolicy {
            scheduler: ctx.scheduler.clone(),
        });
        let guard = start_guard(ctx.git.clone(), policy, guard_tx, GuardOptions::default())
            .map_err(|e| EngineError::Invalid(e.to_string()))?;
        tokio::spawn(forward_guard_events(self.clone(), guard_rx));
        let budget = ctx.budget.clone();
        tokio::spawn(budget_pump(self.clone(), budget, ctx.stop.clone()));
        tokio::spawn(run_main(ctx, request, guard, stop_tx));
        Ok(())
    }

    /// A run needs no live run, and at least one agent that is on and ready.
    pub(crate) fn is_run_live(&self) -> bool {
        matches!(
            self.harness_state(),
            HarnessState::Running
                | HarnessState::Paused
                | HarnessState::Stopping
                | HarnessState::Resuming
        )
    }

    async fn check_can_start(&self, chat: Option<&ChatId>) -> Result<()> {
        if self.is_run_live() {
            if let Some(chat) = chat {
                self.refuse_if_another_chat_is_live(chat)?;
            }
            return Err(EngineError::Refused("A run is already live.".to_owned()));
        }
        if self.store().list_tools()?.is_empty() {
            self.scan_tools().await?;
        }
        let ready = self
            .store()
            .list_tools()?
            .iter()
            .any(|t| t.enabled && t.status.is_ready());
        if !ready {
            return Err(EngineError::Refused(
                "No agent is ready. Turn one on in Settings, Agents and tools.".to_owned(),
            ));
        }
        Ok(())
    }

    /// The git handle for the run, on a clean tree and a fresh private index.
    fn clean_git(&self) -> Result<Git> {
        let repo = self.repo_path();
        let git = Git::new(&repo, repo.join(".sentinel/index"))
            .map_err(|e| EngineError::Invalid(e.to_string()))?;
        git.reset_private_index()
            .map_err(|e| EngineError::Invalid(e.to_string()))?;
        let clean = git
            .is_clean()
            .map_err(|e| EngineError::Invalid(e.to_string()))?;
        if !clean {
            return Err(EngineError::Refused(
                "The working tree has uncommitted changes. Commit or stash them, then start the run.".to_owned(),
            ));
        }
        Ok(git)
    }

    /// Fake mode checks scope only. A real run asks an agent to review the diff.
    fn reviewer_for(
        &self,
        memory: &Arc<Mutex<RunMemory>>,
        stop: &watch::Receiver<bool>,
        budget: &Arc<Budget>,
    ) -> Arc<dyn DiffReviewer> {
        if self.is_fake() {
            return Arc::new(ScopeReviewer {
                store: self.store().clone(),
            });
        }
        Arc::new(AgentReviewer {
            engine: self.clone(),
            memory: memory.clone(),
            stop: stop.clone(),
            budget: budget.clone(),
        })
    }

    fn new_run_info(
        &self,
        source: &RunSource,
        config: &Config,
        resume: Option<OffsetDateTime>,
    ) -> RunInfo {
        let backlog = match (&source.words, &source.backlog) {
            (Some(_), _) => "your message".to_owned(),
            (None, Some(name)) => name.clone(),
            (None, None) => config.backlog.clone(),
        };
        RunInfo {
            id: RunId::generate(),
            chat: source.chat.clone(),
            backlog,
            wave: 0,
            started_at: resume.unwrap_or_else(|| self.now()),
        }
    }

    /// The tree for this run. A fresh run needs a clean tree and an empty board. A resumed run
    /// puts stray changes back and keeps its tasks.
    fn open_tree(&self, resume: Option<OffsetDateTime>) -> Result<Git> {
        if resume.is_some() {
            return restored_git(self);
        }
        let git = self.clean_git()?;
        self.store().reset_for_new_run()?;
        Ok(git)
    }

    /// The budget guard. A resumed run starts with what the interrupted run already spent.
    fn make_budget(&self, config: &Config, resume: Option<OffsetDateTime>) -> Result<Arc<Budget>> {
        let (budget, _kill) = Budget::new(config.cap()?);
        if let Some(since) = resume {
            budget.seed(self.store().cost_since(since)?);
        }
        Ok(budget)
    }

    fn prepare_run(
        &self,
        source: RunSource,
        resume: Option<OffsetDateTime>,
        intent: Option<Intent>,
    ) -> Result<(Arc<RunCtx>, watch::Sender<bool>)> {
        let config = self.config();
        let git = self.open_tree(resume)?;
        let budget = self.make_budget(&config, resume)?;
        let retry_limit = self.router()?.retry_limit();
        let scheduler = Scheduler::new(self.store().clone())
            .map_err(|e| EngineError::Invalid(e.to_string()))?
            .with_retry_limit(retry_limit);
        let (pause_tx, pause_rx) = watch::channel(false);
        let (stop_tx, stop_rx) = watch::channel(false);
        let memory = Arc::new(Mutex::new(RunMemory::default()));
        let reviewer = self.reviewer_for(&memory, &stop_rx, &budget);
        let run_info = self.new_run_info(&source, &config, resume);
        let ctx = Arc::new(RunCtx {
            engine: self.clone(),
            git,
            scheduler: Arc::new(Mutex::new(scheduler)),
            budget: budget.clone(),
            stop: stop_rx,
            pause: pause_rx,
            limits: ContextLimits::default(),
            mode: Mutex::new(permission_for(intent, config.permission_mode)),
            config,
            book: Mutex::new(ApprovalBook::new()),
            memory,
            reviewer,
            suite_dir: self.repo_path().join(".sentinel/logs"),
            forced: self.chat_agent(&source.chat)?,
            source,
            checkpoint_lock: Mutex::new(()),
            resuming: resume.is_some(),
        });
        self.register_live(run_info, &budget, (pause_tx, stop_tx.clone()));
        Ok((ctx, stop_tx))
    }

    /// Put the run where the controls (pause, resume, stop) and the snapshot can reach it.
    fn register_live(
        &self,
        run_info: RunInfo,
        budget: &Arc<Budget>,
        (pause, stop): (watch::Sender<bool>, watch::Sender<bool>),
    ) {
        let mut live = lock(&self.inner.live);
        live.run = Some(run_info);
        live.budget = Some(budget.clone());
        live.pause = Some(pause);
        live.stop = Some(stop);
    }

    pub fn pause_run(&self) -> Result<()> {
        self.signal(|live| live.pause.as_ref(), true)?;
        self.set_state(HarnessState::Paused);
        Ok(())
    }

    pub fn resume_run(&self) -> Result<()> {
        self.signal(|live| live.pause.as_ref(), false)?;
        self.set_state(HarnessState::Running);
        Ok(())
    }

    /// Stop the run: every worker is killed, checkpoints stay.
    pub fn stop_run(&self) -> Result<()> {
        self.signal(|live| live.stop.as_ref(), true)?;
        self.set_state(HarnessState::Stopping);
        Ok(())
    }

    fn signal(
        &self,
        pick: impl Fn(&crate::live::LiveRun) -> Option<&watch::Sender<bool>>,
        value: bool,
    ) -> Result<()> {
        let live = lock(&self.inner.live);
        let sender =
            pick(&live).ok_or_else(|| EngineError::Refused("No run is live.".to_owned()))?;
        sender.send_replace(value);
        Ok(())
    }

    /// Kill the agent that is working on a task. The task goes back for another try, one tier up.
    pub fn kill_task(&self, task: &TaskId) -> Result<()> {
        let live = lock(&self.inner.live);
        let sender = live
            .task_kills
            .get(task)
            .ok_or_else(|| EngineError::NotFound(format!("a running agent for {task}")))?;
        sender.send_replace(true);
        Ok(())
    }

    pub fn kill_agent(&self, agent: &ToolId) -> Result<()> {
        let task = lock(&self.inner.live)
            .agent_task
            .get(agent)
            .cloned()
            .ok_or_else(|| EngineError::NotFound(format!("a running task for {agent}")))?;
        self.kill_task(&task)
    }
}

async fn run_main(
    ctx: Arc<RunCtx>,
    request: RunRequest,
    guard: sentinel_tree::GuardHandle,
    stop_tx: watch::Sender<bool>,
) {
    let ending = match drive(&ctx, &request).await {
        Ok(ending) => ending,
        Err(error) => {
            ctx.engine.emit_quiet(&Event::Message {
                from: "sentinel".to_owned(),
                to: None,
                text: format!("The run ended because of an error: {error}"),
            });
            Ending::Finished
        }
    };
    match ending {
        Ending::Finished => finish(&ctx).await,
        Ending::WithoutRun => close_without_run(&ctx),
    }
    drop(guard);
    // Ends the helper tasks that watch the stop signal.
    stop_tx.send_replace(true);
}

/// How the run loop ended. A message that needed no run never reports a run to close out.
pub(crate) enum Ending {
    Finished,
    WithoutRun,
}

async fn drive(ctx: &Arc<RunCtx>, request: &RunRequest) -> Result<Ending> {
    let first = match first_wave(ctx, request).await? {
        Ok(number) => number,
        Err(ending) => return Ok(ending),
    };
    for number in first..first + MAX_WAVES {
        match run_wave(ctx, number).await? {
            WaveEnd::Ran => {}
            WaveEnd::Empty | WaveEnd::Stop => break,
        }
    }
    Ok(Ending::Finished)
}

/// Take the run down when it never really started. The reply is already in the chat, so no run
/// event is written: no report, no journal, and no `RunFinished`.
fn close_without_run(ctx: &RunCtx) {
    let engine = &ctx.engine;
    {
        let mut live = lock(&engine.inner.live);
        live.budget = None;
        live.run = None;
        live.pause = None;
        live.stop = None;
        live.task_kills.clear();
        live.agent_task.clear();
    }
    engine.clear_run_live();
    engine.set_state(HarnessState::Idle);
}
