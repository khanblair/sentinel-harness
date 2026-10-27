//! One attempt at one task: choose an agent, run it, and either checkpoint its work or put the
//! task back for another try.

use std::sync::Arc;

use sentinel_core::{Event, RunId, Task, TaskId, TaskStatus, Tier, ToolId, ToolInfo};
use sentinel_store::{CheckpointRecord, RunRecord};
use sentinel_tree::{Checkpoint, checkpoint, restore_path, write_guard_files};
use sentinel_workers::budget::default_ceiling;
use sentinel_workers::context::{FailureInput, failure_summary};
use sentinel_workers::{OutputLine, RunControl, RunOutcome, RunSpec};
use tokio::sync::{mpsc, watch};

use crate::agent_health::{escalate, refuse_attempt, stop_at_agent};
use crate::agents_config::{Pick, roster_pick};
use crate::engine::{Engine, EngineError, Result, lock};
use crate::prompt::{Retry, task_prompt};
use crate::run_ctx::{RunCtx, merge_kill};

/// Output lines stored as events for one attempt. More are dropped from the stream, not from the
/// run: the tail still reaches the failure summary.
const MAX_STREAM_LINES: usize = 2000;

pub(crate) async fn run_attempt(ctx: &Arc<RunCtx>, task_id: TaskId) {
    if let Err(error) = try_attempt(ctx, &task_id).await {
        // A failure of Sentinel itself (a store error, a missing agent) ends the attempt, not the run.
        fail_task(
            ctx,
            &task_id,
            &format!("Sentinel could not run this attempt: {error}"),
            "",
        );
    }
}

async fn try_attempt(ctx: &Arc<RunCtx>, task_id: &TaskId) -> Result<()> {
    let engine = &ctx.engine;
    let task = engine
        .store()
        .get_task(task_id)?
        .ok_or_else(|| EngineError::NotFound(task_id.to_string()))?;
    let tier = lock(&ctx.memory)
        .tiers
        .get(task_id)
        .copied()
        .unwrap_or(task.tier);
    let agent = match pick_attempt(ctx, &task, tier)? {
        Pick::Agent(agent) => agent,
        Pick::Unavailable(reason) => return refuse_attempt(ctx, task_id, &reason),
    };
    let tool = tool_info(engine, &agent)?;
    let worker = engine.worker_for(&tool)?;
    start(ctx, &task, &tool)?;
    let attempt = Attempt {
        task: &task,
        tool: &tool,
        run_id: RunId::generate(),
    };
    let spec = build_spec(ctx, &task, &tool);
    let outcome = run_worker(ctx, worker.as_ref(), spec, &attempt).await;
    finish_bookkeeping(ctx, &attempt, &outcome)?;
    if outcome.end == sentinel_workers::EndKind::Completed {
        return finish_success(ctx, &task, &outcome);
    }
    crate::agent_health::pause_after_failure(engine, attempt.tool, &outcome);
    let reason = reason_for(ctx, &outcome);
    if ctx.forced.is_some() && outcome.end == sentinel_workers::EndKind::Failed {
        return stop_at_agent(ctx, task_id, &attempt.tool.id, &reason);
    }
    fail_task(ctx, task_id, &reason, &outcome.output_tail.join("\n"));
    Ok(())
}

/// The agent for one try. With a roster, the task's turn and its failed tries pick it, and a
/// skipped agent is passed over. A forced agent, or no roster, leaves the pick to the routing.
fn pick_attempt(ctx: &RunCtx, task: &Task, tier: Tier) -> Result<Pick> {
    let roster = ctx.engine.roster();
    if ctx.forced.is_some() || roster.is_empty() {
        return ctx.engine.pick_agent(ctx.forced.as_ref(), tier);
    }
    let (turn, failures) = {
        let mut memory = lock(&ctx.memory);
        (memory.turn_of(&task.id), memory.failures_of(&task.id))
    };
    let skip = ctx.engine.skipped_agents(&roster)?;
    match roster_pick(&roster, turn, failures, &skip) {
        Some(agent) => Ok(Pick::Agent(agent)),
        None => ctx.engine.pick_agent(None, tier),
    }
}

