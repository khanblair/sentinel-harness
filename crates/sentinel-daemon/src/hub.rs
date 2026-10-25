//! Many projects in one app. Each project is a folder with its own engine and its own `.sentinel`
//! state. The first project is the primary one: it serves the page, knows the paired phones, and
//! runs Telegram and the routines. A request that names another project (the `X-Sentinel-Project`
//! header, or `?project=` for the WebSocket) goes to the router of that project. Every other
//! request goes to the primary one. The routers are the same ones a single project always had.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use axum::extract::{Path as UrlPath, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router, middleware};
use sentinel_core::api::{AddProject, ProjectInfo};
use serde_json::{Value, json};
use tower::ServiceExt;
use tower_http::limit::RequestBodyLimitLayer;

use crate::auth::{self, Computer};
use crate::engine::{Engine, lock};
use crate::routes::clearing;
use crate::routes::{ApiFailure, ApiResult};
use crate::server::{self, BODY_LIMIT_BYTES};

/// The header that names the project of a request.
pub const PROJECT_HEADER: &str = "x-sentinel-project";

/// Paths that always belong to the primary project, whatever project a request names: the paired
/// phones, the tailnet, the keys, Telegram, the routines, the setup wizard, and the folder dialog.
const PRIMARY_PATHS: [&str; 15] = [
    "/api/health",
    "/api/pair",
    "/api/devices",
    "/api/tailscale",
    "/api/host",
    "/api/whoami",
    "/api/quit",
    "/api/fs/",
    "/api/connections",
    "/api/secrets",
    "/api/doctor",
    "/api/telegram",
    "/api/routines",
    "/api/briefs",
    "/api/onboarding",
];

pub type OpenFuture = Pin<Box<dyn Future<Output = Result<Engine, String>> + Send>>;

/// Opens the engine of one more project. The command line supplies it, because it knows how an
/// engine is made: the runner, the keychain, and the first scan of the agent CLIs.
pub trait ProjectOpener: Send + Sync {
    fn open(&self, path: PathBuf) -> OpenFuture;
}

#[derive(Debug, thiserror::Error)]
pub enum HubError {
    #[error("That is not a folder.")]
    NotAFolder,
    #[error("That folder is not a git repository. Run git init there first.")]
    NotGit,
    #[error("That project is already in the list.")]
    AlreadyThere,
    #[error("This app cannot add projects.")]
    CannotAdd,
    #[error("Could not open the project: {0}")]
    Open(String),
    #[error("No such project.")]
    NotFound,
    #[error("The first project stays. It also holds the paired phones and the keys.")]
    IsPrimary,
    #[error("A run is live in that project. Stop it first.")]
    Live,
}

impl From<HubError> for ApiFailure {
    fn from(error: HubError) -> Self {
        let status = match error {
            HubError::NotAFolder | HubError::NotGit | HubError::Open(_) | HubError::IsPrimary => {
                StatusCode::BAD_REQUEST
            }
            HubError::AlreadyThere | HubError::Live => StatusCode::CONFLICT,
            HubError::CannotAdd => StatusCode::NOT_IMPLEMENTED,
            HubError::NotFound => StatusCode::NOT_FOUND,
        };
        ApiFailure::new(status, error.to_string())
    }
}

#[derive(Clone)]
struct Entry {
    id: String,
    path: PathBuf,
    engine: Engine,
    router: Router,
}

impl Entry {
    fn new(path: PathBuf, engine: Engine) -> Self {
        Self {
            id: project_id(&path),
            router: server::router(engine.clone()),
            path,
            engine,
        }
    }
}

struct HubInner {
    primary: Entry,
    others: Mutex<Vec<Entry>>,
    opener: Option<Arc<dyn ProjectOpener>>,
}

#[derive(Clone)]
pub struct Hub {
    inner: Arc<HubInner>,
}

/// A stable id for a folder: FNV-1a over its path.
pub fn project_id(path: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.to_string_lossy().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn project_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || "project".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    )
}

impl Hub {
    /// A hub with the primary project. Without an opener it cannot add more.
    pub fn new(primary: Engine, opener: Option<Arc<dyn ProjectOpener>>) -> Hub {
        let path = primary.repo_path();
        Hub {
            inner: Arc::new(HubInner {
                primary: Entry::new(path, primary),
                others: Mutex::new(Vec::new()),
                opener,
            }),
        }
    }

    pub fn primary(&self) -> &Engine {
        &self.inner.primary.engine
    }

    fn entries(&self) -> Vec<Entry> {
        let mut all = vec![self.inner.primary.clone()];
        all.extend(lock(&self.inner.others).iter().cloned());
        all
    }

    /// Every engine, the primary first.
    pub fn engines(&self) -> Vec<Engine> {
        self.entries().into_iter().map(|e| e.engine).collect()
    }

    fn list_file(&self) -> PathBuf {
        self.primary().home().join(".sentinel/projects.json")
    }

    fn saved_paths(&self) -> Vec<PathBuf> {
        let text = std::fs::read_to_string(self.list_file()).unwrap_or_default();
        let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        value["projects"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| item.as_str().map(PathBuf::from))
            .collect()
    }

