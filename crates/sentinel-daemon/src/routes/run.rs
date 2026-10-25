//! Routes that start, steer, and answer a run.

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::post;
use serde::Deserialize;
use serde_json::{Value, json};

use sentinel_core::BrainMode;
use sentinel_core::api::{AnswerApproval, AnswerDecision, LowerCap, RunRequest, SetBrainMode};
use sentinel_core::ids::{ApprovalId, DecisionId, TaskId, ToolId};
use sentinel_core::permission::Approval;
use sentinel_core::snapshot::RunInfo;
use sentinel_core::state::Decision;

use super::{ApiResult, blocking, okay};
use crate::controls::{Caller, usd_text};
use crate::engine::Engine;

#[derive(Debug, Deserialize)]
struct MessageBody {
    text: String,
    to: Option<ToolId>,
}

pub(super) fn routes() -> Router<Engine> {
    Router::new()
        .route("/api/run", post(start_run))
        .route("/api/pause", post(pause))
        .route("/api/resume", post(resume))
        .route("/api/stop", post(stop))
        .route("/api/tasks/{id}/kill", post(kill_task))
        .route("/api/agents/{id}/kill", post(kill_agent))
        .route("/api/cap/lower", post(lower_cap))
        .route("/api/decisions/{id}", post(answer_decision))
        .route("/api/approvals/{id}", post(answer_approval))
        .route("/api/message", post(send_message))
        .route("/api/brain/mode", post(set_brain_mode))
}

/// `null` when the message got a reply and no run started.
async fn start_run(
    State(engine): State<Engine>,
    caller: Caller,
    Json(request): Json<RunRequest>,
) -> ApiResult<Option<RunInfo>> {
    Ok(Json(engine.start_run_as(request, &caller).await?))
}

async fn pause(State(engine): State<Engine>) -> ApiResult<Value> {
    blocking(engine, |e| e.pause_run().map(|()| okay())).await
}

async fn resume(State(engine): State<Engine>) -> ApiResult<Value> {
    blocking(engine, |e| e.resume_run().map(|()| okay())).await
}

async fn stop(State(engine): State<Engine>) -> ApiResult<Value> {
    blocking(engine, |e| e.stop_run().map(|()| okay())).await
}

async fn kill_task(State(engine): State<Engine>, Path(id): Path<String>) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.kill_task(&TaskId::new(id)).map(|()| okay())
    })
    .await
}

async fn kill_agent(State(engine): State<Engine>, Path(id): Path<String>) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.kill_agent(&ToolId::new(id)).map(|()| okay())
    })
    .await
}

/// The only cap route. It lowers the cap and never raises it.
async fn lower_cap(
    State(engine): State<Engine>,
    caller: Caller,
    Json(body): Json<LowerCap>,
) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.lower_cap(&body.cap, &caller)
            .map(|cost| json!({ "cap": usd_text(cost) }))
    })
    .await
}

async fn answer_decision(
    State(engine): State<Engine>,
    caller: Caller,
    Path(id): Path<String>,
    Json(body): Json<AnswerDecision>,
) -> ApiResult<Decision> {
    blocking(engine, move |e| {
        e.answer_decision(&DecisionId::new(id), &body.answer, &caller)
    })
    .await
}

async fn answer_approval(
    State(engine): State<Engine>,
    caller: Caller,
    Path(id): Path<String>,
    Json(body): Json<AnswerApproval>,
) -> ApiResult<Approval> {
    blocking(engine, move |e| {
        e.answer_approval(&ApprovalId::new(id), body.allow, &caller)
    })
    .await
}

async fn send_message(
    State(engine): State<Engine>,
    caller: Caller,
    Json(body): Json<MessageBody>,
) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.send_message(&body.text, body.to, &caller)
            .map(|()| okay())
    })
    .await
}

async fn set_brain_mode(
    State(engine): State<Engine>,
    caller: Caller,
    Json(body): Json<SetBrainMode>,
) -> ApiResult<BrainMode> {
    blocking(engine, move |e| e.set_brain_mode(body.mode, &caller)).await
}