fn tool_info(engine: &Engine, agent: &ToolId) -> Result<ToolInfo> {
    engine
        .store()
        .list_tools()?
        .into_iter()
        .find(|t| &t.id == agent)
        .ok_or_else(|| EngineError::NotFound(agent.to_string()))
}

/// One task handed to one agent, with the id of the run that records it.
struct Attempt<'a> {
    task: &'a Task,
    tool: &'a ToolInfo,
    run_id: RunId,
}

fn start(ctx: &RunCtx, task: &Task, tool: &ToolInfo) -> Result<()> {
    let engine = &ctx.engine;
    lock(&ctx.scheduler)
        .start_task(&task.id)
        .map_err(|e| EngineError::Invalid(e.to_string()))?;
    lock(&ctx.memory)
        .agents
        .insert(task.id.clone(), tool.id.clone());
    lock(&engine.inner.live)
        .agent_task
        .insert(tool.id.clone(), task.id.clone());
    engine.emit(&Event::FilesReserved {
        task: task.id.clone(),
        paths: task.scope.entries().map(str::to_owned).collect(),
    })?;
    engine.emit(&Event::TaskStatus {
        task: task.id.clone(),
        status: TaskStatus::Running,
    })?;
    engine.emit(&Event::Message {
        from: "sentinel".to_owned(),
        to: Some(tool.id.clone()),
        text: format!(
            "{} goes to {} ({} task).",
            task.id,
            tool.name,
            task.tier.ui_label()
        ),
    })?;
    let guard_dir = engine.repo_path().join(".sentinel/guard");
    write_guard_files(&guard_dir, &task.id, &task.scope)
        .map_err(|e| EngineError::Invalid(e.to_string()))?;
    // A tool that reports its cost only at the end is charged by the clock at its own tier's rate,
    // so the ceiling must be at least that tier's, or the guess trips it before the tool reports.
    ctx.budget
        .set_ceiling(&task.id, default_ceiling(task.tier.max(tool.tier)));
    Ok(())
}

fn build_spec(ctx: &RunCtx, task: &Task, tool: &ToolInfo) -> RunSpec {
    let memory = lock(&ctx.memory);
    let failure = memory.failures.get(&task.id).cloned();
    drop(memory);
    let memory = ctx.engine.task_notes(task, &ctx.source.attachments);
    let retry = failure.as_ref().map(|text| Retry {
        failure: text,
        diff_stat: "",
        notes: std::slice::from_ref(&memory),
        lessons: &[],
    });
    let prompt = task_prompt(task, retry.as_ref(), &memory, &ctx.limits);
    RunSpec {
        scope: task.scope.entries().map(str::to_owned).collect(),
        task: task.id.clone(),
        agent: tool.id.clone(),
        model: ctx
            .config
            .models
            .get(tool.id.as_str())
            .cloned()
            .unwrap_or_default(),
        prompt,
        workdir: ctx.engine.repo_path(),
        timeout: ctx.task_timeout(),
    }
}

async fn run_worker(
    ctx: &RunCtx,
    worker: &dyn sentinel_workers::Worker,
    spec: RunSpec,
    attempt: &Attempt<'_>,
) -> RunOutcome {
    record_run_start(ctx, attempt, &spec);
    let kill = kill_signal(ctx, &attempt.task.id);
    let (lines, line_rx) = mpsc::unbounded_channel();
    let forwarder = tokio::spawn(forward_lines(ctx.engine.clone(), line_rx));
    let control = RunControl {
        kill,
        lines,
        budget: ctx.budget.clone(),
    };
    let outcome = worker.run(spec, control).await;
    // The sender is gone once the worker returns, so the forwarder drains and ends.
    let _ = forwarder.await;
    outcome
}

