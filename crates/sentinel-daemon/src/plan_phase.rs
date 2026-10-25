//! The first phase of a run: read the backlog, plan it, settle contradictions, and mark which
//! tasks are ready. A message that needs no work ends here with its reply, and no run starts.

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
use sentinel_plan::planner::{Plan, PlanRequest, build_tasks, holds_plan_json, parse_plan_json};
use sentinel_store::VerificationRecord;
use sentinel_workers::{EndKind, RunOutcome};

use crate::ask::{ask_approval, ask_decision};
use crate::engine::{EngineError, Result, lock};
use crate::run_ctx::RunCtx;
use crate::run_source::{backlog_path, with_context, words_item};

/// The longest reply the chat gets from a planner.
const REPLY_CHARS: usize = 4000;

/// What the plan phase found.
pub(crate) enum PlanOutcome {
    /// The plan is approved, or needed no approval: the run goes on.
    Approved,
    /// The user denied the plan card: the run ends before any task starts.
    Declined,
    /// The message got a reply, or no planner answered. The run never starts.
    NoRun,
}

/// What the planning step produced.
pub(crate) enum PlanStep {
    Plan(Plan),
    /// The planner answered a message that needs no plan. The text goes to the chat as it is.
    Reply(String),
    /// No planning agent answered. The reason goes to the chat.
    Unreached(String),
}

/// What one planner agent said, as far as the run needs it.
pub(crate) enum Asked {
    Plan(Plan),
    Reply(String),
    /// The agent could not answer. A message run stops here, a backlog run tries another agent.
    Failed(String),
    /// The agent answered, but not with a usable plan.
    Unusable,
}

pub(crate) async fn plan_phase(ctx: &Arc<RunCtx>, request: &RunRequest) -> Result<PlanOutcome> {
    let items = read_items(ctx, request)?;
    let plan_request = PlanRequest {
        items,
        repo_files: repo_files(ctx),
    };
    let Some(plan) = plan_or_message(ctx, &plan_request).await? else {
        return Ok(PlanOutcome::NoRun);
    };
    let message = ctx.source.words.is_some();
    if message {
        ctx.engine.announce_run_started()?;
    }
    let (tasks, dependencies) =
        build_tasks(&plan_request, &plan).map_err(|e| EngineError::Invalid(e.to_string()))?;
    if message {
        warn_if_no_files(ctx, &tasks)?;
    }
    let pairs = find_contradictions(&plan);
    store_plan(ctx, &tasks, &dependencies)?;
    let already_done = skip_claimed_done(ctx, &tasks)?;
    ctx.engine.emit(&Event::PlanReady {
        tasks: count(tasks.len()),
        contradictions: count(pairs.len()),
        already_done,
    })?;
    settle_contradictions(ctx, &pairs, &tasks).await?;
    mark_ready(ctx)?;
    if approve_plan(ctx).await {
        Ok(PlanOutcome::Approved)
    } else {
        Ok(PlanOutcome::Declined)
    }
}

fn store_plan(
    ctx: &RunCtx,
    tasks: &[Task],
    dependencies: &[sentinel_core::Dependency],
) -> Result<()> {
    let store = ctx.engine.store();
    for task in tasks {
        store.upsert_task(task)?;
    }
    for dependency in dependencies {
        store.add_dependency(dependency)?;
    }
    Ok(())
}

/// The plan to run, or `None` when the message ends with a reply or with no planner. Either text
/// goes to the chat, and no run starts.
async fn plan_or_message(ctx: &RunCtx, request: &PlanRequest) -> Result<Option<Plan>> {
    let text = match crate::agents::make_plan(ctx, request).await? {
        PlanStep::Plan(plan) => return Ok(Some(plan)),
        PlanStep::Reply(text) => text,
        PlanStep::Unreached(reason) => unreached_text(&reason),
    };
    ctx.engine.emit_quiet(&Event::Message {
        from: "sentinel".to_owned(),
        to: None,
        text,
    });
    Ok(None)
}

/// A message run that plans a task with no files cannot change anything. Said once, naming the
/// first such task, so the owner can name the files.
fn warn_if_no_files(ctx: &RunCtx, tasks: &[Task]) -> Result<()> {
    let Some(task) = tasks.iter().find(|t| t.scope.is_empty()) else {
        return Ok(());
    };
    ctx.engine.emit(&Event::Message {
        from: "sentinel".to_owned(),
        to: None,
        text: format!(
            "{} has no files to write, so it cannot change anything. Name the files in your message.",
            task.id
        ),
    })?;
    Ok(())
}

