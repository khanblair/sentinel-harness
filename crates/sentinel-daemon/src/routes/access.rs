//! Pairing, devices, Tailscale, and the folder pickers.

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::routing::{delete, get, post};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use time::OffsetDateTime;

use sentinel_core::DeviceInfo;
use sentinel_core::api::{HostStatus, PairStart, PairSubmit, TailscaleStatus, WhoAmI};
use sentinel_core::ids::DeviceId;

use super::{ApiFailure, ApiResult, blocking, okay};
use crate::auth::Computer;
use crate::controls::Caller;
use crate::engine::Engine;
use crate::pairing::{PairOutcome, PendingId, PendingPair};

#[derive(Debug, Deserialize)]
pub struct FolderQuery {
    path: Option<String>,
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

/// The pairing routes a phone calls before it has a token.
pub(super) fn public() -> Router<Engine> {
    Router::new()
        .route("/api/pair", post(pair_submit))
        .route("/api/pair/{id}/outcome", get(pair_outcome))
}

pub(super) fn routes() -> Router<Engine> {
    Router::new()
        .route("/api/pair/start", post(pair_start))
        .route("/api/pair/pending", get(pair_pending))
        .route("/api/pair/{id}/approve", post(pair_approve))
        .route("/api/pair/{id}/deny", post(pair_deny))
        .route("/api/devices", get(devices))
        .route("/api/devices/{id}", delete(revoke_device))
        .route("/api/tailscale/status", get(tailscale_status))
        .route("/api/tailscale/signin", post(tailscale_sign_in))
        .route("/api/host", get(host_status))
        .route("/api/whoami", get(whoami))
        .route("/api/host/check", post(host_check))
        .route("/api/quit", post(quit))
        .route("/api/fs/pick", post(fs_pick))
        .route("/api/fs/list", get(fs_list))
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
    Computer(_): Computer,
    headers: HeaderMap,
) -> ApiResult<PairStart> {
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            ApiFailure::new(StatusCode::BAD_REQUEST, "The request has no Host header.")
        })?;
    let base_url = format!("http://{host}");
    Ok(Json(engine.pair_start(&base_url).await))
}

async fn pair_pending(
    State(engine): State<Engine>,
    Computer(_): Computer,
) -> ApiResult<Vec<PendingBody>> {
    let rows = engine
        .pair_pending()
        .iter()
        .map(PendingBody::from)
        .collect();
    Ok(Json(rows))
}

async fn pair_approve(
    State(engine): State<Engine>,
    Computer(_): Computer,
    Path(id): Path<String>,
) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.pair_approve(&PendingId::new(id)).map(|_| okay())
    })
    .await
}

async fn pair_deny(
    State(engine): State<Engine>,
    Computer(_): Computer,
    Path(id): Path<String>,
) -> ApiResult<Value> {
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

async fn host_status(State(engine): State<Engine>) -> ApiResult<HostStatus> {
    Ok(Json(engine.host_status().await?))
}

/// Who is calling, so the UI can tell a phone from the computer. Any valid device may ask.
async fn whoami(caller: Caller) -> Json<WhoAmI> {
    Json(WhoAmI {
        from_pc: caller.from_pc,
        name: caller.name,
    })
}

/// Write a heartbeat now and say so in the stream, so the host card can show it.
async fn host_check(State(engine): State<Engine>, Computer(_): Computer) -> ApiResult<HostStatus> {
    engine.beat()?;
    engine.emit_quiet(&sentinel_core::Event::HostHeartbeat);
    Ok(Json(engine.host_status().await?))
}

/// Stop the daemon. The computer only: a phone cannot switch the host off.
async fn quit(State(engine): State<Engine>, Computer(_): Computer) -> ApiResult<Value> {
    engine.request_quit();
    Ok(Json(okay()))
}

async fn tailscale_status(State(engine): State<Engine>) -> ApiResult<TailscaleStatus> {
    Ok(Json(engine.tailscale_status().await))
}

async fn tailscale_sign_in(
    State(engine): State<Engine>,
    Computer(_): Computer,
) -> ApiResult<Value> {
    let result = engine.tailscale_sign_in().await;
    Ok(Json(json!({ "ok": result.ok, "message": result.message })))
}

async fn fs_pick(State(engine): State<Engine>, Computer(_): Computer) -> ApiResult<Value> {
    let picked = tokio::task::spawn_blocking(move || engine.pick_folder_blocking())
        .await
        .map_err(|_| ApiFailure::internal())?;
    Ok(Json(
        json!({ "path": picked.map(|p| p.display().to_string()) }),
    ))
}

/// The folder browser for a phone. The engine keeps it inside the home folder.
async fn fs_list(
    State(engine): State<Engine>,
    Query(query): Query<FolderQuery>,
) -> ApiResult<Vec<sentinel_core::api::FolderEntry>> {
    blocking(engine, move |e| e.list_folders(query.path.as_deref())).await
}

#[cfg(test)]
mod tests {
    use super::*;

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
