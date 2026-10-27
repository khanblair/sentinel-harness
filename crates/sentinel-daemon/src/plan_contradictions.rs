//! Contradictions between planned tasks: ask the owner, or park both tasks when no one can answer.

use std::sync::Arc;

use sentinel_core::{DecisionState, Task, TaskId, TaskStatus};
use sentinel_plan::contradiction::{ContradictionPair, Resolution, apply_answer, decision_for};
use sentinel_plan::planner::Plan;

use crate::ask::ask_decision;
use crate::engine::Result;
use crate::plan_phase::{finish_without_work, say};
use crate::run_ctx::RunCtx;

pub(crate) async fn settle_contradictions(
    ctx: &Arc<RunCtx>,
    pairs: &[ContradictionPair],
    tasks: &[Task],
    plan: &Plan,
) -> Result<()> {
    let title = |id: &TaskId| {
        tasks
            .iter()
            .find(|t| &t.id == id)
            .map_or_else(String::new, |t| t.title.clone())
    };
    for pair in pairs {
        if ctx.config.park_contradictions {
            park_pair(ctx, pair, plan)?;
            continue;
        }
        let decision = decision_for(pair, &title);
        let answer = ask_decision(ctx, &decision).await;
        let state = if answer.is_some() {
            DecisionState::Answered
        } else {
            DecisionState::Defaulted
        };
        let resolved = sentinel_core::Decision {
            state,
            answer: answer.clone(),
            ..decision
        };
        let resolution = apply_answer(&resolved, answer.as_deref());
        apply_resolution(ctx, resolution, answer.is_some())?;
    }
    Ok(())
}

/// Skips both tasks of a contradiction without asking. Each task gives its planner note as the reason.
fn park_pair(ctx: &RunCtx, pair: &ContradictionPair, plan: &Plan) -> Result<()> {
    say(
        ctx,
        format!(
            "{} and {} contradict each other, so both are skipped without a question.",
            pair.a, pair.b
        ),
    );
    for id in [&pair.a, &pair.b] {
        let reason = parked_reason(pair, &note_of(plan, id));
        finish_without_work(ctx, id, TaskStatus::Skipped, &reason)?;
    }
    Ok(())
}

/// The reason a parked task is skipped. It names both tasks and, when the planner gave one, the note.
pub(crate) fn parked_reason(pair: &ContradictionPair, note: &str) -> String {
    let base = format!(
        "These two tasks contradict each other: {} and {}. Neither was done.",
        pair.a, pair.b
    );
    let note = note.trim();
    if note.is_empty() {
        base
    } else {
        format!("{base} {note}")
    }
}

fn note_of(plan: &Plan, id: &TaskId) -> String {
    plan.tasks
        .iter()
        .find(|task| &task.id == id)
        .map_or_else(String::new, |task| task.note.clone())
}

/// Carry out what the user chose: skip the loser, or mark both tasks as contradicting.
fn apply_resolution(ctx: &RunCtx, resolution: Resolution, answered: bool) -> Result<()> {
    match resolution {
        Resolution::Keep { chosen, skipped } => {
            let reason = format!("You chose {chosen} instead, so this task was skipped.");
            finish_without_work(ctx, &skipped, TaskStatus::Skipped, &reason)
        }
        Resolution::SkipBoth { a, b } => {
            let why = if answered {
                "You skipped both tasks."
            } else {
                "Nobody answered, so both tasks were skipped."
            };
            let reason = format!("{a} and {b} contradict each other. {why}");
            finish_without_work(ctx, &a, TaskStatus::Contradiction, &reason)?;
            finish_without_work(ctx, &b, TaskStatus::Contradiction, &reason)
        }
    }
}