fn record_run_start(ctx: &RunCtx, attempt: &Attempt<'_>, spec: &RunSpec) {
    let record = RunRecord {
        id: attempt.run_id.clone(),
        task: attempt.task.id.clone(),
        tool: attempt.tool.id.clone(),
        model: spec.model.clone(),
        started_at: ctx.engine.now(),
        ended_at: None,
        exit_code: None,
        cost: sentinel_core::Cost::ZERO,
    };
    if let Err(error) = ctx.engine.store().insert_run(&record) {
        eprintln!("sentinel: could not record the run: {error}");
    }
}

/// Fires when the run stops, the budget guard trips, or the user kills this one task.
fn kill_signal(ctx: &RunCtx, task: &TaskId) -> watch::Receiver<bool> {
    let (task_kill, task_kill_rx) = watch::channel(false);
    lock(&ctx.engine.inner.live)
        .task_kills
        .insert(task.clone(), task_kill);
    merge_kill(vec![ctx.stop.clone(), ctx.budget.receiver(), task_kill_rx])
}

async fn forward_lines(
    engine: crate::engine::Engine,
    mut lines: mpsc::UnboundedReceiver<OutputLine>,
) {
    let mut sent = 0;
    while let Some(line) = lines.recv().await {
        sent += 1;
        if sent <= MAX_STREAM_LINES {
            engine.emit_quiet(&Event::AgentOutput {
                task: line.task,
                agent: line.agent,
                line: line.text,
            });
        }
    }
}

fn finish_bookkeeping(ctx: &RunCtx, attempt: &Attempt<'_>, outcome: &RunOutcome) -> Result<()> {
    let engine = &ctx.engine;
    let code = outcome.exit_code.unwrap_or(-1);
    engine
        .store()
        .finish_run(&attempt.run_id, engine.now(), code, outcome.cost)?;
    let mut live = lock(&engine.inner.live);
    live.agent_task.remove(&attempt.tool.id);
    live.task_kills.remove(&attempt.task.id);
    drop(live);
    let guard_dir = engine.repo_path().join(".sentinel/guard");
    let _ = sentinel_tree::guard_config::remove_guard_files(&guard_dir, &attempt.task.id);
    Ok(())
}

fn reason_for(ctx: &RunCtx, outcome: &RunOutcome) -> String {
    use sentinel_workers::EndKind;
    match outcome.end {
        EndKind::Completed => "The agent finished.".to_owned(),
        EndKind::Failed => format!(
            "The agent stopped with an error (exit {}).",
            outcome.exit_code.unwrap_or(-1)
        ),
        EndKind::TimedOut => format!(
            "The agent ran past {} seconds.",
            ctx.config.task_timeout_seconds
        ),
        EndKind::Killed if ctx.stopped() => "The run was stopped.".to_owned(),
        EndKind::Killed => "The agent was stopped (killed, or the 95% budget guard).".to_owned(),
        EndKind::CostCeiling => "The agent hit the cost ceiling for one task.".to_owned(),
    }
}

/// The agent finished cleanly: commit the files of its scope that changed.
fn finish_success(ctx: &RunCtx, task: &Task, outcome: &RunOutcome) -> Result<()> {
    let changed = ctx
        .git
        .changed_paths()
        .map_err(|e| EngineError::Invalid(e.to_string()))?;
    let mine: Vec<String> = changed
        .into_iter()
        .filter(|path| task.scope.covers(path))
        .collect();
    let message = format!("{}: {}", task.id, task.title);
    let made = if mine.is_empty() {
        None
    } else {
        let _one_at_a_time = lock(&ctx.checkpoint_lock);
        checkpoint(&ctx.git, &mine, &message).map_err(|e| EngineError::Invalid(e.to_string()))?
    };
    let Some(made) = made else {
        fail_task(
            ctx,
            &task.id,
            "The agent finished but changed none of the files of its task.",
            &outcome.output_tail.join("\n"),
        );
        return Ok(());
    };
    record_checkpoint(ctx, task, made)
}

