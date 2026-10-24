//! Questions the run loop puts to the user: approval cards and decisions. Each one waits for an
//! answer, a timeout, or a stop. A question nobody answers has a safe default.

use std::sync::Arc;
use std::time::Duration;

use sentinel_core::{Approval, ApprovalState, Decision, DecisionState, Event};
use tokio::sync::oneshot;

use crate::engine::lock;
use crate::run_ctx::RunCtx;

/// Put an approval card in the stream and wait. Returns true only when someone allowed it. A card
/// nobody answers in `approval_timeout_seconds` is denied (state `Defaulted`).
pub(crate) async fn ask_approval(ctx: &Arc<RunCtx>, approval: Approval) -> bool {
    let engine = &ctx.engine;
    let (sender, receiver) = oneshot::channel();
    if engine
        .store()
        .insert_approval(&approval, engine.now())
        .is_err()
    {
        return false;
    }
    lock(&engine.inner.live)
        .approvals
        .insert(approval.id.clone(), sender);
    engine.emit_quiet(Event::ApprovalRequested {
        approval: approval.clone(),
    });
    let timeout = Duration::from_secs(ctx.config.approval_timeout_seconds);
    let mut stop = ctx.stop.clone();
    let answer = tokio::select! {
        answer = receiver => answer.ok(),
        () = tokio::time::sleep(timeout) => None,
        _ = stop.wait_for(|stopped| *stopped) => None,
    };
    if let Some(allow) = answer {
        return allow;
    }
    lock(&engine.inner.live).approvals.remove(&approval.id);
    let saved = engine.store().answer_approval(
        &approval.id,
        ApprovalState::Defaulted,
        "default",
        engine.now(),
    );
    if saved.is_ok() {
        engine.emit_quiet(Event::ApprovalResolved {
            approval: approval.id.clone(),
            state: ApprovalState::Defaulted,
            decided_by: "default".to_owned(),
        });
    }
    false
}

/// Put a decision in the stream and wait. Returns the answer, or `None` when nobody answered in
/// `decision_timeout_seconds` (the caller then skips both tasks).
pub(crate) async fn ask_decision(ctx: &Arc<RunCtx>, decision: &Decision) -> Option<String> {
    let engine = &ctx.engine;
    let (sender, receiver) = oneshot::channel();
    engine
        .store()
        .insert_decision(decision, engine.now())
        .ok()?;
    lock(&engine.inner.live)
        .decisions
        .insert(decision.id.clone(), sender);
    engine.emit_quiet(Event::DecisionRequested {
        decision: decision.clone(),
    });
    let timeout = Duration::from_secs(ctx.config.decision_timeout_seconds);
    let mut stop = ctx.stop.clone();
    let answer = tokio::select! {
        answer = receiver => answer.ok(),
        () = tokio::time::sleep(timeout) => None,
        _ = stop.wait_for(|stopped| *stopped) => None,
    };
    if answer.is_none() {
        lock(&engine.inner.live).decisions.remove(&decision.id);
        // The decision may have been answered a moment ago: then the store refuses and that is fine.
        let _ = engine.store().answer_decision(
            &decision.id,
            DecisionState::Defaulted,
            None,
            "default",
            engine.now(),
        );
    }
    answer
}
