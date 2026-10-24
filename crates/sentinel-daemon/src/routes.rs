//! The HTTP routes. Each handler checks the caller, calls one Engine method, and returns JSON.
//! Errors leave as `ApiError` bodies. Blocking store work runs off the async runtime.

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use time::OffsetDateTime;

use sentinel_core::api::{
    AnswerApproval, AnswerDecision, ApiError, DoctorReport, LowerCap, PairStart, PairSubmit,
    PermissionsUpdate, RepoCheckRequest, RepoCheckResponse, RoutingUpdate, RunRequest,
    SetBrainMode, TailscaleStatus, ToolToggle,
};
use sentinel_core::ids::{ApprovalId, DecisionId, DeviceId, TaskId, ToolId};
use sentinel_core::permission::Approval;
use sentinel_core::snapshot::{RunInfo, Snapshot};
use sentinel_core::state::{ConnectionState, Decision};
use sentinel_core::tool::ToolInfo;
use sentinel_core::{BrainMode, DeviceInfo, EventRecord, OnboardingState};

use crate::connections::ConnectionStatus;
use crate::controls::{Caller, usd_text};
use crate::engine::{Engine, EngineError};
use crate::pairing::{PairError, PairOutcome, PendingId, PendingPair};

pub type ApiResult<T> = Result<Json<T>, ApiFailure>;

const DEFAULT_PAGE: usize = 500;
const MAX_PAGE: usize = 5000;

/// A failed request: a status code and the sentence the client shows.
#[derive(Debug)]
pub struct ApiFailure {
    status: StatusCode,
    message: String,
}

impl ApiFailure {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "Send a valid device token.")
    }

    fn needs_computer() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "This needs the computer. Do it there.",
        )
    }

    fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "The computer could not finish that.",
        )
    }
}

impl IntoResponse for ApiFailure {
    fn into_response(self) -> Response {
        let body = ApiError {
            error: self.message,
        };
        (self.status, Json(body)).into_response()
    }
}

