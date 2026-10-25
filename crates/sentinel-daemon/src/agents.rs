//! Finding the agent CLIs on this computer, turning them into workers, and asking one of them
//! for a plan. Tests may swap in three scripted agents (the fixtures in `fixtures/fake-cli`); the
//! product only ever uses the agent CLIs found on this computer.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use sentinel_core::{Capability, Event, Routing, Tier, ToolId, ToolInfo, ToolStatus};
use sentinel_gate::policy::default_capabilities;
use sentinel_plan::planner::{HeuristicPlanner, Plan, PlanRequest, PlannerBackend, validate_plan};
use sentinel_tree::looks_like_test;
use sentinel_workers::adapter_cli::CliWorker;
use sentinel_workers::detect::{DetectOptions, detect_all};
use sentinel_workers::profile::{Profile, builtin_profiles};
use sentinel_workers::router::Router;
use sentinel_workers::supervisor::{CostSource, run_command};
use sentinel_workers::{CommandLine, RunControl, RunOutcome, RunSpec, Worker};
use tokio::sync::{mpsc, watch};

use crate::agents_config::{fill_routing, route_for_tier, without_missing_tools};
use crate::answer::{answer_text, ask_agent_for_plan};
use crate::engine::{Engine, EngineError, FakeAgents, Result};
use crate::keys;
use crate::openrouter_tool::{is_openrouter_adapter, openrouter_entry, openrouter_worker};
use crate::plan_phase::{Asked, PlanStep};
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
                    (
                        "FAKE_PLANS_DIR".to_owned(),
                        self.plans_dir.to_string_lossy().into_owned(),
                    ),
                ],
                stdin: None,
                workdir: spec.workdir.clone(),
            };
            let source = CostSource {
                tier: self.tier,
                reported_prefix: Some("Total cost: $".to_owned()),
                free: false,
            };
            run_command(command, &spec, ctrl, &source).await
        })
    }
}

/// Set once the default tools and routing were chosen.
const DEFAULTS_KEY: &str = "tools.defaults_applied";

impl Engine {
    /// Find the agent CLIs (or make the fake ones), save them, and set sensible defaults the first
    /// time: every ready tool is turned on, and each tier is routed to a ready tool of that tier.
    pub async fn scan_tools(&self) -> Result<Vec<ToolInfo>> {
        let found = match &self.inner.fake_agents {
            Some(fake) => fake_tools(fake),
            None => self.detect_real().await?,
        };
        for tool in &found {
            self.store().upsert_tool(tool)?;
            self.emit(&Event::ToolDetected { tool: tool.clone() })?;
        }
        let enabled_before = self.config().enabled_tools.len();
        self.drop_missing_tools(&found)?;
        let lost_every_tool = enabled_before > 0 && self.config().enabled_tools.is_empty();
        // The defaults wait for the first scan that finds a ready tool, then never run again, so
        // a user's later choices stay. The exception: every enabled tool was a stale one.
        let defaults_done = self.store().get_setting(DEFAULTS_KEY)?.is_some();
        if (!defaults_done || lost_every_tool) && found.iter().any(|t| t.status.is_ready()) {
            self.apply_first_scan_defaults(&found)?;
            self.store().set_setting(DEFAULTS_KEY, "1")?;
        }
        self.fill_empty_routing(&found)?;
        for tool in self.store().list_tools()? {
            self.ensure_default_capabilities(&tool.id)?;
        }
        Ok(self.store().list_tools()?)
    }

    /// Remove the tools this scan did not find from the config and from the store, so the scan alone
    /// decides what is on this computer.
    fn drop_missing_tools(&self, found: &[ToolInfo]) -> Result<()> {
        let config = self.config();
        let (enabled, routing) =
            without_missing_tools(&config.enabled_tools, &config.routing, found);
        if enabled != config.enabled_tools || routing != config.routing {
            self.update_config(|c| {
                c.enabled_tools = enabled;
                c.routing = routing;
            })?;
        }
        for tool in self.store().list_tools()? {
            let is_found = found.iter().any(|f| f.id == tool.id);
            if !is_found {
                self.store().delete_tool(&tool.id)?;
            }
        }
        Ok(())
    }

    /// Route each empty tier to an enabled tool that is ready now. Runs on every scan.
    fn fill_empty_routing(&self, found: &[ToolInfo]) -> Result<()> {
        let config = self.config();
        let usable: Vec<&ToolInfo> = found
            .iter()
            .filter(|t| t.status.is_ready() && config.enabled_tools.contains(&t.id))
            .collect();
        let routing = fill_routing(config.routing.clone(), &usable);
        if routing != config.routing {
            self.set_routing(routing)?;
        }
        Ok(())
    }

