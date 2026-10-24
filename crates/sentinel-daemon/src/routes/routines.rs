//! Routines and briefs. The computer makes and deletes routines. A phone may turn one off and press
//! Send now. The engine checks each of those rules, so a route only passes the caller on.

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post, put};
use serde::Deserialize;
use serde_json::Value;

use sentinel_core::RoutineId;
use sentinel_core::routines::{Brief, EditRoutine, NewRoutine, Routine};

use super::{ApiResult, blocking, okay};
use crate::auth::Computer;
use crate::controls::Caller;
use crate::engine::{Engine, EngineError};

const BRIEFS_DEFAULT: u32 = 14;
const BRIEFS_MAX: u32 = 50;

#[derive(Debug, Deserialize)]
pub(super) struct BriefQuery {
    limit: Option<u32>,
}

pub(super) fn routes() -> Router<Engine> {
    Router::new()
        .route("/api/routines", get(list).post(create))
        .route("/api/routines/{id}", put(edit).delete(delete))
        .route("/api/routines/{id}/run", post(send_now))
        .route("/api/briefs", get(briefs))
}

async fn list(State(engine): State<Engine>) -> ApiResult<Vec<Routine>> {
    blocking(engine, Engine::list_routines).await
}

async fn create(
    State(engine): State<Engine>,
    Computer(caller): Computer,
    Json(body): Json<NewRoutine>,
) -> ApiResult<Routine> {
    blocking(engine, move |e| e.create_routine(&body, &caller)).await
}

async fn edit(
    State(engine): State<Engine>,
    caller: Caller,
    Path(id): Path<String>,
    Json(body): Json<EditRoutine>,
) -> ApiResult<Routine> {
    blocking(engine, move |e| {
        e.edit_routine(&RoutineId::new(id), body, &caller)
    })
    .await
}

async fn delete(
    State(engine): State<Engine>,
    Computer(caller): Computer,
    Path(id): Path<String>,
) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.delete_routine(&RoutineId::new(id), &caller)
            .map(|()| okay())
    })
    .await
}

async fn send_now(
    State(engine): State<Engine>,
    caller: Caller,
    Path(id): Path<String>,
) -> ApiResult<Value> {
    engine
        .send_routine_now(&RoutineId::new(id), &caller)
        .await?;
    Ok(Json(okay()))
}

async fn briefs(
    State(engine): State<Engine>,
    Query(query): Query<BriefQuery>,
) -> ApiResult<Vec<Brief>> {
    let limit = query.limit.unwrap_or(BRIEFS_DEFAULT).clamp(1, BRIEFS_MAX);
    blocking(engine, move |e| {
        e.store().list_briefs(limit).map_err(EngineError::from)
    })
    .await
}
