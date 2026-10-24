//! After the barrier: check each checkpointed task of the wave and give it a status. Only
//! `sentinel_verify::verdict::decide` can say Verified, from evidence, never from agent text.

use std::sync::Arc;

use sentinel_core::{Event, TaskId, TaskStatus};
use sentinel_store::VerificationRecord;
use sentinel_tree::{Checkpoint, edited_tests, revert_checkpoint};
use sentinel_verify::gate::SuiteResult;
use sentinel_verify::quality::quality_notes;
use sentinel_verify::verdict::{Checks, Verdict, adds_new_test, decide};
use sentinel_verify::verifier::ReviewRequest;
use sentinel_workers::context::{FailureInput, failure_summary};

use crate::attempt::record_end;
use crate::engine::{EngineError, Result, lock};
use crate::run_ctx::RunCtx;

pub(crate) async fn verify_wave(
    ctx: &Arc<RunCtx>,
    suite: &SuiteResult,
    tasks: &[TaskId],
) -> Result<()> {
    for id in tasks {
        let Some(task) = ctx.engine.store().get_task(id)? else {
            continue;
        };
        if task.status == TaskStatus::Checkpointed {
            verify_one(ctx, suite, id).await?;
        }
    }
    Ok(())
}

async fn verify_one(ctx: &Arc<RunCtx>, suite: &SuiteResult, id: &TaskId) -> Result<()> {
    let engine = &ctx.engine;
    let Some(checkpoint) = lock(&ctx.memory).checkpoints.get(id).cloned() else {
        return Ok(());
    };
    engine.store().set_task_status(id, TaskStatus::Verifying)?;
    engine.emit(Event::TaskStatus {
        task: id.clone(),
        status: TaskStatus::Verifying,
    })?;
    let diff = ctx
        .git
        .run(&["show", "--format=", "--no-color", &checkpoint.commit])
        .map_err(|e| EngineError::Invalid(e.to_string()))?;
    let tamper_clear = edited_tests(
        &ctx.git,
        &format!("{}^", checkpoint.commit),
        &checkpoint.commit,
    )
    .map_err(|e| EngineError::Invalid(e.to_string()))?
    .is_empty();
    let task = engine
        .store()
        .get_task(id)?
        .ok_or_else(|| EngineError::NotFound(id.to_string()))?;
    let request = ReviewRequest {
        task_id: id.clone(),
        title: task.title.clone(),
        body: task.body.clone(),
        diff: diff.clone(),
    };
    let review = ctx
        .reviewer
        .review(&request)
        .await
        .map_err(|e| EngineError::Invalid(e.to_string()))?;
    let checks = Checks {
        tests_pass: suite.passed,
        new_test: adds_new_test(&diff),
        reviewer_ok: review.ok,
        tamper_clear,
    };
    let verdict = decide(&checks);
    store_notes(ctx, id, &diff)?;
    store_verification(
        ctx,
        id,
        &checks,
        &verdict,
        &checkpoint,
        &review.reviewer,
        suite,
    )?;
    match verdict.status {
        TaskStatus::Failed => send_back(
            ctx,
            id,
            &checkpoint,
            &verdict.reason,
            &suite.failing_tests,
            &suite.tail.join("\n"),
        ),
        status => {
            engine.store().set_task_status(id, status)?;
            engine.emit(Event::TaskStatus {
                task: id.clone(),
                status,
            })?;
            engine.emit(Event::Verdict {
                task: id.clone(),
                status,
                reason: detail(&verdict, &review.reason),
            })?;
            Ok(())
        }
    }
}

fn detail(verdict: &Verdict, review_reason: &str) -> String {
    if verdict.status == TaskStatus::Verified {
        verdict.reason.clone()
    } else {
        format!("{} Reviewer: {review_reason}", verdict.reason)
    }
}

fn store_notes(ctx: &RunCtx, id: &TaskId, diff: &str) -> Result<()> {
    for note in quality_notes(id, diff) {
        ctx.engine
            .store()
            .insert_quality_note(&note, ctx.engine.now())?;
        ctx.engine.emit(Event::QualityNote { note })?;
    }
    Ok(())
}

fn store_verification(
    ctx: &RunCtx,
    id: &TaskId,
    checks: &Checks,
    verdict: &Verdict,
    checkpoint: &Checkpoint,
    reviewer: &str,
    suite: &SuiteResult,
) -> Result<()> {
    let receipt = serde_json::json!({
        "commit": checkpoint.commit,
        "suite_log": suite.log_ref.to_string_lossy(),
        "reviewer": reviewer,
    });
    ctx.engine
        .store()
        .upsert_verification(&VerificationRecord {
            task: id.clone(),
            tests_pass: checks.tests_pass,
            new_test: checks.new_test,
            reviewer_ok: checks.reviewer_ok,
            tamper_clear: checks.tamper_clear,
            verdict: verdict.status,
            reason: verdict.reason.clone(),
            receipt_json: receipt.to_string(),
        })?;
    Ok(())
}

/// A task whose wave could not be checked (for example, a dirty tree at the barrier): undo its
/// commit and send it back, the same as a failed verification.
pub(crate) fn send_back_by_id(ctx: &RunCtx, id: &TaskId, reason: &str) -> Result<()> {
    let Some(checkpoint) = lock(&ctx.memory).checkpoints.get(id).cloned() else {
        return Ok(());
    };
    ctx.engine
        .store()
        .set_task_status(id, TaskStatus::Verifying)?;
    send_back(ctx, id, &checkpoint, reason, &[], "")
}

/// The task failed verification. Undo its commit so the next wave does not inherit its code,
/// then retry it one tier up, or block it when its tries are used up.
fn send_back(
    ctx: &RunCtx,
    id: &TaskId,
    checkpoint: &Checkpoint,
    reason: &str,
    failing_tests: &[String],
    tail: &str,
) -> Result<()> {
    let engine = &ctx.engine;
    revert_checkpoint(&ctx.git, checkpoint, &format!("revert {id}: {reason}"))
        .map_err(|e| EngineError::Invalid(e.to_string()))?;
    engine.store().set_task_status(id, TaskStatus::Failed)?;
    let summary = failure_summary(
        &FailureInput {
            failing_tests,
            error_blocks: &[reason.to_owned()],
            output: tail,
        },
        &ctx.limits,
    );
    lock(&ctx.memory).failures.insert(id.clone(), summary);
    let task = engine
        .store()
        .get_task(id)?
        .ok_or_else(|| EngineError::NotFound(id.to_string()))?;
    let router = engine.router()?;
    if task.attempts >= router.retry_limit() {
        engine.store().set_task_status(id, TaskStatus::Blocked)?;
        record_end(
            ctx,
            id,
            TaskStatus::Blocked,
            &format!("{reason} It failed {} times.", task.attempts),
        );
        return Ok(());
    }
    engine.store().set_task_status(id, TaskStatus::Ready)?;
    raise_tier(ctx, id, task.tier);
    engine.emit(Event::TaskStatus {
        task: id.clone(),
        status: TaskStatus::Ready,
    })?;
    engine.emit(Event::Message {
        from: "sentinel".to_owned(),
        to: None,
        text: format!(
            "{id} did not pass: {reason} Its change was undone. It goes back for another try."
        ),
    })?;
    Ok(())
}

fn raise_tier(ctx: &RunCtx, id: &TaskId, base: sentinel_core::Tier) {
    let mut memory = lock(&ctx.memory);
    let current = memory.tiers.get(id).copied().unwrap_or(base);
    if let Some(next) = current.escalate() {
        memory.tiers.insert(id.clone(), next);
    }
}
