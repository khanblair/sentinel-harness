//! The HTTP routes. Each handler checks the caller, calls one Engine method, and returns JSON.
//! Errors leave as `ApiError` bodies. Blocking store work runs off the async runtime.

mod access;
mod chats;
mod run;
mod setup;
mod state;

use axum::Json;
use axum::Router;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::Serialize;
use serde_json::{Value, json};

use sentinel_core::api::ApiError;

use crate::controls::Caller;
use crate::engine::{Engine, EngineError};
use crate::pairing::PairError;

pub use state::page_limit;

pub type ApiResult<T> = Result<Json<T>, ApiFailure>;

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

/// The routes anyone may call. They carry no device token.
pub fn public_routes() -> Router<Engine> {
    Router::new()
        .route("/api/health", get(health))
        .merge(access::public())
}

/// The routes that carry file bytes and need a device token. They get their own body limit.
pub fn upload_routes() -> Router<Engine> {
    chats::upload_routes()
}

/// The routes that need a device token.
pub fn protected_routes() -> Router<Engine> {
    Router::new()
        .merge(chats::routes())
        .merge(state::routes())
        .merge(run::routes())
        .merge(access::routes())
        .merge(setup::routes())
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
