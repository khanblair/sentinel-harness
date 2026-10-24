//! Repository choice, the first-run steps, and the settings only the computer may change.

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{get, post, put};
use serde::Deserialize;
use serde_json::{Value, json};

use sentinel_core::OnboardingState;
use sentinel_core::api::{RepoCheckRequest, RepoCheckResponse};

use super::state::ConnectionBody;
use super::{ApiResult, blocking, okay};
use crate::auth::Computer;
use crate::controls::usd_text;
use crate::engine::{Engine, EngineError};

pub(super) fn routes() -> Router<Engine> {
    Router::new()
        .route("/api/config/cap", put(set_cap))
        .route("/api/secrets/openrouter", put(set_openrouter_key))
        .route("/api/connections/{name}/test", post(test_connection))
        .route("/api/files", get(repo_files))
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
    Json(body): Json<RepoCheckRequest>,
) -> ApiResult<RepoCheckResponse> {
    blocking(engine, move |e| {
        Ok::<_, EngineError>(e.repo_check(&body.path))
    })
    .await
}

async fn starter_backlog(
    State(engine): State<Engine>,
    Computer(_): Computer,
    Json(body): Json<RepoCheckRequest>,
) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.create_starter_backlog(&body.path).map(|()| okay())
    })
    .await
}

async fn set_repo(
    State(engine): State<Engine>,
    Json(body): Json<RepoCheckRequest>,
) -> ApiResult<RepoCheckResponse> {
    blocking(engine, move |e| e.set_repo(&body.path)).await
}

async fn onboarding(State(engine): State<Engine>) -> ApiResult<OnboardingState> {
    blocking(engine, Engine::onboarding).await
}

async fn onboarding_step(
    State(engine): State<Engine>,
    Computer(_): Computer,
    Path(step): Path<u8>,
) -> ApiResult<OnboardingState> {
    blocking(engine, move |e| e.onboarding_step(step)).await
}

async fn onboarding_skip(
    State(engine): State<Engine>,
    Computer(_): Computer,
    Path(step): Path<u8>,
) -> ApiResult<OnboardingState> {
    blocking(engine, move |e| e.onboarding_skip(step)).await
}

async fn onboarding_complete(
    State(engine): State<Engine>,
    Computer(_): Computer,
) -> ApiResult<OnboardingState> {
    blocking(engine, Engine::onboarding_complete).await
}

async fn onboarding_reset(
    State(engine): State<Engine>,
    Computer(_): Computer,
) -> ApiResult<OnboardingState> {
    blocking(engine, Engine::onboarding_reset).await
}

#[derive(Debug, Deserialize)]
struct CapBody {
    cap: String,
}

#[derive(Debug, Deserialize)]
struct KeyBody {
    key: String,
}

/// Set the cap before a run. Once a run is live the cap can only be lowered.
async fn set_cap(
    State(engine): State<Engine>,
    Computer(caller): Computer,
    Json(body): Json<CapBody>,
) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.set_cap(&body.cap, &caller)
            .map(|cap| json!({ "cap": usd_text(cap) }))
    })
    .await
}

async fn set_openrouter_key(
    State(engine): State<Engine>,
    Computer(_): Computer,
    Json(body): Json<KeyBody>,
) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.set_openrouter_key(&body.key).map(|()| okay())
    })
    .await
}

async fn test_connection(
    State(engine): State<Engine>,
    Computer(_): Computer,
    Path(name): Path<String>,
) -> ApiResult<ConnectionBody> {
    Ok(Json(engine.test_connection(&name).await?.into()))
}

async fn repo_files(State(engine): State<Engine>) -> ApiResult<Vec<String>> {
    blocking(engine, Engine::repo_files).await
}
