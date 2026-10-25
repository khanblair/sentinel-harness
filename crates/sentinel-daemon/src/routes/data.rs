//! Clear data by scope. The project route clears the project the request names. The app and
//! "all" routes live on the hub, which calls the handlers here, since they reach every project.

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::routing::post;
use sentinel_core::api::{ClearData, DataScope};
use serde_json::Value;

use super::{ApiFailure, ApiResult, blocking, okay};
use crate::auth::Computer;
use crate::data_clear::ensure_no_live_run;
use crate::engine::{Engine, EngineError};
use crate::hub::Hub;

pub fn routes() -> Router<Engine> {
    Router::new().route("/api/data/project", post(clear_project))
}

/// Runs one clear on the blocking pool and keeps only whether it worked.
async fn clear_on<F>(engine: Engine, job: F) -> Result<(), ApiFailure>
where
    F: FnOnce(&Engine) -> Result<(), EngineError> + Send + 'static,
{
    blocking(engine, move |e| job(e).map(|()| okay()))
        .await
        .map(|_| ())
}

/// `POST /api/data/project`. Settings also scans the agent CLIs again, so the ready ones come back.
async fn clear_project(
    State(engine): State<Engine>,
    Computer(_): Computer,
    Json(body): Json<ClearData>,
) -> ApiResult<Value> {
    let settings = body.scopes.contains(&DataScope::Settings);
    let scopes = body.scopes;
    clear_on(engine.clone(), move |e| e.clear_project_data(&scopes)).await?;
    if settings {
        engine.scan_tools().await?;
    }
    Ok(Json(okay()))
}

/// `POST /api/data/app`, on the hub. The project list is forgotten when it is in the request.
pub(crate) async fn clear_app(
    State(hub): State<Hub>,
    Computer(_): Computer,
    Json(body): Json<ClearData>,
) -> ApiResult<Value> {
    let forget = body.scopes.contains(&DataScope::Projects);
    let scopes: Vec<DataScope> = body
        .scopes
        .into_iter()
        .filter(|scope| *scope != DataScope::Projects)
        .collect();
    clear_on(hub.primary().clone(), move |e| e.clear_app_data(&scopes)).await?;
    if forget {
        hub.forget_others();
    }
    Ok(Json(okay()))
}

/// `POST /api/data/all`, on the hub. Every project first, then the app, then the project list. A
/// live run in any project refuses the whole reset before anything is removed.
pub(crate) async fn clear_all(State(hub): State<Hub>, Computer(_): Computer) -> ApiResult<Value> {
    let engines = hub.engines();
    ensure_no_live_run(&engines)?;
    for engine in engines {
        clear_project_and_agents(engine).await?;
    }
    let app: Vec<DataScope> = DataScope::APP
        .into_iter()
        .filter(|scope| *scope != DataScope::Projects)
        .collect();
    clear_on(hub.primary().clone(), move |e| e.clear_app_data(&app)).await?;
    hub.forget_others();
    Ok(Json(okay()))
}

/// One project's chats, brain, history, and settings, then its agent CLIs again.
async fn clear_project_and_agents(engine: Engine) -> Result<(), ApiFailure> {
    clear_on(engine.clone(), |e| {
        e.clear_project_data(&DataScope::PROJECT)
    })
    .await?;
    engine.scan_tools().await?;
    Ok(())
}
