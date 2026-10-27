//! How an attempt ends for good or goes back: the failure summary, the status, and the undo of
//! half-written files.

use sentinel_core::{Event, Scope, TaskId, TaskStatus, ToolInfo};
use sentinel_tree::restore_path;
use sentinel_verify::Protected;
use sentinel_verify::verdict::looks_like_test_path;
use sentinel_workers::RunOutcome;
use sentinel_workers::context::{FailureInput, failure_summary};

use crate::agent_health::{escalate, pause_after_failure};
use crate::engine::lock;
use crate::run_ctx::RunCtx;

/// How many tries one task may get back. Past that, a broken agent counts against the task, so a
/// run with only broken agents still ends.
const MAX_REFUNDS: u32 = 3;

/// An attempt failed. When the agent failed for its own reasons (no login, no credit, a limit), it
/// is paused and the task gets the try back, because the task did nothing wrong.
pub(crate) fn note_agent_failure(
    ctx: &RunCtx,
    tool: &ToolInfo,
    task_id: &TaskId,
    outcome: &RunOutcome,
) {
    if !pause_after_failure(&ctx.engine, tool, outcome) {
        return;
    }
    let refunds = {
        let mut memory = lock(&ctx.memory);
        let count = memory.refunds.entry(task_id.clone()).or_insert(0);
        *count += 1;
        *count
    };
    if refunds <= MAX_REFUNDS {
        let _ = ctx.engine.store().give_back_attempt(task_id);
    }
}

/// The write guard blocked paths while this task ran, and the task failed. It may have needed them,
/// so its next try may write them too, unless they are protected or are tests. A file listed by
/// two tasks only makes them run one after the other.
fn widen_scope_after_strays(ctx: &RunCtx, task_id: &TaskId) {
    let strays = lock(&ctx.memory).take_strays();
    let Ok(Some(task)) = ctx.engine.store().get_task(task_id) else {
        return;
    };
    let protected = Protected::load(&ctx.engine.repo_path(), &ctx.config.protected_paths);
    let mut paths: Vec<String> = task.scope.entries().map(str::to_owned).collect();
    let known = paths.len();
    for stray in strays {
        if !protected.covers(&stray) && !looks_like_test_path(&stray) && !paths.contains(&stray) {
            paths.push(stray);
        }
    }
    let Ok(wider) = Scope::from_paths(&paths) else {
        return;
    };
    if paths.len() == known || ctx.engine.store().set_task_scope(task_id, &wider).is_err() {
        return;
    }
    ctx.engine.emit_quiet(&Event::Message {
        from: "sentinel".to_owned(),
        to: None,
        text: format!(
            "{task_id} also gets {} on its next try: its agent wrote there and the guard blocked it.",
            paths[known..].join(", ")
        ),
    });
}

/// Put a task back for another try at a higher tier, or block it when the tries are used up.
pub(crate) fn fail_task(ctx: &RunCtx, task_id: &TaskId, reason: &str, output: &str) {
    let engine = &ctx.engine;
    undo_partial_writes(ctx, task_id);
    widen_scope_after_strays(ctx, task_id);
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
