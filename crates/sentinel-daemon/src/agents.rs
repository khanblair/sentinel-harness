//! Finding the agent CLIs on this computer, turning them into workers, and asking one of them
//! for a plan. In fake mode the tools are three scripted pretend agents (decision D43 and the
//! fixtures in `fixtures/fake-cli`).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use sentinel_core::{Capability, Event, Tier, ToolId, ToolInfo, ToolStatus};
use sentinel_gate::policy::default_capabilities;
use sentinel_plan::planner::{HeuristicPlanner, Plan, PlanRequest, PlannerBackend, parse_plan_json, planner_prompt, validate_plan};
use sentinel_workers::adapter_cli::CliWorker;
use sentinel_workers::detect::{DetectOptions, detect_all};
use sentinel_workers::profile::{Profile, builtin_profiles};
use sentinel_workers::router::Router;
use sentinel_workers::supervisor::{CostSource, run_command};
use sentinel_workers::{CommandLine, RunControl, RunOutcome, RunSpec, Worker};
use tokio::sync::{mpsc, watch};

use crate::engine::{Engine, EngineError, FakeAgents, Result};
use crate::run_ctx::RunCtx;

/// The three pretend agents. One per tier, each with its own adapter name, so the run satisfies
/// "at least 3 agents across at least 2 tools".
const FAKE_TOOLS: [(&str, &str, Tier); 3] = [
    ("fake-a", "Fake agent A", Tier::Cheap),
    ("fake-b", "Fake agent B", Tier::Mid),
    ("fake-c", "Fake agent C", Tier::Smart),
];

/// Runs the fake agent script. The task id and agent id reach it through the environment.
pub(crate) struct FakeWorker {
    script: std::path::PathBuf,
    plans_dir: std::path::PathBuf,
    tier: Tier,
}

impl Worker for FakeWorker {
    fn run(&self, spec: RunSpec, ctrl: RunControl) -> BoxFuture<'_, RunOutcome> {
        Box::pin(async move {
            let command = CommandLine {
                program: "sh".to_owned(),
                args: vec![self.script.to_string_lossy().into_owned()],
                env: vec![
                    ("SENTINEL_TASK_ID".to_owned(), spec.task.to_string()),
                    ("SENTINEL_AGENT".to_owned(), spec.agent.to_string()),
                    ("FAKE_PLANS_DIR".to_owned(), self.plans_dir.to_string_lossy().into_owned()),
                ],
                stdin: None,
                workdir: spec.workdir.clone(),
            };
            let source = CostSource { tier: self.tier, reported_prefix: Some("Total cost: $".to_owned()) };
            run_command(command, &spec, ctrl, &source).await
        })
    }
}

impl Engine {
    /// Find the agent CLIs (or make the fake ones), save them, and set sensible defaults the first
    /// time: every ready tool is turned on, and each tier is routed to a ready tool of that tier.
    pub async fn scan_tools(&self) -> Result<Vec<ToolInfo>> {
        let found = match &self.inner.fake_agents {
            Some(fake) => fake_tools(fake),
            None => self.detect_real().await?,
        };
        let first_scan = self.store().list_tools()?.is_empty();
        for tool in &found {
            self.store().upsert_tool(tool)?;
            self.emit(Event::ToolDetected { tool: tool.clone() })?;
        }
        if first_scan {
            self.apply_first_scan_defaults(&found)?;
        }
        for tool in self.store().list_tools()? {
            self.ensure_default_capabilities(&tool.id)?;
        }
        Ok(self.store().list_tools()?)
    }

    async fn detect_real(&self) -> Result<Vec<ToolInfo>> {
        let profiles = builtin_profiles().map_err(|e| EngineError::Invalid(e.to_string()))?;
        let options = DetectOptions { home: Some(self.home().to_path_buf()), ..DetectOptions::default() };
        Ok(detect_all(&profiles, &options).await)
    }

    fn apply_first_scan_defaults(&self, found: &[ToolInfo]) -> Result<()> {
        let ready: Vec<&ToolInfo> = found.iter().filter(|t| t.status.is_ready()).collect();
        for tool in &ready {
            self.toggle_tool(&tool.id, true)?;
        }
        let for_tier = |tier: Tier| {
            ready.iter().find(|t| t.tier == tier).or_else(|| ready.first()).map(|t| t.id.clone())
        };
        self.set_routing(sentinel_core::Routing {
            cheap: for_tier(Tier::Cheap),
            mid: for_tier(Tier::Mid),
            smart: for_tier(Tier::Smart),
        })
    }

    fn ensure_default_capabilities(&self, agent: &ToolId) -> Result<()> {
        let known = self.store().list_capabilities()?;
        if known.iter().any(|row| &row.agent == agent) {
            return Ok(());
        }
        for capability in default_capabilities(agent).allowed {
            self.store().set_capability(agent, capability, true)?;
        }
        for off in [Capability::Shell, Capability::Net] {
            self.store().set_capability(agent, off, false)?;
        }
        Ok(())
    }

