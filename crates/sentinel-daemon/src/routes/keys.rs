//! The model provider keys: one row per provider, a save, a delete, and a test of the saved key.
//! Only the computer may change them, and no answer carries a key.

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use sentinel_core::api::{KeyBody, KeyInfo, KeyTest};
use serde_json::Value;

use super::{ApiResult, blocking, okay};
use crate::auth::Computer;
use crate::engine::{Engine, EngineError};

pub(super) fn routes() -> Router<Engine> {
    Router::new()
        .route("/api/keys", get(list_keys))
        .route(
            "/api/keys/{provider}",
            axum::routing::put(save_key).delete(delete_key),
        )
        .route("/api/keys/{provider}/test", post(test_key))
}

async fn list_keys(State(engine): State<Engine>, Computer(_): Computer) -> ApiResult<Vec<KeyInfo>> {
    blocking(engine, |e| Ok::<_, EngineError>(e.key_rows())).await
}

async fn save_key(
    State(engine): State<Engine>,
    Computer(_): Computer,
    Path(provider): Path<String>,
    Json(body): Json<KeyBody>,
) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.save_key(&provider, &body.key).map(|()| okay())
    })
    .await
}

async fn delete_key(
    State(engine): State<Engine>,
    Computer(_): Computer,
    Path(provider): Path<String>,
) -> ApiResult<Value> {
    blocking(engine, move |e| e.delete_key(&provider).map(|()| okay())).await
}

async fn test_key(
    State(engine): State<Engine>,
    Computer(_): Computer,
    Path(provider): Path<String>,
) -> ApiResult<KeyTest> {
    Ok(Json(engine.test_key(&provider).await?))
}
