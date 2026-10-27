//! One wave: reserve files, run the tasks together, wait at the barrier, run the full suite on a
//! clean tree, and verify.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use sentinel_core::{Event, TaskId, TaskStatus};
use sentinel_gate::policy::{WaveGate, wave_gate};
use sentinel_reserve::{Barrier, BarrierDecision};
use sentinel_store::VerificationRecord;
use sentinel_tree::{Checkpoint, restore_path, revert_checkpoint};
use sentinel_verify::Protected;
use sentinel_verify::gate::{SuiteConfig, SuiteResult, run_suite};
use sentinel_verify::protect::tamper_verdict;
use sentinel_verify::verdict::Verdict;
use tokio::task::JoinSet;

use crate::ask::ask_approval;
use crate::attempt::{record_end, run_attempt};
use crate::engine::{EngineError, Result, lock};
use crate::run_ctx::RunCtx;
use crate::verify_phase::{send_back_by_id, verify_wave};

/// The full suite may take a while on a real repository.
const SUITE_TIMEOUT: Duration = Duration::from_mins(15);

pub(crate) enum WaveEnd {
    /// The wave ran.
    Ran,
    /// Nothing is ready: the run is finished.
    Empty,
    /// The run must stop: stop signal, budget, or a denied wave.
    Stop,
}

pub(crate) async fn run_wave(ctx: &Arc<RunCtx>, number: u32) -> Result<WaveEnd> {
    ctx.wait_unpaused().await;
    if ctx.stopped() {
        return Ok(WaveEnd::Stop);
    }
    if !ctx.budget.can_start_task() {
        ctx.engine.emit_quiet(&Event::Message {
            from: "sentinel".to_owned(),
            to: None,
            text: "70% of the cap is spent, so no new task starts.".to_owned(),
        });
        return Ok(WaveEnd::Stop);
    }
    skip_unrunnable(ctx)?;
    let wave = lock(&ctx.scheduler)
        .next_wave(
            number,
            usize::try_from(ctx.config.max_parallel).unwrap_or(1).max(1),
            ctx.engine.now(),
        )
        .map_err(|e| EngineError::Invalid(e.to_string()))?;
    if wave.tasks.is_empty() {
        return Ok(WaveEnd::Empty);
    }
    if !gate_wave(ctx, number).await {
        let _ = lock(&ctx.scheduler).recover_after_crash(ctx.engine.now());
        return Ok(WaveEnd::Stop);
    }
    run_tasks(ctx, number, &wave.tasks).await?;
    barrier_and_verify(ctx, number, &wave.tasks).await?;
    Ok(WaveEnd::Ran)
}

/// Tasks that wait on a task that ended badly can never run: say so and skip them.
fn skip_unrunnable(ctx: &RunCtx) -> Result<()> {
    let ids = lock(&ctx.scheduler)
        .unrunnable_tasks()
        .map_err(|e| EngineError::Invalid(e.to_string()))?;
    for id in ids {
        ctx.engine
            .store()
            .set_task_status(&id, TaskStatus::Skipped)?;
        record_end(
            ctx,
            &id,
            TaskStatus::Skipped,
            "A task this one depends on did not finish, so it never started.",
        );
    }
    Ok(())
}

async fn gate_wave(ctx: &Arc<RunCtx>, number: u32) -> bool {
    if wave_gate(ctx.permission_mode()) == WaveGate::None {
        return true;
    }
    let card = lock(&ctx.book).request_wave(number, ctx.engine.now());
    ask_approval(ctx, card).await
}

async fn run_tasks(ctx: &Arc<RunCtx>, number: u32, tasks: &[TaskId]) -> Result<()> {
    ctx.engine.emit(&Event::WaveStarted {
        wave: number,
        tasks: tasks.to_vec(),
    })?;
    if let Some(run) = lock(&ctx.engine.inner.live).run.as_mut() {
        run.wave = number;
    }
    let mut set = JoinSet::new();
    for id in tasks {
        let ctx = Arc::clone(ctx);
        let id = id.clone();
        set.spawn(async move { run_attempt(&ctx, id).await });
    }
    while let Some(joined) = set.join_next().await {
        if let Err(error) = joined {
            eprintln!("sentinel: a task runner crashed: {error}");
        }
    }
    Ok(())
}

pub(crate) async fn barrier_and_verify(
    ctx: &Arc<RunCtx>,
    number: u32,
    tasks: &[TaskId],
) -> Result<()> {
    let checkpointed = checkpointed(ctx, tasks)?;
    let checkpointed = guard_protected(ctx, &checkpointed)?;
    let mut barrier = Barrier::new(number, tasks.len());
    for _ in tasks {
        barrier.task_finished();
    }
    if checkpointed.is_empty() {
        return Ok(());
    }
    restore_strays(ctx);
    let clean = ctx
        .git
        .is_clean()
        .map_err(|e| EngineError::Invalid(e.to_string()))?;
    match barrier.decide(clean) {
        BarrierDecision::RunSuite => {}
        BarrierDecision::RefuseDirty | BarrierDecision::WaitForTasks => {
            ctx.engine.emit(&Event::Message {
                from: "sentinel".to_owned(),
                to: None,
                text: "The barrier did not run: the tree is not clean. Look at the Changes panel."
                    .to_owned(),
            })?;
            return fail_wave(ctx, &checkpointed, "The tree was not clean at the barrier.");
        }
    }
    let suite = run_barrier(ctx, number).await?;
    barrier.suite_finished(suite.passed);
    verify_wave(ctx, &suite, &checkpointed).await
}

