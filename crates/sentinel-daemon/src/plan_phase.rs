//! The first phase of a run: read the backlog, plan it, settle contradictions, and mark which
//! tasks are ready.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sentinel_core::api::RunRequest;
use sentinel_core::{DecisionState, Event, Task, TaskId, TaskStatus};
use sentinel_gate::policy::{next_mode_after_plan_approval, plan_gate};
use sentinel_plan::backlog::{BacklogItem, parse_backlog};
use sentinel_plan::contradiction::{
    ContradictionPair, Resolution, apply_answer, decision_for, find_contradictions,
};
use sentinel_plan::planner::{PlanRequest, build_tasks};
use sentinel_store::VerificationRecord;

use crate::ask::{ask_approval, ask_decision};
use crate::engine::{EngineError, Result, lock};
use crate::run_ctx::RunCtx;

/// What the plan phase found.
pub(crate) struct PlanOutcome {
    /// False when the user denied the plan card: the run ends before any task starts.
    pub approved: bool,
}

pub(crate) async fn plan_phase(ctx: &Arc<RunCtx>, request: &RunRequest) -> Result<PlanOutcome> {
    let items = read_items(ctx, request)?;
    let plan_request = PlanRequest {
        items,
        repo_files: repo_files(ctx),
    };
    let plan = crate::agents::make_plan(ctx, &plan_request).await?;
    let (tasks, dependencies) =
        build_tasks(&plan_request, &plan).map_err(|e| EngineError::Invalid(e.to_string()))?;
    let pairs = find_contradictions(&plan);
    let store = ctx.engine.store();
    for task in &tasks {
        store.upsert_task(task)?;
    }
    for dependency in &dependencies {
        store.add_dependency(dependency)?;
    }
    let already_done = skip_claimed_done(ctx, &tasks)?;
    ctx.engine.emit(&Event::PlanReady {
        tasks: count(tasks.len()),
        contradictions: count(pairs.len()),
        already_done,
    })?;
    settle_contradictions(ctx, &pairs, &tasks).await?;
    mark_ready(ctx)?;
    let approved = approve_plan(ctx).await;
    Ok(PlanOutcome { approved })
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn read_items(ctx: &RunCtx, request: &RunRequest) -> Result<Vec<BacklogItem>> {
    let engine = &ctx.engine;
    let name = request
        .backlog
        .clone()
        .unwrap_or_else(|| ctx.config.backlog.clone());
    let path = resolve(&engine.repo_path(), &name);
    let text = fs::read_to_string(&path).map_err(|e| {
        EngineError::Invalid(format!(
            "Could not read the backlog {}: {e}",
            path.display()
        ))
    })?;
    let mut items = parse_backlog(&text).map_err(|e| EngineError::Invalid(e.to_string()))?;
    if let Some(only) = &request.task {
        items.retain(|item| &item.id == only);
        if items.is_empty() {
            return Err(EngineError::NotFound(format!("task {only} in the backlog")));
        }
    }
    Ok(items)
}

fn resolve(repo: &Path, name: &str) -> PathBuf {
    let path = Path::new(name);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo.join(path)
    }
}

/// Files of the repository, for the planner to choose scopes from.
fn repo_files(ctx: &RunCtx) -> Vec<String> {
    ctx.git
        .run(&["ls-files"])
        .map(|text| text.lines().take(2000).map(str::to_owned).collect())
        .unwrap_or_default()
}

/// A task the backlog calls done has nothing to verify: it is skipped, with the reason on record.
fn skip_claimed_done(ctx: &RunCtx, tasks: &[Task]) -> Result<u32> {
    let mut skipped = 0;
    for task in tasks.iter().filter(|t| t.flags.already_done) {
        finish_without_work(
            ctx,
            &task.id,
            TaskStatus::Skipped,
            "The backlog says this is already done, so there was nothing to verify.",
        )?;
        skipped += 1;
    }
    Ok(skipped)
}

/// Move a task to an end status that no worker produced, and record why.
pub(crate) fn finish_without_work(
    ctx: &RunCtx,
    task: &TaskId,
    status: TaskStatus,
    reason: &str,
) -> Result<()> {
    let store = ctx.engine.store();
    store.set_task_status(task, status)?;
    store.upsert_verification(&VerificationRecord {
        task: task.clone(),
        tests_pass: false,
        new_test: false,
        reviewer_ok: false,
        tamper_clear: true,
        verdict: status,
        reason: reason.to_owned(),
        receipt_json: "null".to_owned(),
    })?;
    ctx.engine.emit(&Event::TaskStatus {
        task: task.clone(),
        status,
    })?;
    ctx.engine.emit(&Event::Verdict {
        task: task.clone(),
        status,
        reason: reason.to_owned(),
    })?;
    Ok(())
}

async fn settle_contradictions(
    ctx: &Arc<RunCtx>,
    pairs: &[ContradictionPair],
    tasks: &[Task],
) -> Result<()> {
    let title = |id: &TaskId| {
        tasks
            .iter()
            .find(|t| &t.id == id)
            .map_or_else(String::new, |t| t.title.clone())
    };
    for pair in pairs {
        let decision = decision_for(pair, &title);
        let answer = ask_decision(ctx, &decision).await;
        let answered = if answer.is_some() {
            DecisionState::Answered
        } else {
            DecisionState::Defaulted
        };
        let resolved = sentinel_core::Decision {
            state: answered,
            answer: answer.clone(),
            ..decision.clone()
        };
        match apply_answer(&resolved, answer.as_deref()) {
            Resolution::Keep { chosen, skipped } => {
                let reason = format!("You chose {chosen} instead, so this task was skipped.");
                finish_without_work(ctx, &skipped, TaskStatus::Skipped, &reason)?;
            }
            Resolution::SkipBoth { a, b } => {
                let why = if answer.is_some() {
                    "You skipped both tasks."
                } else {
                    "Nobody answered, so both tasks were skipped."
                };
                let reason = format!("{a} and {b} contradict each other. {why}");
                finish_without_work(ctx, &a, TaskStatus::Contradiction, &reason)?;
                finish_without_work(ctx, &b, TaskStatus::Contradiction, &reason)?;
            }
        }
    }
    Ok(())
}

fn mark_ready(ctx: &RunCtx) -> Result<()> {
    let store = ctx.engine.store();
    for task in store.list_tasks()? {
        if task.status == TaskStatus::Planned {
            store.set_task_status(&task.id, TaskStatus::Ready)?;
            ctx.engine.emit(&Event::TaskStatus {
                task: task.id,
                status: TaskStatus::Ready,
            })?;
        }
    }
    Ok(())
}

/// Plan only mode puts one card after the plan. After an approval the run continues in Auto.
async fn approve_plan(ctx: &Arc<RunCtx>) -> bool {
    let mode = ctx.permission_mode();
    if !plan_gate(mode) {
        return true;
    }
    let card = lock(&ctx.book).request_plan(ctx.engine.now());
    let allowed = ask_approval(ctx, card).await;
    if allowed {
        let next = next_mode_after_plan_approval(mode);
        *lock(&ctx.mode) = next;
        ctx.engine
            .emit_quiet(&Event::PermissionChanged { mode: next });
        ctx.engine.emit_quiet(&Event::Message {
            from: "sentinel".to_owned(),
            to: None,
            text: format!("Plan approved. The run goes on in {}.", next.label()),
        });
    }
    allowed
}
