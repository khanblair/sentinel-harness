//! The planner's calls: ask agents in turn for a plan, or for a plain reply to a message that
//! needs no plan. A chat with its own agent asks that agent alone. With a roster, the strongest
//! agent plans first.

use std::sync::Arc;

use sentinel_core::{Tier, ToolId, ToolInfo};
use sentinel_plan::planner::{PlanRequest, parse_plan_json, planner_prompt};
use sentinel_workers::Worker;

use crate::agents::{TEXT_TIMEOUT, TextJob, run_for_outcome};
use crate::agents_config::Pick;
use crate::answer::{NO_PLANNER, read_answer};
use crate::engine::Engine;
use crate::plan_phase::Asked;
use crate::run_ctx::RunCtx;

/// How many times one agent is asked for a plan. The second ask carries the reason for the first.
const PLAN_ASKS: u32 = 2;

/// Ask the planner agents in turn. A message run stops at the first answer that is not a plan,
/// so a greeting costs one call. A forced agent is asked once, and its answer is final.
pub(crate) async fn ask_agent_for_plan(ctx: &RunCtx, request: &PlanRequest) -> Asked {
    if ctx.forced.is_some() {
        return ask_tier(ctx, request, Tier::Cheap).await;
    }
    let roster = ctx.engine.roster();
    if !roster.is_empty() {
        return ask_roster_for_plan(ctx, request, &roster).await;
    }
    let message = ctx.source.words.is_some();
    let mut tier = Tier::Cheap;
    for _ in 0..2 {
        match ask_tier(ctx, request, tier).await {
            Asked::Unusable => {}
            Asked::Failed(_) if !message => {}
            asked @ (Asked::Plan(_) | Asked::Reply(_) | Asked::Failed(_)) => return asked,
        }
        tier = tier.escalate().unwrap_or(tier);
    }
    Asked::Unusable
}

/// With a roster, the plan is the most important call of the run, so the strongest agent plans.
/// An agent that fails or answers with no plan is left out, and the next one is asked.
async fn ask_roster_for_plan(ctx: &RunCtx, request: &PlanRequest, roster: &[ToolId]) -> Asked {
    let message = ctx.source.words.is_some();
    for agent in roster.iter().rev() {
        match ask_picked(ctx, request, Tier::Smart, Some(agent)).await {
            Asked::Unusable | Asked::Failed(_) if !message => {}
            asked @ (Asked::Plan(_) | Asked::Reply(_) | Asked::Failed(_)) => return asked,
            Asked::Unusable => {}
        }
    }
    Asked::Unusable
}

/// One question to the planner agent of a tier.
async fn ask_tier(ctx: &RunCtx, request: &PlanRequest, tier: Tier) -> Asked {
    ask_picked(ctx, request, tier, ctx.forced.as_ref()).await
}

/// One question to a planner agent: the named one, or the pick of the tier.
async fn ask_picked(
    ctx: &RunCtx,
    request: &PlanRequest,
    tier: Tier,
    agent: Option<&ToolId>,
) -> Asked {
    let (tool, worker) = match planner_agent(&ctx.engine, tier, agent) {
        Ok(found) => found,
        Err(reason) => return Asked::Failed(reason),
    };
    let message = ctx.source.words.is_some();
    let mut feedback = String::new();
    for attempt in 0..PLAN_ASKS {
        let job = TextJob {
            stop: ctx.stop.clone(),
            budget: ctx.budget.clone(),
            worker: worker.as_ref(),
            tool: &tool,
            prompt: format!("{}{feedback}", planner_prompt(request)),
        };
        let (text, outcome) = run_for_outcome(&ctx.engine, job, TEXT_TIMEOUT).await;
        crate::agent_health::pause_after_failure(&ctx.engine, &tool, &outcome);
        let asked = read_answer(&text, &outcome, request, message);
        if message || attempt + 1 == PLAN_ASKS || !matches!(asked, Asked::Unusable) {
            return asked;
        }
        feedback = unusable_feedback(ctx, &tool, &text, request);
    }
    Asked::Unusable
}

/// A plan that does not parse or does not validate is said in the stream, with the reason, and
/// the same agent is asked once more with that reason in the prompt.
fn unusable_feedback(ctx: &RunCtx, tool: &ToolInfo, text: &str, request: &PlanRequest) -> String {
    let reason = parse_plan_json(text, request)
        .err()
        .map_or_else(|| "it was not a plan".to_owned(), |error| error.to_string());
    let reason: String = reason.chars().take(200).collect();
    let began: String = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(160)
        .collect();
    crate::plan_phase::say(
        ctx,
        format!(
            "{} gave no usable plan: {reason} It began: {began} Asking once more.",
            tool.name
        ),
    );
    format!(
        "\n\nYour last answer could not be used: {reason}\nAnswer again with ONE JSON object that follows the rules, and nothing else."
    )
}

/// The tool and worker of the planner agent for a tier, or the reason there is none. A forced
/// agent is the only planner, so its reason is the one the owner reads.
pub(crate) fn planner_agent(
    engine: &Engine,
    tier: Tier,
    forced: Option<&ToolId>,
) -> std::result::Result<(ToolInfo, Arc<dyn Worker>), String> {
    let agent = match engine.pick_agent(forced, tier).map_err(|e| e.to_string())? {
        Pick::Agent(agent) => agent,
        Pick::Unavailable(reason) if forced.is_some() => return Err(reason),
        Pick::Unavailable(_) => return Err(NO_PLANNER.to_owned()),
    };
    let tool = engine
        .store()
        .list_tools()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|t| t.id == agent)
        .ok_or_else(|| NO_PLANNER.to_owned())?;
    let worker = engine.worker_for(&tool).map_err(|e| e.to_string())?;
    Ok((tool, worker))
}
