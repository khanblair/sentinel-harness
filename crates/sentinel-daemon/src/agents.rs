//! Finding the agent CLIs on this computer, turning them into workers, and asking one of them
//! for a plan. Tests may swap in three scripted agents (the fixtures in `fixtures/fake-cli`); the
//! product only ever uses the agent CLIs found on this computer.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use sentinel_core::{Capability, Event, Routing, Tier, ToolId, ToolInfo, ToolStatus};
use sentinel_gate::policy::default_capabilities;
use sentinel_plan::planner::{
    HeuristicPlanner, Plan, PlanRequest, PlannerBackend, parse_plan_json, planner_prompt,
    validate_plan,
};
use sentinel_tree::looks_like_test;
use sentinel_workers::adapter_cli::CliWorker;
use sentinel_workers::detect::{DetectOptions, detect_all};
use sentinel_workers::profile::{Profile, builtin_profiles};
use sentinel_workers::router::Router;
use sentinel_workers::supervisor::{CostSource, run_command};
use sentinel_workers::{CommandLine, RunControl, RunOutcome, RunSpec, Worker};
use tokio::sync::{mpsc, watch};

use crate::engine::{Engine, EngineError, FakeAgents, Result};
use crate::openrouter_tool::{is_openrouter_adapter, openrouter_entry, openrouter_worker};
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

    /// Remove the tools this scan did not find from the config, and mark their store rows off. Old
    /// rows stay in the store (it has no delete), so the scan alone decides what is on this computer.
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
            if !is_found && tool.enabled {
                self.store().set_tool_enabled(&tool.id, false)?;
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
        let worker = CliWorker::new(profile, tool.path.clone().into()).with_free_cost(free);
        Ok(Arc::new(worker))
    }
}

/// The first ready tool of the tier, else the first ready tool of any tier.
fn route_for_tier(ready: &[&ToolInfo], tier: Tier) -> Option<ToolId> {
    ready
        .iter()
        .find(|t| t.tier == tier)
        .or_else(|| ready.first())
        .map(|t| t.id.clone())
}

/// The enabled tools and the routing, with every id the scan did not find taken out.
fn without_missing_tools(
    enabled: &[ToolId],
    routing: &Routing,
    found: &[ToolInfo],
) -> (Vec<ToolId>, Routing) {
    let known = |id: &ToolId| found.iter().any(|t| &t.id == id);
    let keep = |slot: &Option<ToolId>| slot.clone().filter(|id| known(id));
    (
        enabled.iter().filter(|id| known(id)).cloned().collect(),
        Routing {
            cheap: keep(&routing.cheap),
            mid: keep(&routing.mid),
            smart: keep(&routing.smart),
        },
    )
}

/// Fill each empty tier of the routing from the usable tools, by tier. A set tier stays as it is.
fn fill_routing(routing: Routing, usable: &[&ToolInfo]) -> Routing {
    Routing {
        cheap: routing
            .cheap
            .or_else(|| route_for_tier(usable, Tier::Cheap)),
        mid: routing.mid.or_else(|| route_for_tier(usable, Tier::Mid)),
        smart: routing
            .smart
            .or_else(|| route_for_tier(usable, Tier::Smart)),
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
        let plan: Plan =
            serde_json::from_str(&text).map_err(|e| EngineError::Invalid(e.to_string()))?;
        return restrict_plan(plan, request);
    }
    if let Some(plan) = ask_agent_for_plan(ctx, request).await {
        return Ok(plan);
    }
    engine.emit_quiet(&Event::Message {
        from: "sentinel".to_owned(),
        to: None,
        text: "No agent gave a usable plan, so Sentinel used its offline planner. Scopes may be empty.".to_owned(),
    });
    HeuristicPlanner
        .plan(request)
        .await
        .map_err(|e| EngineError::Invalid(e.to_string()))
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
        let tool = engine
            .store()
            .list_tools()
            .ok()?
            .into_iter()
            .find(|t| t.id == agent)?;
        let worker = engine.worker_for(&tool).ok()?;
        let job = TextJob {
            stop: ctx.stop.clone(),
            budget: ctx.budget.clone(),
            worker: worker.as_ref(),
            tool: &tool,
            prompt: planner_prompt(request),
        };
        let text = run_for_text(engine, job).await;
        if let Ok(plan) = parse_plan_json(&text, request) {
            return Some(without_existing_tests(plan, request));
        }
        tier = tier.escalate().unwrap_or(tier);
    }
    None
}

