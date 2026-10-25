//! Live project state: the working tree, one file's diff, and the backlog tasks. Any device
//! token may read them, the same as `GET /api/state`.

use axum::Router;
use axum::extract::{Query, State};
use axum::routing::get;
use serde::Deserialize;

use sentinel_core::api::{BacklogTask, ChangeDiff, ChangedFile};

use super::{ApiResult, blocking};
use crate::changes;
use crate::engine::Engine;

pub(super) fn routes() -> Router<Engine> {
    Router::new()
        .route("/api/changes", get(list_changes))
        .route("/api/changes/diff", get(change_diff))
        .route("/api/backlog/tasks", get(list_backlog))
}

#[derive(Debug, Deserialize)]
struct DiffQuery {
    /// A path relative to the repository root.
    path: String,
}

async fn list_changes(State(engine): State<Engine>) -> ApiResult<Vec<ChangedFile>> {
    blocking(engine, |e| changes::working_tree(&e.repo_path())).await
}

async fn change_diff(
    State(engine): State<Engine>,
    Query(query): Query<DiffQuery>,
) -> ApiResult<ChangeDiff> {
    blocking(engine, move |e| {
        changes::file_diff(&e.repo_path(), &query.path)
    })
    .await
}

async fn list_backlog(State(engine): State<Engine>) -> ApiResult<Vec<BacklogTask>> {
    blocking(engine, |e| {
        changes::backlog_tasks(&e.repo_path(), &e.config().backlog)
    })
    .await
}
