//! Read routes and the settings that change what the daemon is allowed to use.

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post, put};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use sentinel_core::EventRecord;
use sentinel_core::api::{DoctorReport, PermissionsUpdate, RoutingUpdate, ToolToggle};
use sentinel_core::ids::{TaskId, ToolId};
use sentinel_core::permission::Approval;
use sentinel_core::snapshot::Snapshot;
use sentinel_core::state::ConnectionState;
use sentinel_core::tool::ToolInfo;

use super::{ApiResult, blocking, okay, require_computer};
use crate::connections::ConnectionStatus;
use crate::controls::Caller;
use crate::engine::{Engine, EngineError};

const DEFAULT_PAGE: usize = 500;
const MAX_PAGE: usize = 5000;

/// The page size a client asked for, kept between 1 and 5000.
pub fn page_limit(requested: Option<usize>) -> usize {
    requested.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE)
}

#[derive(Debug, Deserialize)]
pub(super) struct PageQuery {
    pub(super) after: Option<i64>,
    pub(super) limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(super) struct ConnectionBody {
    name: String,
    state: ConnectionState,
    detail: String,
}

impl From<ConnectionStatus> for ConnectionBody {
    fn from(status: ConnectionStatus) -> Self {
        Self {
            name: status.name,
            state: status.state,
            detail: status.detail,
        }
    }
}

pub(super) fn routes() -> Router<Engine> {
    Router::new()
        .route("/api/state", get(get_state))
        .route("/api/events", get(events))
        .route("/api/tasks/{id}/output", get(task_output))
        .route("/api/report", get(report))
        .route("/api/approvals", get(approvals))
        .route("/api/permissions", get(permissions).put(update_permissions))
        .route("/api/routing", put(set_routing))
        .route("/api/tools", get(tools))
        .route("/api/tools/scan", post(scan_tools))
        .route("/api/tools/{id}", put(toggle_tool))
        .route("/api/doctor", post(doctor))
        .route("/api/connections", get(connections))
}

#[derive(Debug, Deserialize)]
struct StateQuery {
    /// The chat whose stream events the snapshot carries.
    chat: Option<String>,
}

async fn get_state(
    State(engine): State<Engine>,
    Query(query): Query<StateQuery>,
) -> ApiResult<Snapshot> {
    blocking(engine, move |e| {
        let chat = query.chat.map(sentinel_core::ChatId::new);
        e.snapshot_for(chat.as_ref())
    })
    .await
}

async fn events(
    State(engine): State<Engine>,
    Query(query): Query<PageQuery>,
) -> ApiResult<Vec<EventRecord>> {
    let after = query.after.unwrap_or(0);
    let limit = u32::try_from(page_limit(query.limit)).unwrap_or(u32::MAX);
    blocking(engine, move |e| e.events_after(after, limit)).await
}

async fn task_output(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    Query(query): Query<PageQuery>,
) -> ApiResult<Vec<EventRecord>> {
    let after = query.after.unwrap_or(0);
    let limit = page_limit(query.limit);
    blocking(engine, move |e| {
        e.task_output(&TaskId::new(id), after, limit)
    })
    .await
}

async fn report(State(engine): State<Engine>) -> ApiResult<Value> {
    blocking(engine, |e| {
        e.report_markdown().map(|md| json!({ "markdown": md }))
    })
    .await
}

async fn approvals(State(engine): State<Engine>) -> ApiResult<Vec<Approval>> {
    blocking(engine, |e| e.snapshot().map(|s| s.approvals)).await
}

async fn permissions(State(engine): State<Engine>) -> ApiResult<Value> {
    blocking(engine, |e| {
        e.snapshot()
            .map(|s| json!({ "mode": s.permission_mode, "capabilities": s.capabilities }))
    })
    .await
}

async fn update_permissions(
    State(engine): State<Engine>,
    caller: Caller,
    Json(update): Json<PermissionsUpdate>,
) -> ApiResult<Value> {
    if update.capabilities.is_some() {
        require_computer(&caller)?;
    }
    blocking(engine, move |e| {
        e.update_permissions(&update, &caller).map(|()| okay())
    })
    .await
}

async fn set_routing(
    State(engine): State<Engine>,
    caller: Caller,
    Json(update): Json<RoutingUpdate>,
) -> ApiResult<Value> {
    require_computer(&caller)?;
    blocking(engine, move |e| {
        e.set_routing(update.routing).map(|()| okay())
    })
    .await
}

async fn tools(State(engine): State<Engine>) -> ApiResult<Vec<ToolInfo>> {
    blocking(engine, |e| {
        e.store().list_tools().map_err(EngineError::from)
    })
    .await
}

async fn scan_tools(State(engine): State<Engine>, caller: Caller) -> ApiResult<Vec<ToolInfo>> {
    require_computer(&caller)?;
    Ok(Json(engine.scan_tools().await?))
}

async fn toggle_tool(
    State(engine): State<Engine>,
    caller: Caller,
    Path(id): Path<String>,
    Json(toggle): Json<ToolToggle>,
) -> ApiResult<Value> {
    require_computer(&caller)?;
    blocking(engine, move |e| {
        e.toggle_tool(&ToolId::new(id), toggle.enabled)
            .map(|()| okay())
    })
    .await
}

async fn doctor(State(engine): State<Engine>, caller: Caller) -> ApiResult<DoctorReport> {
    require_computer(&caller)?;
    Ok(Json(engine.doctor().await))
}

async fn connections(State(engine): State<Engine>) -> ApiResult<Vec<ConnectionBody>> {
    let rows = engine
        .connections()
        .await
        .into_iter()
        .map(ConnectionBody::from)
        .collect();
    Ok(Json(rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_limit_defaults_and_clamps() {
        assert_eq!(page_limit(None), 500);
        assert_eq!(page_limit(Some(0)), 1);
        assert_eq!(page_limit(Some(99_999)), 5000);
        assert_eq!(page_limit(Some(40)), 40);
    }
}
