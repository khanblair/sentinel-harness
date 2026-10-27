//! After the barrier: check each checkpointed task of the wave and give it a status. Only
//! `sentinel_verify::verdict::decide` can say Verified, from evidence, never from agent text.

use std::sync::Arc;

use sentinel_core::{Event, TaskId, TaskStatus};
use sentinel_store::VerificationRecord;
use sentinel_tree::{Checkpoint, edited_tests};
use sentinel_verify::gate::SuiteResult;
use sentinel_verify::quality::quality_notes;
use sentinel_verify::verdict::{Checks, Verdict, adds_new_test, decide};
use sentinel_verify::verifier::{Review, ReviewRequest};
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
        if matches!(
            task.status,
            TaskStatus::Checkpointed | TaskStatus::Verifying
        ) {
            verify_one(ctx, suite, id).await?;
        }
    }
    Ok(())
}

/// The evidence for one verdict, read from the commit the agent made.
struct Evidence {
    diff: String,
    tamper_clear: bool,
}

/// What was decided about one task, with the receipt data.
struct Judgement<'a> {
    checks: Checks,
    verdict: Verdict,
    review: Review,
    checkpoint: &'a Checkpoint,
    suite: &'a SuiteResult,
}

/// Why a task goes back: the reason and what the suite said.
struct Failure<'a> {
    reason: &'a str,
    failing_tests: &'a [String],
    tail: &'a str,
}

fn invalid(error: impl std::fmt::Display) -> EngineError {
    EngineError::Invalid(error.to_string())
}

fn set_status(ctx: &RunCtx, id: &TaskId, status: TaskStatus) -> Result<()> {
    ctx.engine.store().set_task_status(id, status)?;
    ctx.engine.emit(&Event::TaskStatus {
        task: id.clone(),
        status,
    })?;
    Ok(())
}

fn read_evidence(ctx: &RunCtx, checkpoint: &Checkpoint) -> Result<Evidence> {
    let diff = ctx
        .git
        .run(&["show", "--format=", "--no-color", &checkpoint.commit])
        .map_err(invalid)?;
    let parent = format!("{}^", checkpoint.commit);
    let edited = edited_tests(&ctx.git, &parent, &checkpoint.commit).map_err(invalid)?;
    Ok(Evidence {
        diff,
        tamper_clear: edited.is_empty(),
    })
}

async fn verify_one(ctx: &Arc<RunCtx>, suite: &SuiteResult, id: &TaskId) -> Result<()> {
    let Some(checkpoint) = lock(&ctx.memory).checkpoints.get(id).cloned() else {
        return Ok(());
    };
    set_status(ctx, id, TaskStatus::Verifying)?;
    let evidence = read_evidence(ctx, &checkpoint)?;
    let task = ctx
        .engine
        .store()
        .get_task(id)?
        .ok_or_else(|| EngineError::NotFound(id.to_string()))?;
    let request = ReviewRequest {
        task_id: id.clone(),
        title: task.title,
        body: task.body,
        diff: evidence.diff.clone(),
    };
    let review = ctx.reviewer.review(&request).await.map_err(invalid)?;
    let checks = Checks {
        tests_pass: suite.passed,
        new_test: adds_new_test(&evidence.diff),
        reviewer_ok: review.ok,
        tamper_clear: evidence.tamper_clear,
    };
    let verdict = decide(&checks);
    store_notes(ctx, id, &evidence.diff)?;
    let judgement = Judgement {
        checks,
        verdict,
        review,
        checkpoint: &checkpoint,
        suite,
    };
    store_verification(ctx, id, &judgement)?;
    apply_verdict(ctx, id, &judgement)
}

fn apply_verdict(ctx: &RunCtx, id: &TaskId, judgement: &Judgement) -> Result<()> {
    let Judgement {
        verdict,
        review,
        suite,
        checkpoint,
        ..
    } = judgement;
    if verdict.status == TaskStatus::Failed {
        let tail = suite.tail.join("\n");
        let failure = Failure {
            reason: &verdict.reason,
            failing_tests: &suite.failing_tests,
            tail: &tail,
        };
        return send_back(ctx, id, checkpoint, &failure);
    }
    set_status(ctx, id, verdict.status)?;
    ctx.engine.emit(&Event::Verdict {
        task: id.clone(),
        status: verdict.status,
        reason: detail(verdict, &review.reason),
    })?;
    Ok(())
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
        ctx.engine.emit(&Event::QualityNote { note })?;
    }
    Ok(())
}

fn store_verification(ctx: &RunCtx, id: &TaskId, judgement: &Judgement) -> Result<()> {
    let receipt = serde_json::json!({
        "commit": judgement.checkpoint.commit,
        "suite_log": judgement.suite.log_ref.to_string_lossy(),
        "reviewer": judgement.review.reviewer,
    });
    ctx.engine
        .store()
        .upsert_verification(&VerificationRecord {
            task: id.clone(),
            tests_pass: judgement.checks.tests_pass,
            new_test: judgement.checks.new_test,
            reviewer_ok: judgement.checks.reviewer_ok,
            tamper_clear: judgement.checks.tamper_clear,
            verdict: judgement.verdict.status,
            reason: judgement.verdict.reason.clone(),
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
    let failure = Failure {
        reason,
        failing_tests: &[],
        tail: "",
    };
    send_back(ctx, id, &checkpoint, &failure)
}

/// The task failed verification. Undo its commit so the next wave does not inherit its code,
/// then retry it one tier up, or block it when its tries are used up.
fn send_back(ctx: &RunCtx, id: &TaskId, checkpoint: &Checkpoint, failure: &Failure) -> Result<()> {
    let reason = failure.reason;
    ctx.revert_quietly(checkpoint, &format!("revert {id}: {reason}"))
        .map_err(invalid)?;
    ctx.engine.store().set_task_status(id, TaskStatus::Failed)?;
    let summary = failure_summary(
        &FailureInput {
            failing_tests: failure.failing_tests,
            error_blocks: &[reason.to_owned()],
            output: failure.tail,
        },
        &ctx.limits,
    );
    lock(&ctx.memory).failures.insert(id.clone(), summary);
    let task = ctx
        .engine
        .store()
        .get_task(id)?
        .ok_or_else(|| EngineError::NotFound(id.to_string()))?;
    if task.attempts >= ctx.engine.router()?.retry_limit() {
        ctx.engine
            .store()
            .set_task_status(id, TaskStatus::Blocked)?;
        let note = format!("{reason} It failed {} times.", task.attempts);
        record_end(ctx, id, TaskStatus::Blocked, &note);
        return Ok(());
    }
    raise_tier(ctx, id, task.tier);
    set_status(ctx, id, TaskStatus::Ready)?;
    ctx.engine.emit(&Event::Message {
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
