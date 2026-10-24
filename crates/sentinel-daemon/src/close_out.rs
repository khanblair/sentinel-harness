//! Closing a run: every task that has not reached an end gets one, the report is written, and the
//! harness goes to Done.

use std::sync::Arc;

use sentinel_core::{Event, HarnessState, TaskStatus};

use crate::attempt::record_end;
use crate::engine::{Engine, Result, lock};
use crate::run_ctx::RunCtx;

/// Close every task that has not reached an end, write the report, and mark the run done.
pub(crate) async fn finish(ctx: &Arc<RunCtx>) {
    let engine = &ctx.engine;
    if let Err(error) = close_out(ctx) {
        eprintln!("sentinel: could not close out the run: {error}");
    }
    let _ = lock(&ctx.scheduler).recover_after_crash(engine.now());
    if let Err(error) = engine.write_report_file().await {
        engine.emit_quiet(&Event::Message {
            from: "sentinel".to_owned(),
            to: None,
            text: format!("The report could not be written: {error}"),
        });
    }
    let (verified, total) = count_verified(engine);
    engine.emit_quiet(&Event::RunFinished { verified, total });
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
    engine.set_state(HarnessState::Done);
    engine.refresh_repo();
}

fn count_verified(engine: &Engine) -> (u32, u32) {
    let tasks = engine.store().list_tasks().unwrap_or_default();
    let verified = tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Verified)
        .count();
    (
        u32::try_from(verified).unwrap_or(0),
        u32::try_from(tasks.len()).unwrap_or(0),
    )
}

/// How a task that has not reached an end gets one: the statuses it passes through, the last one
/// being the end, and the reason. `None` for a task that already ended.
fn close_plan(status: TaskStatus, why: &str) -> Option<(&'static [TaskStatus], String)> {
    match status {
        TaskStatus::Planned | TaskStatus::Ready | TaskStatus::Waiting => Some((
            &[TaskStatus::Skipped],
            format!("{why} before this task started."),
        )),
        TaskStatus::Reserved | TaskStatus::Running => Some((
            &[TaskStatus::Failed, TaskStatus::Blocked],
            format!("{why} while this task was running."),
        )),
        TaskStatus::Failed => Some((
            &[TaskStatus::Blocked],
            format!("{why} while this task was failing."),
        )),
        TaskStatus::Checkpointed | TaskStatus::Verifying => Some((
            &[TaskStatus::Verifying, TaskStatus::Partial],
            format!("{why} before this task was verified. Its commit is kept."),
        )),
        TaskStatus::Verified
        | TaskStatus::Blocked
        | TaskStatus::Contradiction
        | TaskStatus::Skipped
        | TaskStatus::Partial => None,
    }
}

fn close_out(ctx: &RunCtx) -> Result<()> {
    let store = ctx.engine.store();
    let why = if ctx.stopped() {
        "The run was stopped"
    } else {
        "The run ended"
    };
    for task in store.list_tasks()? {
        let Some((steps, reason)) = close_plan(task.status, why) else {
            continue;
        };
        for step in steps {
            store.set_task_status(&task.id, *step)?;
        }
        if let Some(end) = steps.last() {
            record_end(ctx, &task.id, *end, &reason);
        }
    }
    Ok(())
}