/// Drop test files that already exist from every scope. Writing one is a tamper by rule, so a plan
/// that lists one would send the agent straight into a failure.
fn without_existing_tests(mut plan: Plan, request: &PlanRequest) -> Plan {
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

/// Run a worker once and join its output lines. The planner and the reviewer use this.
pub(crate) async fn run_for_text(engine: &Engine, job: TextJob<'_>) -> String {
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
        timeout: Duration::from_secs(180),
    };
    let outcome = job.worker.run(spec, control).await;
    answer_text(&outcome.output_tail.join("\n"))
}

/// The answer in a tool's output. A tool run with a JSON output format (claude) wraps its answer
/// in an object with a `result` text. Anything else is the plain output.
fn answer_text(output: &str) -> String {
    output
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line.trim()).ok())
        .find_map(|value| value.get("result")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| output.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready_tool(id: &str, tier: Tier) -> ToolInfo {
        ToolInfo {
            id: ToolId::new(id),
            name: id.to_owned(),
            path: format!("/usr/bin/{id}"),
            version: "1.0".to_owned(),
            adapter: id.to_owned(),
            status: ToolStatus::Ready,
            tier,
            enabled: false,
        }
    }

    #[test]
    fn a_tier_takes_its_own_ready_tool_before_any_other() {
        let cli = ready_tool("claude", Tier::Cheap);
        let key = ready_tool("openrouter", Tier::Cheap);
        let ready = [&cli, &key];
        assert_eq!(
            route_for_tier(&ready, Tier::Cheap),
            Some(ToolId::new("claude"))
        );
    }

    #[test]
    fn a_saved_key_only_serves_a_tier_no_cli_covers() {
        let mid = ready_tool("claude", Tier::Mid);
        let smart = ready_tool("codex", Tier::Smart);
        let key = ready_tool("openrouter", Tier::Cheap);
        let ready = [&mid, &smart, &key];
        assert_eq!(
            route_for_tier(&ready, Tier::Cheap),
            Some(ToolId::new("openrouter"))
        );
        assert_eq!(
            route_for_tier(&ready, Tier::Mid),
            Some(ToolId::new("claude"))
        );
        assert_eq!(
            route_for_tier(&ready, Tier::Smart),
            Some(ToolId::new("codex"))
        );
    }

    #[test]
    fn a_tier_with_no_ready_tool_falls_back_to_the_first_ready_one() {
        let key = ready_tool("openrouter", Tier::Cheap);
        let ready = [&key];
        assert_eq!(
            route_for_tier(&ready, Tier::Smart),
            Some(ToolId::new("openrouter"))
        );
        assert_eq!(route_for_tier(&[], Tier::Smart), None);
    }

    #[test]
    fn a_json_envelope_gives_its_result_text() {
        let output = concat!(
            r#"{"type":"result","result":"the plan","#,
            r#""total_cost_usd":0.01}"#
        );
        assert_eq!(answer_text(output), "the plan");
    }

    #[test]
    fn plain_output_stays_as_it_is() {
        assert_eq!(answer_text("line one\nline two"), "line one\nline two");
        assert_eq!(answer_text(r#"{"no":"result"}"#), r#"{"no":"result"}"#);
    }

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

    #[test]
    fn a_stale_fake_tool_is_dropped_and_its_tier_refilled_from_the_real_one() {
        let claude = ready_tool("claude", Tier::Mid);
        let stale = ToolId::new("fake-a");
        let routing = Routing {
            cheap: Some(stale.clone()),
            mid: Some(ToolId::new("claude")),
            smart: None,
        };
        let (enabled, kept) = without_missing_tools(
            &[stale, ToolId::new("claude")],
            &routing,
            std::slice::from_ref(&claude),
        );
        assert_eq!(enabled, [ToolId::new("claude")]);
        assert_eq!(kept.cheap, None);
        let filled = fill_routing(kept, &[&claude]);
        assert_eq!(filled.cheap, Some(ToolId::new("claude")));
        assert_eq!(filled.mid, Some(ToolId::new("claude")));
        assert_eq!(filled.smart, Some(ToolId::new("claude")));
    }

    #[test]
    fn a_computer_with_no_ready_tool_ends_with_nothing_enabled_or_routed() {
        let mut login = ready_tool("claude", Tier::Mid);
        login.status = ToolStatus::NeedsLogin;
        let stale = ToolId::new("fake-a");
        let routing = Routing {
            cheap: Some(stale.clone()),
            mid: Some(stale.clone()),
            smart: Some(stale.clone()),
        };
        let (enabled, kept) = without_missing_tools(&[stale], &routing, &[login]);
        assert_eq!(enabled, Vec::<ToolId>::new());
        assert_eq!(fill_routing(kept, &[]), Routing::default());
    }
}
