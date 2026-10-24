//! The dry run routes. Only the computer starts one. Any device may read the last one, so a page
//! that reloads in the middle of it can pick up where it was.

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use serde_json::{Value, json};

use sentinel_core::dry_run::DryRunStatus;

use super::{ApiFailure, ApiResult};
use crate::auth::Computer;
use crate::engine::Engine;

pub(super) fn routes() -> Router<Engine> {
    Router::new().route("/api/dry-run", get(status).post(start))
}

async fn start(
    State(engine): State<Engine>,
    Computer(_): Computer,
) -> Result<(StatusCode, Json<Value>), ApiFailure> {
    engine.start_dry_run()?;
    Ok((StatusCode::ACCEPTED, Json(json!({ "started": true }))))
}

async fn status(State(engine): State<Engine>) -> ApiResult<DryRunStatus> {
    Ok(Json(engine.dry_run_status()))
}