    fn save(&self) {
        let paths: Vec<String> = lock(&self.inner.others)
            .iter()
            .map(|e| e.path.to_string_lossy().into_owned())
            .collect();
        let file = self.list_file();
        let text = json!({ "projects": paths }).to_string();
        let written = file
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&file, text));
        if let Err(error) = written {
            eprintln!("sentinel: could not save the project list: {error}");
        }
    }

    /// Open the projects kept from the last run. A folder that is gone or does not open is left
    /// out of the list and the app goes on.
    pub async fn load_saved(&self) {
        for path in self.saved_paths() {
            if let Err(error) = self.add_path(&path).await {
                eprintln!("sentinel: skipped project {}: {error}", path.display());
            }
        }
        self.save();
    }

    pub async fn add_path(&self, path: &Path) -> Result<ProjectInfo, HubError> {
        let path = std::fs::canonicalize(path).map_err(|_| HubError::NotAFolder)?;
        if !path.is_dir() {
            return Err(HubError::NotAFolder);
        }
        if !path.join(".git").exists() {
            return Err(HubError::NotGit);
        }
        let id = project_id(&path);
        if self.entries().iter().any(|e| e.id == id) {
            return Err(HubError::AlreadyThere);
        }
        let opener = self.inner.opener.clone().ok_or(HubError::CannotAdd)?;
        let engine = opener.open(path.clone()).await.map_err(HubError::Open)?;
        engine.use_devices_of(self.primary());
        // The setup wizard belongs to the app, not to one project.
        let _ = engine.onboarding_complete();
        let entry = Entry::new(path, engine);
        lock(&self.inner.others).push(entry.clone());
        self.save();
        info_of(&entry, false).map_err(|e| HubError::Open(e.to_string()))
    }

    pub fn remove(&self, id: &str) -> Result<(), HubError> {
        if id == self.inner.primary.id {
            return Err(HubError::IsPrimary);
        }
        let mut others = lock(&self.inner.others);
        let at = others
            .iter()
            .position(|e| e.id == id)
            .ok_or(HubError::NotFound)?;
        if others[at].engine.is_run_live() {
            return Err(HubError::Live);
        }
        others.remove(at);
        drop(others);
        self.save();
        Ok(())
    }

    /// Forget every project but the primary one. The folders stay where they are.
    pub fn forget_others(&self) {
        lock(&self.inner.others).clear();
        self.save();
    }

    pub fn list(&self) -> Vec<ProjectInfo> {
        let primary = self.inner.primary.id.clone();
        self.entries()
            .iter()
            .filter_map(|entry| info_of(entry, entry.id == primary).ok())
            .collect()
    }

    fn router_for(&self, id: Option<&str>) -> Result<Router, HubError> {
        let Some(id) = id.filter(|id| *id != self.inner.primary.id) else {
            return Ok(self.inner.primary.router.clone());
        };
        lock(&self.inner.others)
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.router.clone())
            .ok_or(HubError::NotFound)
    }
}

fn info_of(entry: &Entry, primary: bool) -> Result<ProjectInfo, crate::engine::EngineError> {
    Ok(ProjectInfo {
        id: entry.id.clone(),
        name: project_name(&entry.path),
        path: entry.path.to_string_lossy().into_owned(),
        primary,
        live: entry.engine.is_run_live(),
        chats: entry.engine.list_chats()?,
    })
}

#[derive(Clone)]
struct Dispatch {
    hub: Hub,
    api: Router,
}

/// The whole app with projects: the routes of the list, then the router of the project a
/// request names.
pub fn hub_router(hub: Hub) -> Router {
    let api = routes()
        .route_layer(middleware::from_fn_with_state(
            hub.primary().clone(),
            auth::require_device,
        ))
        .layer(RequestBodyLimitLayer::new(BODY_LIMIT_BYTES))
        .layer(middleware::from_fn(auth::check_origin))
        .with_state(hub.clone());
    Router::new()
        .fallback(dispatch)
        .with_state(Dispatch { hub, api })
}

async fn dispatch(State(state): State<Dispatch>, request: Request) -> Response {
    let path = request.uri().path().to_owned();
    let router = if is_hub_path(&path) {
        Ok(state.api.clone())
    } else if is_primary_path(&path) {
        state.hub.router_for(None)
    } else {
        state.hub.router_for(project_of(&request).as_deref())
    };
    match router {
        Ok(router) => router.oneshot(request).await.into_response(),
        Err(error) => ApiFailure::from(error).into_response(),
    }
}

fn is_hub_path(path: &str) -> bool {
    path == "/api/projects"
        || path.starts_with("/api/projects/")
        || path == "/api/data/app"
        || path == "/api/data/all"
}

fn is_primary_path(path: &str) -> bool {
    PRIMARY_PATHS.iter().any(|prefix| path.starts_with(prefix))
}

/// The project a request names: the header, or `project=` in the query (a WebSocket cannot send
/// a header).
fn project_of(request: &Request) -> Option<String> {
    let header = request
        .headers()
        .get(PROJECT_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    header.or_else(|| {
        request
            .uri()
            .query()?
            .split('&')
            .find_map(|pair| pair.strip_prefix("project=").map(str::to_owned))
    })
}

fn routes() -> Router<Hub> {
    Router::new()
        .route("/api/projects", get(list_projects).post(add_project))
        .route("/api/projects/{id}", delete(remove_project))
        .route("/api/data/app", post(clearing::clear_app))
        .route("/api/data/all", post(clearing::clear_all))
}

async fn list_projects(State(hub): State<Hub>) -> ApiResult<Vec<ProjectInfo>> {
    let listed = tokio::task::spawn_blocking(move || hub.list())
        .await
        .map_err(|_| ApiFailure::new(StatusCode::INTERNAL_SERVER_ERROR, "Something broke."))?;
    Ok(Json(listed))
}

async fn add_project(
    State(hub): State<Hub>,
    Computer(_): Computer,
    Json(body): Json<AddProject>,
) -> ApiResult<ProjectInfo> {
    Ok(Json(hub.add_path(Path::new(&body.path)).await?))
}

async fn remove_project(
    State(hub): State<Hub>,
    Computer(_): Computer,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Value> {
    hub.remove(&id)?;
    Ok(Json(json!({ "ok": true })))
}