    async fn detect_real(&self) -> Result<Vec<ToolInfo>> {
        let profiles = builtin_profiles().map_err(|e| EngineError::Invalid(e.to_string()))?;
        let options = DetectOptions {
            home: Some(self.home().to_path_buf()),
            ..DetectOptions::default()
        };
        let mut found = detect_all(&profiles, &options).await;
        // Last on purpose: the first-scan routing takes the first ready CLI of a tier, so a saved
        // key only serves a tier that no CLI covers.
        found.push(openrouter_entry(self.inner.secrets.as_ref()));
        Ok(found)
    }

    /// Turn on the ready tools and route each tier to one of them. A config file that already
    /// names its tools or its routing wins over these guesses.
    fn apply_first_scan_defaults(&self, found: &[ToolInfo]) -> Result<()> {
        let config = self.config();
        let wanted = |tool: &&ToolInfo| {
            config.enabled_tools.is_empty() || config.enabled_tools.contains(&tool.id)
        };
        let ready: Vec<&ToolInfo> = found
            .iter()
            .filter(|t| t.status.is_ready())
            .filter(wanted)
            .collect();
        for tool in &ready {
            self.toggle_tool(&tool.id, true)?;
        }
        if config.routing != sentinel_core::Routing::default() {
            return Ok(());
        }
        self.set_routing(sentinel_core::Routing {
            cheap: route_for_tier(&ready, Tier::Cheap),
            mid: route_for_tier(&ready, Tier::Mid),
            smart: route_for_tier(&ready, Tier::Smart),
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
        Ok(Router::new(routing_map(&self.config().routing), tools))
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
        if is_openrouter_adapter(&tool.adapter) {
            return openrouter_worker(self.inner.secrets.as_ref(), &self.config());
        }
        let profile = profile_for(&tool.adapter)?;
        if tool.path.is_empty() {
            return Err(EngineError::Invalid(format!(
                "{} has no known location",
                tool.name
            )));
        }
        let free = self
            .config()
            .is_free_model(&tool.id, &profile.default_model);
        let env = keys::agent_env(&tool.adapter, self.inner.secrets.as_ref(), &|name| {
            std::env::var(name).ok()
        });
        let worker = CliWorker::new(profile, tool.path.clone().into())
            .with_free_cost(free)
            .with_env(env);
        Ok(Arc::new(worker))
    }
}

/// The tier to tool map of a routing, as the router takes it.
pub(crate) fn routing_map(routing: &Routing) -> HashMap<Tier, ToolId> {
    [Tier::Cheap, Tier::Mid, Tier::Smart]
        .into_iter()
        .filter_map(|tier| routing.for_tier(tier).map(|id| (tier, id.clone())))
        .collect()
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

/// Said to the owner when the dry run's plan stands in for an agent's plan.
pub(crate) const FALLBACK_PLAN_NOTE: &str =
    "No agent gave a usable plan. The dry run uses the plan that ships with the sample.";

/// The file that scripts the fake planner. The fake agent runs it as task `PLAN`.
const SCRIPTED_PLANNER: &str = "PLAN.fake";

/// The longest a planner or a reviewer may take to answer.
pub(crate) const TEXT_TIMEOUT: Duration = Duration::from_secs(180);

/// The plan of a run. Fake mode reads `plan.json` next to the fake plans. Real mode asks the
/// cheapest ready agent, then the engine's fallback plan if it has one, and last the offline
/// heuristic. A message run may get a reply or a failure instead of a plan.
pub(crate) async fn make_plan(ctx: &RunCtx, request: &PlanRequest) -> Result<PlanStep> {
    let engine = &ctx.engine;
    // Fake mode reads `plan.json`, unless the folder scripts a planner.
    if let Some(fake) = &engine.inner.fake_agents
        && !fake.plans_dir.join(SCRIPTED_PLANNER).exists()
    {
        let text = std::fs::read_to_string(fake.plans_dir.join("plan.json"))
            .map_err(|e| EngineError::Invalid(format!("fake plan.json: {e}")))?;
        let plan: Plan =
            serde_json::from_str(&text).map_err(|e| EngineError::Invalid(e.to_string()))?;
        return restrict_plan(plan, request).map(PlanStep::Plan);
    }
    let message = ctx.source.words.is_some();
    let asked = ask_agent_for_plan(ctx, request).await;
    match (ctx.forced.as_ref(), asked) {
        (_, Asked::Plan(plan)) => Ok(PlanStep::Plan(plan)),
        (_, Asked::Reply(text)) => Ok(PlanStep::Reply(text)),
        (Some(agent), Asked::Failed(reason)) => halted(ctx, agent, &reason),
        (Some(agent), Asked::Unusable) => halted(ctx, agent, "it gave no usable plan"),
        (None, Asked::Failed(reason)) if message => Ok(PlanStep::Unreached(reason)),
        (None, Asked::Failed(_) | Asked::Unusable) => offline_plan(ctx, request).await,
    }
}

/// The chat's agent gave no plan. The run stops and says why. No other agent is asked, and the
/// offline planner does not stand in.
fn halted(ctx: &RunCtx, agent: &ToolId, reason: &str) -> Result<PlanStep> {
    let text = ctx
        .engine
        .no_answer(&ctx.source.chat, Some(agent), reason)?;
    Ok(PlanStep::Halted(text))
}

/// The fallback plan of the dry run, else the offline heuristic. Both stand in for an agent's plan.
async fn offline_plan(ctx: &RunCtx, request: &PlanRequest) -> Result<PlanStep> {
    let engine = &ctx.engine;
    if let Some(plan) = engine.fallback_plan() {
        engine.emit_quiet(&Event::Message {
            from: "sentinel".to_owned(),
            to: None,
            text: FALLBACK_PLAN_NOTE.to_owned(),
        });
        return restrict_plan(plan, request).map(PlanStep::Plan);
    }
    engine.emit_quiet(&Event::Message {
        from: "sentinel".to_owned(),
        to: None,
        text: "No agent gave a usable plan, so Sentinel used its offline planner. Scopes may be empty.".to_owned(),
    });
    HeuristicPlanner
        .plan(request)
        .await
        .map(PlanStep::Plan)
        .map_err(|e| EngineError::Invalid(e.to_string()))
}

/// Keep only what is in the request: a run of one task must not name tasks that are not in it.
pub(crate) fn restrict_plan(mut plan: Plan, request: &PlanRequest) -> Result<Plan> {
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

/// Drop test files that already exist from every scope. Writing one is a tamper by rule, so a plan
/// that lists one would send the agent straight into a failure.
pub(crate) fn without_existing_tests(mut plan: Plan, request: &PlanRequest) -> Plan {
    for task in &mut plan.tasks {
        task.scope
            .retain(|path| !(request.repo_files.contains(path) && looks_like_test(path)));
    }
    plan
}

/// One question put to a worker whose answer is plain text.
pub(crate) struct TextJob<'a> {
    pub stop: watch::Receiver<bool>,
    pub budget: Arc<sentinel_workers::Budget>,
    pub worker: &'a dyn Worker,
    pub tool: &'a ToolInfo,
    pub prompt: String,
}

/// Run a worker once and join its output lines. The reviewer uses this.
pub(crate) async fn run_for_text(engine: &Engine, job: TextJob<'_>) -> String {
    run_for_outcome(engine, job, TEXT_TIMEOUT).await.0
}

/// Run a worker once and give up after `timeout`. Returns its answer and how the run ended, so a
/// failed run is not read as an answer.
pub(crate) async fn run_for_outcome(
    engine: &Engine,
    job: TextJob<'_>,
    timeout: Duration,
) -> (String, RunOutcome) {
    let (lines, _receiver) = mpsc::unbounded_channel();
    let control = RunControl {
        kill: job.stop,
        lines,
        budget: job.budget,
    };
    let spec = RunSpec {
        scope: Vec::new(),
        task: sentinel_core::TaskId::new("PLAN"),
        agent: job.tool.id.clone(),
        model: String::new(),
        prompt: job.prompt,
        workdir: engine.repo_path(),
        timeout,
    };
    let outcome = job.worker.run(spec, control).await;
    let text = answer_text(&outcome.output_tail.join("\n"));
    (text, outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_test_files_leave_the_scope_and_new_ones_stay() {
        let plan: Plan = serde_json::from_str(
            r#"{"tasks":[{"id":"T-01","scope":["src/ledger.rs","tests/ledger.rs","tests/negative.rs"],"tier":"cheap"}]}"#,
        )
        .unwrap();
        let request = PlanRequest {
            items: Vec::new(),
            repo_files: vec!["src/ledger.rs".to_owned(), "tests/ledger.rs".to_owned()],
        };
        let kept = without_existing_tests(plan, &request);
        assert_eq!(kept.tasks[0].scope, ["src/ledger.rs", "tests/negative.rs"]);
    }
}
