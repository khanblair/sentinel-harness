//! The hub: many projects behind one server. The first project keeps the paired phones, the keys,
//! and Telegram. A request that names another project (the `x-sentinel-project` header) goes to
//! that project's routes. Global paths always go to the first project.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use reqwest::header::AUTHORIZATION;
use reqwest::{Method, StatusCode};
use sentinel_daemon::engine::Engine;
use sentinel_daemon::hub::{Hub, OpenFuture, PROJECT_HEADER, ProjectOpener, hub_router};
use serde_json::{Value, json};
use tokio::net::TcpListener;

mod common;
use common::TailnetRunner;

/// Opens the other projects with the same scripted agents as the first one.
struct TestOpener;

impl ProjectOpener for TestOpener {
    fn open(&self, path: PathBuf) -> OpenFuture {
        Box::pin(async move { open_test_engine(&path).await })
    }
}

async fn open_test_engine(path: &Path) -> Result<Engine, String> {
    let engine = common::engine_with_runner(path, Arc::new(TailnetRunner));
    engine
        .scan_tools()
        .await
        .map_err(|error| error.to_string())?;
    Ok(engine)
}

/// A folder the hub accepts: it holds a `.git` folder.
fn make_repo(path: &Path) {
    fs::create_dir_all(path.join(".git")).expect("hub test setup");
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

/// A caller of the hub: who it is, and the project it names.
#[derive(Clone)]
struct Client {
    base: String,
    token: Option<String>,
    project: Option<String>,
}

impl Client {
    fn anonymous(base: &str) -> Self {
        Self {
            base: base.to_owned(),
            token: None,
            project: None,
        }
    }

    fn with_token(&self, token: &str) -> Self {
        Self {
            token: Some(token.to_owned()),
            ..self.clone()
        }
    }

    fn in_project(&self, id: &str) -> Self {
        Self {
            project: Some(id.to_owned()),
            ..self.clone()
        }
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        let mut request = reqwest::Client::new().request(method, format!("{}{path}", self.base));
        if let Some(token) = &self.token {
            request = request.header(AUTHORIZATION, bearer(token));
        }
        if let Some(project) = &self.project {
            request = request.header(PROJECT_HEADER, project.as_str());
        }
        request
    }

    async fn send(&self, method: Method, path: &str, body: Option<&Value>) -> (StatusCode, Value) {
        let mut request = self.request(method, path);
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().await.expect("hub test setup");
        let status = response.status();
        (status, response.json().await.unwrap_or(Value::Null))
    }

    async fn get(&self, path: &str) -> (StatusCode, Value) {
        self.send(Method::GET, path, None).await
    }

    async fn post(&self, path: &str, body: &Value) -> (StatusCode, Value) {
        self.send(Method::POST, path, Some(body)).await
    }

    async fn delete(&self, path: &str) -> (StatusCode, Value) {
        self.send(Method::DELETE, path, None).await
    }
}

/// A running hub: the address, the desktop token of the first project, and the temp folder.
struct Served {
    anonymous: Client,
    desktop_token: String,
    root: tempfile::TempDir,
}

impl Served {
    fn desktop(&self) -> Client {
        self.anonymous.with_token(&self.desktop_token)
    }
}

/// A hub on a free port, with the first project in a temp folder.
async fn serve_hub() -> Served {
    let root = tempfile::tempdir().expect("hub test setup");
    let first = root.path().join("first");
    make_repo(&first);
    let primary = common::engine_with_runner(&first, Arc::new(TailnetRunner));
    let desktop_token = primary.ensure_desktop_token().expect("hub test setup");
    let hub = Hub::new(primary, Some(Arc::new(TestOpener)));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("hub test setup");
    let addr = listener.local_addr().expect("hub test setup");
    tokio::spawn(async move {
        let _ = axum::serve(listener, hub_router(hub)).await;
    });
    Served {
        anonymous: Client::anonymous(&format!("http://{addr}")),
        desktop_token,
        root,
    }
}

/// Adds a path as a project with the desktop token, and returns the reply.
async fn add_path(served: &Served, folder: &Path) -> (StatusCode, Value) {
    let body = json!({ "path": folder.to_string_lossy() });
    served.desktop().post("/api/projects", &body).await
}

/// Adds a new git folder as a project, and returns its id.
async fn add_project(served: &Served, name: &str) -> String {
    let folder = served.root.path().join(name);
    make_repo(&folder);
    let (status, body) = add_path(served, &folder).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["id"].as_str().expect("hub test setup").to_owned()
}

/// Pairs a phone through the routes and returns its token, the way the app does.
async fn pair_phone(served: &Served) -> String {
    let desktop = served.desktop();
    let (started_status, started) = desktop.post("/api/pair/start", &json!({})).await;
    assert_eq!(started_status, StatusCode::OK, "{started}");
    let code = started["code"].as_str().expect("hub test setup").to_owned();
    let body = json!({ "code": code, "device_name": "Blair phone" });
    let (_, submitted) = served.anonymous.post("/api/pair", &body).await;
    let id = submitted["id"].as_str().expect("hub test setup").to_owned();
    let (approved, _) = desktop
        .post(&format!("/api/pair/{id}/approve"), &json!({}))
        .await;
    assert_eq!(approved, StatusCode::OK);
    let (_, outcome) = served
        .anonymous
        .get(&format!("/api/pair/{id}/outcome"))
        .await;
    assert_eq!(outcome["state"], "approved", "{outcome}");
    outcome["token"]
        .as_str()
        .expect("hub test setup")
        .to_owned()
}

fn has_chat(list: &Value, id: &str) -> bool {
    list.as_array()
        .expect("hub test setup")
        .iter()
        .any(|chat| chat["id"] == id)
}

#[tokio::test]
async fn adding_a_folder_lists_two_projects_with_the_first_primary() {
    let served = serve_hub().await;
    let second = add_project(&served, "second").await;
    let (status, list) = served.desktop().get("/api/projects").await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let projects = list.as_array().expect("hub test setup");
    assert_eq!(projects.len(), 2, "{list}");
    assert_eq!(projects[0]["primary"], true, "{list}");
    assert_eq!(projects[1]["id"], second, "{list}");
    assert_eq!(projects[1]["primary"], false, "{list}");
}

#[tokio::test]
async fn a_project_header_creates_the_chat_in_that_project() {
    let served = serve_hub().await;
    let second = add_project(&served, "second").await;
    let in_second = served.desktop().in_project(&second);
    let (status, chat) = in_second.post("/api/chats", &json!({})).await;
    assert_eq!(status, StatusCode::OK, "{chat}");
    let chat_id = chat["id"].as_str().expect("hub test setup").to_owned();
    let (_, listed_in_second) = in_second.get("/api/chats").await;
    assert!(has_chat(&listed_in_second, &chat_id), "{listed_in_second}");
    let (_, listed_in_first) = served.desktop().get("/api/chats").await;
    assert!(!has_chat(&listed_in_first, &chat_id), "{listed_in_first}");
}

#[tokio::test]
async fn a_wrong_token_on_a_named_project_is_401() {
    let served = serve_hub().await;
    let second = add_project(&served, "second").await;
    let wrong = served
        .anonymous
        .with_token("not-a-real-token")
        .in_project(&second);
    let (status, body) = wrong.get("/api/chats").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
}

#[tokio::test]
async fn an_unknown_project_id_is_404() {
    let served = serve_hub().await;
    let (status, body) = served
        .desktop()
        .in_project("0000000000000000")
        .get("/api/chats")
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test]
async fn a_paired_phone_cannot_add_or_remove_projects() {
    let served = serve_hub().await;
    let second = add_project(&served, "second").await;
    let phone = served.anonymous.with_token(&pair_phone(&served).await);
    let (added, _) = phone
        .post(
            "/api/projects",
            &json!({ "path": served.root.path().to_string_lossy() }),
        )
        .await;
    assert_eq!(added, StatusCode::FORBIDDEN);
    let (removed, _) = phone.delete(&format!("/api/projects/{second}")).await;
    assert_eq!(removed, StatusCode::FORBIDDEN);
    let (anonymous, _) = served
        .anonymous
        .post("/api/projects", &json!({ "path": "/nowhere" }))
        .await;
    assert_eq!(anonymous, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_plain_folder_is_400_and_the_same_git_folder_twice_is_409() {
    let served = serve_hub().await;
    let plain = served.root.path().join("plain");
    fs::create_dir_all(&plain).expect("hub test setup");
    let (plain_status, plain_body) = add_path(&served, &plain).await;
    assert_eq!(plain_status, StatusCode::BAD_REQUEST, "{plain_body}");
    let repo = served.root.path().join("repo");
    make_repo(&repo);
    let (first_status, first_body) = add_path(&served, &repo).await;
    assert_eq!(first_status, StatusCode::OK, "{first_body}");
    let (again_status, again_body) = add_path(&served, &repo).await;
    assert_eq!(again_status, StatusCode::CONFLICT, "{again_body}");
}

#[tokio::test]
async fn the_second_project_can_be_removed_but_the_first_cannot() {
    let served = serve_hub().await;
    let second = add_project(&served, "second").await;
    let (removed, removed_body) = served
        .desktop()
        .delete(&format!("/api/projects/{second}"))
        .await;
    assert_eq!(removed, StatusCode::OK, "{removed_body}");
    let (_, list) = served.desktop().get("/api/projects").await;
    let projects = list.as_array().expect("hub test setup");
    assert_eq!(projects.len(), 1, "{list}");
    let first = projects[0]["id"]
        .as_str()
        .expect("hub test setup")
        .to_owned();
    let (refused, refused_body) = served
        .desktop()
        .delete(&format!("/api/projects/{first}"))
        .await;
    assert_eq!(refused, StatusCode::BAD_REQUEST, "{refused_body}");
}

#[tokio::test]
async fn devices_stay_in_the_first_project_when_a_project_is_named() {
    let served = serve_hub().await;
    let second = add_project(&served, "second").await;
    pair_phone(&served).await;
    let (status, devices) = served
        .desktop()
        .in_project(&second)
        .get("/api/devices")
        .await;
    assert_eq!(status, StatusCode::OK, "{devices}");
    let names: Vec<&str> = devices
        .as_array()
        .expect("hub test setup")
        .iter()
        .filter_map(|device| device["name"].as_str())
        .collect();
    assert!(names.contains(&"Blair phone"), "{devices}");
}
