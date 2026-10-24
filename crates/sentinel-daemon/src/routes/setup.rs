//! Repository choice and the first-run steps. The computer only.

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{get, post, put};
use serde_json::Value;

use sentinel_core::OnboardingState;
use sentinel_core::api::{RepoCheckRequest, RepoCheckResponse};

use super::{ApiResult, blocking, okay, require_computer};
use crate::controls::Caller;
use crate::engine::{Engine, EngineError};

pub(super) fn routes() -> Router<Engine> {
    Router::new()
        .route("/api/repo/check", post(repo_check))
        .route("/api/repo/starter-backlog", post(starter_backlog))
        .route("/api/repo", put(set_repo))
        .route("/api/onboarding", get(onboarding))
        .route("/api/onboarding/{step}", post(onboarding_step))
        .route("/api/onboarding/skip/{step}", post(onboarding_skip))
        .route("/api/onboarding/complete", post(onboarding_complete))
        .route("/api/onboarding/reset", post(onboarding_reset))
}

async fn repo_check(
    State(engine): State<Engine>,
    caller: Caller,
    Json(body): Json<RepoCheckRequest>,
) -> ApiResult<RepoCheckResponse> {
    require_computer(&caller)?;
    blocking(engine, move |e| {
        Ok::<_, EngineError>(e.repo_check(&body.path))
    })
    .await
}

async fn starter_backlog(
    State(engine): State<Engine>,
    caller: Caller,
    Json(body): Json<RepoCheckRequest>,
) -> ApiResult<Value> {
    require_computer(&caller)?;
    blocking(engine, move |e| {
        e.create_starter_backlog(&body.path).map(|()| okay())
    })
    .await
}

async fn set_repo(
    State(engine): State<Engine>,
    caller: Caller,
    Json(body): Json<RepoCheckRequest>,
) -> ApiResult<RepoCheckResponse> {
    require_computer(&caller)?;
    blocking(engine, move |e| e.set_repo(&body.path)).await
}

async fn onboarding(State(engine): State<Engine>) -> ApiResult<OnboardingState> {
    blocking(engine, Engine::onboarding).await
}

async fn onboarding_step(
    State(engine): State<Engine>,
    caller: Caller,
    Path(step): Path<u8>,
) -> ApiResult<OnboardingState> {
    require_computer(&caller)?;
    blocking(engine, move |e| e.onboarding_step(step)).await
}

async fn onboarding_skip(
    State(engine): State<Engine>,
    caller: Caller,
    Path(step): Path<u8>,
) -> ApiResult<OnboardingState> {
    require_computer(&caller)?;
    blocking(engine, move |e| e.onboarding_skip(step)).await
}

async fn onboarding_complete(
    State(engine): State<Engine>,
    caller: Caller,
) -> ApiResult<OnboardingState> {
    require_computer(&caller)?;
    blocking(engine, Engine::onboarding_complete).await
}

async fn onboarding_reset(
    State(engine): State<Engine>,
    caller: Caller,
) -> ApiResult<OnboardingState> {
    require_computer(&caller)?;
    blocking(engine, Engine::onboarding_reset).await
}