/// Run the full suite on the clean tree and record it as a barrier.
async fn run_barrier(ctx: &RunCtx, number: u32) -> Result<SuiteResult> {
    let engine = &ctx.engine;
    engine.emit(&Event::BarrierStart { wave: number })?;
    let barrier_id = engine.store().start_barrier(number, engine.now())?;
    let suite = run_the_suite(ctx, number).await?;
    engine.store().finish_barrier(
        barrier_id,
        engine.now(),
        suite.passed,
        &suite.log_ref.to_string_lossy(),
    )?;
    engine.emit(&Event::BarrierEnd {
        wave: number,
        passed: suite.passed,
    })?;
    Ok(suite)
}

fn checkpointed(ctx: &RunCtx, tasks: &[TaskId]) -> Result<Vec<TaskId>> {
    let mut found = Vec::new();
    for id in tasks {
        if ctx
            .engine
            .store()
            .get_task(id)?
            .is_some_and(|t| matches!(t.status, TaskStatus::Checkpointed | TaskStatus::Verifying))
        {
            found.push(id.clone());
        }
    }
    Ok(found)
}

/// Files nobody holds any more but the tree still differs: put them back, and say so.
fn restore_strays(ctx: &RunCtx) {
    let Ok(changed) = ctx.git.changed_paths() else {
        return;
    };
    for path in changed {
        if restore_path(&ctx.git, &path, "HEAD").is_ok() {
            ctx.engine.emit_quiet(&Event::WriteBlocked {
                task: None,
                path,
                restored: true,
            });
        }
    }
}

/// Protected files must not change. A checkpoint that touched one is reverted and blocked. A
/// protected file that still differs from its lock hash goes back to git. Returns the tasks that
/// may still be verified.
fn guard_protected(ctx: &RunCtx, checkpointed: &[TaskId]) -> Result<Vec<TaskId>> {
    let root = ctx.engine.repo_path();
    let protected = Protected::load(&root, &ctx.config.protected_paths);
    let mut kept = Vec::new();
    for id in checkpointed {
        let Some(checkpoint) = lock(&ctx.memory).checkpoints.get(id).cloned() else {
            kept.push(id.clone());
            continue;
        };
        let diff = ctx
            .git
            .run(&["show", "--format=", "--no-color", "--no-renames", &checkpoint.commit])
            .map_err(|e| EngineError::Invalid(e.to_string()))?;
        match tamper_verdict(&protected.touched_by_diff(&diff)) {
            Some(verdict) => block_tampered(ctx, id, &checkpoint, &verdict)?,
            None => kept.push(id.clone()),
        }
    }
    restore_changed_protected(ctx, &protected, &root);
    Ok(kept)
}

/// The checkpoint touched a protected file: undo it and block the task for good. A retry would
/// only repeat the tamper.
fn block_tampered(
    ctx: &RunCtx,
    id: &TaskId,
    checkpoint: &Checkpoint,
    verdict: &Verdict,
) -> Result<()> {
    revert_checkpoint(&ctx.git, checkpoint, &format!("revert {id}: {}", verdict.reason))
        .map_err(|e| EngineError::Invalid(e.to_string()))?;
    let record = VerificationRecord {
        task: id.clone(),
        tests_pass: false,
        new_test: false,
        reviewer_ok: false,
        tamper_clear: false,
        verdict: verdict.status,
        reason: verdict.reason.clone(),
        receipt_json: "null".to_owned(),
    };
    ctx.engine.store().upsert_verification(&record)?;
    ctx.engine.store().set_task_status(id, verdict.status)?;
    ctx.engine.emit(&Event::TaskStatus {
        task: id.clone(),
        status: verdict.status,
    })?;
    ctx.engine.emit(&Event::Verdict {
        task: id.clone(),
        status: verdict.status,
        reason: verdict.reason.clone(),
    })?;
    Ok(())
}

/// Restore each protected file that differs from its lock hash, and say whether it worked.
fn restore_changed_protected(ctx: &RunCtx, protected: &Protected, root: &Path) {
    let changed = protected.changed(root);
    for path in &changed {
        let _ = restore_path(&ctx.git, path, "HEAD");
    }
    let still = protected.changed(root);
    for path in changed {
        let restored = !still.contains(&path);
        ctx.engine.emit_quiet(&Event::WriteBlocked {
            task: None,
            path,
            restored,
        });
    }
}

async fn run_the_suite(ctx: &RunCtx, number: u32) -> Result<SuiteResult> {
    let config = SuiteConfig {
        command: ctx.config.test_command.clone(),
        timeout: SUITE_TIMEOUT,
        log_dir: ctx.suite_dir.clone(),
    };
    run_suite(&ctx.engine.repo_path(), &config, &format!("wave-{number}"))
        .await
        .map_err(|e| EngineError::Invalid(e.to_string()))
}

/// The barrier could not run: every checkpointed task of the wave goes back.
fn fail_wave(ctx: &RunCtx, tasks: &[TaskId], reason: &str) -> Result<()> {
    for id in tasks {
        send_back_by_id(ctx, id, reason)?;
    }
    Ok(())
}