/// Read one planner answer. A message run reads prose as a reply, and a failed run as no answer.
pub(crate) fn read_answer(
    text: &str,
    outcome: &RunOutcome,
    request: &PlanRequest,
    message: bool,
) -> Asked {
    if let Ok(plan) = parse_plan_json(text, request) {
        return Asked::Plan(crate::agents::without_existing_tests(plan, request));
    }
    if !message {
        return Asked::Unusable;
    }
    if outcome.end != EndKind::Completed || text.trim().is_empty() {
        return Asked::Failed(no_answer_reason(outcome));
    }
    reply_from(text).map_or(Asked::Unusable, Asked::Reply)
}

/// Why a planner gave no answer, in words for the owner.
fn no_answer_reason(outcome: &RunOutcome) -> String {
    match outcome.end {
        EndKind::Completed => "it gave no answer".to_owned(),
        EndKind::Failed => match outcome.exit_code {
            Some(code) => format!("it exited with code {code}"),
            None => "it failed".to_owned(),
        },
        EndKind::TimedOut => "it timed out".to_owned(),
        EndKind::Killed => "it was stopped".to_owned(),
        EndKind::CostCeiling => "it hit its cost limit".to_owned(),
    }
}

/// The planner's answer as a chat reply. `None` when the answer holds a plan or is empty.
pub(crate) fn reply_from(answer: &str) -> Option<String> {
    if holds_plan_json(answer) {
        return None;
    }
    let text = strip_fence(answer.trim());
    if text.is_empty() {
        return None;
    }
    Some(text.chars().take(REPLY_CHARS).collect())
}

/// Drop a code fence that wraps the whole answer, with its language line when it has one.
fn strip_fence(text: &str) -> &str {
    let Some(inner) = text
        .strip_prefix("```")
        .and_then(|rest| rest.strip_suffix("```"))
    else {
        return text;
    };
    inner
        .split_once('\n')
        .map_or(inner, |(_, body)| body)
        .trim()
}

/// What the owner reads when no planning agent answered.
pub(crate) fn unreached_text(reason: &str) -> String {
    format!("I could not reach a planning agent: {reason}. Check Settings, Agents and press Test.")
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn read_items(ctx: &RunCtx, request: &RunRequest) -> Result<Vec<BacklogItem>> {
    let source = &ctx.source;
    if let Some(words) = &source.words {
        return Ok(with_context(vec![words_item(words)], &source.context));
    }
    let engine = &ctx.engine;
    let name = source
        .backlog
        .clone()
        .unwrap_or_else(|| ctx.config.backlog.clone());
    let path = resolve(&engine.repo_path(), &backlog_path(&name)?);
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
    Ok(with_context(items, &source.context))
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

#[cfg(test)]
mod tests {
    use sentinel_core::Cost;

    use super::*;

    fn outcome(end: EndKind) -> RunOutcome {
        RunOutcome {
            end,
            exit_code: Some(2),
            output_tail: Vec::new(),
            cost: Cost::ZERO,
        }
    }

    #[test]
    fn a_plain_answer_is_the_reply_as_it_is() {
        assert_eq!(
            reply_from("Hello! What would you like to build?"),
            Some("Hello! What would you like to build?".to_owned())
        );
    }

    #[test]
    fn a_fence_around_the_whole_answer_is_removed() {
        assert_eq!(
            reply_from("```\nHi there.\n```"),
            Some("Hi there.".to_owned())
        );
    }

    #[test]
    fn an_answer_that_holds_a_plan_is_not_a_reply() {
        assert_eq!(reply_from(r#"{"tasks": []}"#), None);
    }

    #[test]
    fn an_empty_answer_is_not_a_reply() {
        assert_eq!(reply_from("  \n "), None);
    }

    #[test]
    fn a_long_reply_is_cut_at_4000_characters() {
        let long = "é".repeat(5000);
        let reply = reply_from(&long).unwrap_or_default();
        assert_eq!(reply.chars().count(), REPLY_CHARS);
    }

    #[test]
    fn a_message_run_reads_a_failed_run_as_no_answer_even_with_text() {
        let request = PlanRequest {
            items: Vec::new(),
            repo_files: Vec::new(),
        };
        let asked = read_answer("error: boom", &outcome(EndKind::Failed), &request, true);
        assert!(
            matches!(asked, Asked::Failed(reason) if reason == "it exited with code 2"),
            "a failed run gave text, so it is not a reply"
        );
    }

    #[test]
    fn the_unreached_message_names_the_reason_and_the_next_step() {
        let text = unreached_text("it timed out");
        assert_eq!(
            text,
            "I could not reach a planning agent: it timed out. Check Settings, Agents and press Test."
        );
    }
}