/// Store a new checkpoint, move the task to Checkpointed, and tell the stream.
fn record_checkpoint(ctx: &RunCtx, task: &Task, made: Checkpoint) -> Result<()> {
    let engine = &ctx.engine;
    engine.store().insert_checkpoint(&CheckpointRecord {
        task: task.id.clone(),
        commit: made.commit.clone(),
        paths: made.paths.clone(),
        at: engine.now(),
    })?;
    lock(&ctx.memory)
        .checkpoints
        .insert(task.id.clone(), made.clone());
    lock(&ctx.scheduler)
        .on_checkpoint(&task.id, engine.now())
        .map_err(|e| EngineError::Invalid(e.to_string()))?;
    engine.emit(&Event::Checkpoint {
        task: task.id.clone(),
        commit: made.commit,
        paths: made.paths.clone(),
    })?;
    engine.emit(&Event::FilesReleased {
        task: task.id.clone(),
        paths: made.paths,
    })?;
    engine.emit(&Event::TaskStatus {
        task: task.id.clone(),
        status: TaskStatus::Checkpointed,
    })?;
    Ok(())
}

/// Put a task back for another try at a higher tier, or block it when the tries are used up.
pub(crate) fn fail_task(ctx: &RunCtx, task_id: &TaskId, reason: &str, output: &str) {
    let engine = &ctx.engine;
    undo_partial_writes(ctx, task_id);
    lock(&ctx.memory).record_failed_try(task_id);
    let summary = failure_summary(
        &FailureInput {
            failing_tests: &[],
            error_blocks: &[reason.to_owned()],
            output,
        },
        &ctx.limits,
    );
    lock(&ctx.memory).failures.insert(task_id.clone(), summary);
    let outcome = lock(&ctx.scheduler).on_failure(task_id, engine.now());
    let status = match outcome {
        Ok(status) => status,
        Err(error) => {
            eprintln!("sentinel: could not move {task_id} after a failure: {error}");
            return;
        }
    };
    if status == TaskStatus::Blocked {
        record_end(ctx, task_id, status, reason);
        return;
    }
    escalate(ctx, task_id);
    engine.emit_quiet(&Event::TaskStatus {
        task: task_id.clone(),
        status,
    });
    engine.emit_quiet(&Event::Message {
        from: "sentinel".to_owned(),
        to: None,
        text: format!("{task_id} did not work out: {reason} It goes back for another try."),
    });
}

/// Record that a task ended for good: its status, its reason, and the verdict card.
pub(crate) fn record_end(ctx: &RunCtx, task_id: &TaskId, status: TaskStatus, reason: &str) {
    let record = sentinel_store::VerificationRecord {
        task: task_id.clone(),
        tests_pass: false,
        new_test: false,
        reviewer_ok: false,
        tamper_clear: true,
        verdict: status,
        reason: reason.to_owned(),
        receipt_json: "null".to_owned(),
    };
    if let Err(error) = ctx.engine.store().upsert_verification(&record) {
        eprintln!("sentinel: could not record the end of {task_id}: {error}");
    }
    ctx.engine.emit_quiet(&Event::TaskStatus {
        task: task_id.clone(),
        status,
    });
    ctx.engine.emit_quiet(&Event::Verdict {
        task: task_id.clone(),
        status,
        reason: reason.to_owned(),
    });
}

/// A failed agent may leave half-written files inside its own scope. Put them back so the next
/// attempt and the next wave start from a clean tree.
pub(crate) fn undo_partial_writes(ctx: &RunCtx, task_id: &TaskId) {
    let Ok(Some(task)) = ctx.engine.store().get_task(task_id) else {
        return;
    };
    let Ok(changed) = ctx.git.changed_paths() else {
        return;
    };
    for path in changed.into_iter().filter(|p| task.scope.covers(p)) {
        let _ = restore_path(&ctx.git, &path, "HEAD");
    }
}