    /// The router for the enabled, ready tools and the configured routing.
    pub(crate) fn router(&self) -> Result<Router> {
        let tools: Vec<ToolInfo> = self.store().list_tools()?;
        let routing = self.config().routing;
        let map: HashMap<Tier, ToolId> = [Tier::Cheap, Tier::Mid, Tier::Smart]
            .into_iter()
            .filter_map(|tier| routing.for_tier(tier).map(|id| (tier, id.clone())))
            .collect();
        Ok(Router::new(map, tools))
    }

    /// A worker for one tool. Fake tools run the script; real tools run their profile.
    pub(crate) fn worker_for(&self, tool: &ToolInfo) -> Result<Arc<dyn Worker>> {
        if let Some(fake) = &self.inner.fake_agents {
            return Ok(Arc::new(FakeWorker {
                script: fake.script.clone(),
                plans_dir: fake.plans_dir.clone(),
                tier: tool.tier,
            }));
        }
        let profile = profile_for(&tool.adapter)?;
        if tool.path.is_empty() {
            return Err(EngineError::Invalid(format!("{} has no known location", tool.name)));
        }
        Ok(Arc::new(CliWorker::new(profile, tool.path.clone().into())))
    }
}

fn profile_for(adapter: &str) -> Result<Profile> {
    builtin_profiles()
        .map_err(|e| EngineError::Invalid(e.to_string()))?
        .into_iter()
        .find(|p| p.id == adapter)
        .ok_or_else(|| EngineError::NotFound(format!("adapter profile {adapter}")))
}

fn fake_tools(fake: &FakeAgents) -> Vec<ToolInfo> {
    FAKE_TOOLS
        .iter()
        .map(|(id, name, tier)| ToolInfo {
            id: ToolId::new(*id),
            name: (*name).to_owned(),
            path: fake.script.to_string_lossy().into_owned(),
            version: "1.0".to_owned(),
            adapter: format!("{id}-adapter"),
            status: ToolStatus::Ready,
            tier: *tier,
            enabled: false,
        })
        .collect()
}

/// A plan for the backlog. Fake mode reads `plan.json` next to the fake plans. Real mode asks the
/// cheapest ready agent and falls back to the offline heuristic when it cannot get a valid plan.
pub(crate) async fn make_plan(ctx: &RunCtx, request: &PlanRequest) -> Result<Plan> {
    let engine = &ctx.engine;
    if let Some(fake) = &engine.inner.fake_agents {
        let text = std::fs::read_to_string(fake.plans_dir.join("plan.json"))
            .map_err(|e| EngineError::Invalid(format!("fake plan.json: {e}")))?;
        let plan: Plan = serde_json::from_str(&text).map_err(|e| EngineError::Invalid(e.to_string()))?;
        return restrict_plan(plan, request);
    }
    if let Some(plan) = ask_agent_for_plan(ctx, request).await {
        return Ok(plan);
    }
    engine.emit_quiet(Event::Message {
        from: "sentinel".to_owned(),
        to: None,
        text: "No agent gave a usable plan, so Sentinel used its offline planner. Scopes may be empty.".to_owned(),
    });
    HeuristicPlanner.plan(request).await.map_err(|e| EngineError::Invalid(e.to_string()))
}

/// Keep only what is in the request: a run of one task must not name tasks that are not in it.
fn restrict_plan(mut plan: Plan, request: &PlanRequest) -> Result<Plan> {
    let ids: Vec<_> = request.items.iter().map(|i| i.id.clone()).collect();
    plan.tasks.retain(|t| ids.contains(&t.id));
    for task in &mut plan.tasks {
        task.depends_on.retain(|d| ids.contains(d));
        task.read_after_write.retain(|d| ids.contains(d));
        task.contradicts.retain(|d| ids.contains(d));
    }
    validate_plan(&plan, request).map_err(|e| EngineError::Invalid(e.to_string()))?;
    Ok(plan)
}

async fn ask_agent_for_plan(ctx: &RunCtx, request: &PlanRequest) -> Option<Plan> {
    let engine = &ctx.engine;
    let router = engine.router().ok()?;
    let mut tier = Tier::Cheap;
    for _ in 0..2 {
        let agent = router.pick(tier)?;
        let tool = engine.store().list_tools().ok()?.into_iter().find(|t| t.id == agent)?;
        let worker = engine.worker_for(&tool).ok()?;
        let text = run_for_text(engine, ctx.stop.clone(), ctx.budget.clone(), worker.as_ref(), &tool, planner_prompt(request)).await;
        if let Ok(plan) = parse_plan_json(&text, request) {
            return Some(plan);
        }
        tier = tier.escalate().unwrap_or(tier);
    }
    None
}

/// Run a worker once and join its output lines. The planner and the reviewer use this.
pub(crate) async fn run_for_text(
    engine: &Engine,
    stop: watch::Receiver<bool>,
    budget: Arc<sentinel_workers::Budget>,
    worker: &dyn Worker,
    tool: &ToolInfo,
    prompt: String,
) -> String {
    let (lines, _receiver) = mpsc::unbounded_channel();
    let control = RunControl { kill: stop, lines, budget };
    let spec = RunSpec {
        task: sentinel_core::TaskId::new("PLAN"),
        agent: tool.id.clone(),
        model: String::new(),
        prompt,
        workdir: engine.repo_path(),
        timeout: Duration::from_secs(180),
    };
    let outcome = worker.run(spec, control).await;
    outcome.output_tail.join("\n")
}