impl From<EngineError> for ApiFailure {
    fn from(error: EngineError) -> Self {
        let status = match error {
            EngineError::NotFound(_) => StatusCode::NOT_FOUND,
            EngineError::Invalid(_) => StatusCode::BAD_REQUEST,
            EngineError::Refused(_) => StatusCode::FORBIDDEN,
            EngineError::Store(_)
            | EngineError::Config(_)
            | EngineError::Onboarding(_)
            | EngineError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        if status.is_server_error() {
            return Self::internal();
        }
        Self::new(status, error.to_string())
    }
}

impl From<PairError> for ApiFailure {
    fn from(error: PairError) -> Self {
        let status = match error {
            PairError::BadCode | PairError::Expired | PairError::AlreadyUsed => {
                StatusCode::BAD_REQUEST
            }
            PairError::UnknownPending => StatusCode::NOT_FOUND,
            PairError::NoTailscale => StatusCode::SERVICE_UNAVAILABLE,
            PairError::Store(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        if status.is_server_error() {
            return Self::internal();
        }
        Self::new(status, error.to_string())
    }
}

/// Runs one blocking Engine call on the blocking pool and returns its value as JSON.
async fn blocking<T, E, F>(engine: Engine, job: F) -> ApiResult<T>
where
    T: Serialize + Send + 'static,
    E: Into<ApiFailure> + Send + 'static,
    F: FnOnce(&Engine) -> Result<T, E> + Send + 'static,
{
    let outcome = tokio::task::spawn_blocking(move || job(&engine))
        .await
        .map_err(|_| ApiFailure::internal())?;
    outcome.map(Json).map_err(Into::into)
}

fn okay() -> Value {
    json!({ "ok": true })
}

fn require_computer(caller: &Caller) -> Result<(), ApiFailure> {
    if caller.from_pc {
        Ok(())
    } else {
        Err(ApiFailure::needs_computer())
    }
}

/// The page size a client asked for, kept between 1 and 5000.
pub fn page_limit(requested: Option<usize>) -> usize {
    requested.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE)
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    after: Option<i64>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct FolderQuery {
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MessageBody {
    text: String,
    to: Option<ToolId>,
}

/// The wire form of a pairing outcome, as the phone reads it.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PairOutcomeBody {
    Waiting,
    Approved { device: DeviceId, token: String },
    Denied,
    Expired,
    Unknown,
}

impl From<PairOutcome> for PairOutcomeBody {
    fn from(outcome: PairOutcome) -> Self {
        match outcome {
            PairOutcome::Waiting => Self::Waiting,
            PairOutcome::Approved(result) => Self::Approved {
                device: result.device,
                token: result.token,
            },
            PairOutcome::Denied => Self::Denied,
            PairOutcome::Expired => Self::Expired,
            PairOutcome::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PendingBody {
    id: String,
    device_name: String,
    #[serde(with = "time::serde::rfc3339")]
    requested_at: OffsetDateTime,
}

impl From<&PendingPair> for PendingBody {
    fn from(pending: &PendingPair) -> Self {
        Self {
            id: pending.id.as_str().to_owned(),
            device_name: pending.device_name.clone(),
            requested_at: pending.requested_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ConnectionBody {
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

/// The routes anyone may call. They carry no device token.
pub fn public_routes() -> Router<Engine> {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/pair", post(pair_submit))
        .route("/api/pair/{id}/outcome", get(pair_outcome))
}

/// The routes that need a device token.
pub fn protected_routes() -> Router<Engine> {
    Router::new()
        .merge(state_routes())
        .merge(run_routes())
        .merge(access_routes())
        .merge(setup_routes())
}

fn state_routes() -> Router<Engine> {
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

fn run_routes() -> Router<Engine> {
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

fn access_routes() -> Router<Engine> {
    Router::new()
        .route("/api/pair/start", post(pair_start))
        .route("/api/pair/pending", get(pair_pending))
        .route("/api/pair/{id}/approve", post(pair_approve))
        .route("/api/pair/{id}/deny", post(pair_deny))
        .route("/api/devices", get(devices))
        .route("/api/devices/{id}", delete(revoke_device))
        .route("/api/tailscale/status", get(tailscale_status))
        .route("/api/tailscale/signin", post(tailscale_sign_in))
        .route("/api/fs/pick", post(fs_pick))
        .route("/api/fs/list", get(fs_list))
}

fn setup_routes() -> Router<Engine> {
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

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn get_state(State(engine): State<Engine>) -> ApiResult<Snapshot> {
    blocking(engine, Engine::snapshot).await
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
    blocking(engine, move |e| {
        e.update_permissions(&update, &caller).map(|()| okay())
    })
    .await
}

async fn set_routing(
    State(engine): State<Engine>,
    Json(update): Json<RoutingUpdate>,
) -> ApiResult<Value> {
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

async fn scan_tools(State(engine): State<Engine>) -> ApiResult<Vec<ToolInfo>> {
    Ok(Json(engine.scan_tools().await?))
}

async fn toggle_tool(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    Json(toggle): Json<ToolToggle>,
) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.toggle_tool(&ToolId::new(id), toggle.enabled)
            .map(|()| okay())
    })
    .await
}

async fn doctor(State(engine): State<Engine>) -> ApiResult<DoctorReport> {
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

async fn start_run(
    State(engine): State<Engine>,
    Json(request): Json<RunRequest>,
) -> ApiResult<RunInfo> {
    Ok(Json(engine.start_run(request).await?))
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

async fn pair_submit(
    State(engine): State<Engine>,
    Json(submit): Json<PairSubmit>,
) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.pair_submit(&submit)
            .map(|id| json!({ "id": id.as_str() }))
    })
    .await
}

async fn pair_outcome(
    State(engine): State<Engine>,
    Path(id): Path<String>,
) -> Json<PairOutcomeBody> {
    Json(engine.pair_outcome(&PendingId::new(id)).into())
}

async fn pair_start(
    State(engine): State<Engine>,
    caller: Caller,
    headers: HeaderMap,
) -> ApiResult<PairStart> {
    require_computer(&caller)?;
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            ApiFailure::new(StatusCode::BAD_REQUEST, "The request has no Host header.")
        })?;
    let base_url = format!("http://{host}");
    Ok(Json(engine.pair_start(&base_url).await))
}

async fn pair_pending(State(engine): State<Engine>, caller: Caller) -> ApiResult<Vec<PendingBody>> {
    require_computer(&caller)?;
    let rows = engine
        .pair_pending()
        .iter()
        .map(PendingBody::from)
        .collect();
    Ok(Json(rows))
}

async fn pair_approve(
    State(engine): State<Engine>,
    caller: Caller,
    Path(id): Path<String>,
) -> ApiResult<Value> {
    require_computer(&caller)?;
    blocking(engine, move |e| {
        e.pair_approve(&PendingId::new(id)).map(|_| okay())
    })
    .await
}

async fn pair_deny(
    State(engine): State<Engine>,
    caller: Caller,
    Path(id): Path<String>,
) -> ApiResult<Value> {
    require_computer(&caller)?;
    blocking(engine, move |e| {
        e.pair_deny(&PendingId::new(id)).map(|()| okay())
    })
    .await
}

async fn devices(State(engine): State<Engine>) -> ApiResult<Vec<DeviceInfo>> {
    blocking(engine, Engine::devices).await
}

async fn revoke_device(
    State(engine): State<Engine>,
    caller: Caller,
    Path(id): Path<String>,
) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.revoke_device(&DeviceId::new(id), &caller.name)
            .map(|()| okay())
    })
    .await
}

async fn tailscale_status(State(engine): State<Engine>) -> ApiResult<TailscaleStatus> {
    Ok(Json(engine.tailscale_status().await))
}

async fn tailscale_sign_in(State(engine): State<Engine>) -> ApiResult<Value> {
    let result = engine.tailscale_sign_in().await;
    Ok(Json(json!({ "ok": result.ok, "message": result.message })))
}

async fn fs_pick(State(engine): State<Engine>, caller: Caller) -> ApiResult<Value> {
    require_computer(&caller)?;
    let picked = tokio::task::spawn_blocking(move || engine.pick_folder_blocking())
        .await
        .map_err(|_| ApiFailure::internal())?;
    Ok(Json(
        json!({ "path": picked.map(|p| p.display().to_string()) }),
    ))
}

async fn fs_list(
    State(engine): State<Engine>,
    Query(query): Query<FolderQuery>,
) -> ApiResult<Vec<sentinel_core::api::FolderEntry>> {
    blocking(engine, move |e| e.list_folders(query.path.as_deref())).await
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
    Path(step): Path<u8>,
) -> ApiResult<OnboardingState> {
    blocking(engine, move |e| e.onboarding_step(step)).await
}

async fn onboarding_skip(
    State(engine): State<Engine>,
    Path(step): Path<u8>,
) -> ApiResult<OnboardingState> {
    blocking(engine, move |e| e.onboarding_skip(step)).await
}

async fn onboarding_complete(State(engine): State<Engine>) -> ApiResult<OnboardingState> {
    blocking(engine, Engine::onboarding_complete).await
}

async fn onboarding_reset(State(engine): State<Engine>) -> ApiResult<OnboardingState> {
    blocking(engine, Engine::onboarding_reset).await
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

    #[test]
    fn an_invalid_request_maps_to_400_with_its_message() {
        let failure = ApiFailure::from(EngineError::Invalid("bad".to_owned()));
        assert_eq!(failure.status, StatusCode::BAD_REQUEST);
        assert_eq!(failure.message, "bad");
    }

    #[test]
    fn refused_maps_to_403_and_not_found_to_404() {
        let refused = ApiFailure::from(EngineError::Refused("no".to_owned()));
        assert_eq!(refused.status, StatusCode::FORBIDDEN);
        let missing = ApiFailure::from(EngineError::NotFound("decision".to_owned()));
        assert_eq!(missing.status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn an_approved_pairing_outcome_carries_the_token() {
        let body = PairOutcomeBody::from(PairOutcome::Approved(sentinel_core::api::PairResult {
            device: DeviceId::new("dev-1"),
            token: "tok".to_owned(),
        }));
        let text = serde_json::to_string(&body).unwrap_or_default();
        assert_eq!(
            text,
            r#"{"state":"approved","device":"dev-1","token":"tok"}"#
        );
    }

    #[test]
    fn a_waiting_pairing_outcome_is_only_its_state() {
        let body = PairOutcomeBody::from(PairOutcome::Waiting);
        assert_eq!(
            serde_json::to_string(&body).unwrap_or_default(),
            r#"{"state":"waiting"}"#
        );
    }
}
