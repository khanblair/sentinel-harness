//! Starts the daemon for one test and sends it requests. A test gets its own engine, so a row
//! that changes state cannot break the rows after it.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, ORIGIN};
use reqwest::{Client, Method, StatusCode};
use sentinel_daemon::engine::Engine;
use sentinel_daemon::server;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::{TcpListener, TcpStream};

use crate::common::{self, TailnetRunner};
use crate::table::Route;

/// Which token a request carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who {
    Nobody,
    Phone,
    Desktop,
}

pub struct Reply {
    pub status: StatusCode,
    pub text: String,
}

/// One daemon on a free port, with a desktop token and a paired phone.
pub struct Target {
    pub base: String,
    pub home: PathBuf,
    pub desktop: String,
    pub phone: String,
    _dir: tempfile::TempDir,
}

impl Target {
    pub async fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine_in(dir.path());
        let desktop = engine.ensure_desktop_token().unwrap();
        let base = serve(engine).await;
        let phone = pair_phone(&base, &desktop).await;
        Self {
            base,
            home: dir.path().to_path_buf(),
            desktop,
            phone,
            _dir: dir,
        }
    }

    fn token(&self, who: Who) -> Option<&str> {
        match who {
            Who::Nobody => None,
            Who::Phone => Some(&self.phone),
            Who::Desktop => Some(&self.desktop),
        }
    }

    fn fill(&self, body: &str) -> String {
        body.replace("{repo}", &self.home.display().to_string())
    }
}

/// An engine with fake agents on an empty repository. Native dialogs are off, so `fs/pick`
/// returns at once. Telegram calls go to a closed port, so no test reaches the internet.
fn engine_in(repo: &Path) -> Engine {
    let engine = common::engine_with_runner(repo, Arc::new(TailnetRunner));
    engine.set_headless(true);
    engine
        .update_config(|c| {
            "5.00".clone_into(&mut c.cap_usd);
            c.task_timeout_seconds = 5;
            c.max_parallel = 3;
            "http://127.0.0.1:9".clone_into(&mut c.telegram.base_url);
        })
        .unwrap();
    engine
}

async fn serve(engine: Engine) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, server::router(engine)).await;
    });
    format!("http://{addr}")
}

/// Pairs a phone through the real routes and returns its token.
async fn pair_phone(base: &str, desktop: &str) -> String {
    let (_, started) = call(
        format!("{base}/api/pair/start"),
        Method::POST,
        Some(desktop),
        json!({}),
    )
    .await;
    let code = started["code"].as_str().unwrap().to_owned();
    let (_, submitted) = call(
        format!("{base}/api/pair"),
        Method::POST,
        None,
        json!({ "code": code, "device_name": "Test phone" }),
    )
    .await;
    let id = submitted["id"].as_str().unwrap().to_owned();
    call(
        format!("{base}/api/pair/{id}/approve"),
        Method::POST,
        Some(desktop),
        json!({}),
    )
    .await;
    let (_, outcome) = call(
        format!("{base}/api/pair/{id}/outcome"),
        Method::GET,
        None,
        Value::Null,
    )
    .await;
    assert_eq!(outcome["state"], "approved", "{outcome}");
    outcome["token"].as_str().unwrap().to_owned()
}

async fn call(
    url: String,
    method: Method,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut request = Client::new().request(method, url);
    if let Some(token) = token {
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    if !body.is_null() {
        request = request.json(&body);
    }
    let response = request.send().await.unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

/// Sends one route with the given token and an optional `Origin` header.
pub async fn send(target: &Target, route: &Route, who: Who, origin: Option<&str>) -> Reply {
    let url = format!("{}{}", target.base, route.path);
    let mut request = Client::new().request(route.method.clone(), url);
    if let Some(token) = target.token(who) {
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(origin) = origin {
        request = request.header(ORIGIN, origin);
    }
    if let Some(body) = route.body {
        request = request
            .header(CONTENT_TYPE, "application/json")
            .body(target.fill(body));
    }
    let response = request.send().await.unwrap();
    let status = response.status();
    Reply {
        status,
        text: response.text().await.unwrap(),
    }
}

/// Opens `/ws`, sends an auth frame with no token, and returns everything the server wrote back
/// after the handshake.
pub async fn socket_without_token(target: &Target) -> String {
    let address = target.base.trim_start_matches("http://");
    let stream = TcpStream::connect(address).await.unwrap();
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    let handshake = format!(
        "GET /ws HTTP/1.1\r\nHost: {address}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    write.write_all(handshake.as_bytes()).await.unwrap();
    let status = read_headers(&mut reader).await;
    assert!(status.starts_with("HTTP/1.1 101"), "{status}");
    write
        .write_all(&masked_text(br#"{"op":"auth","token":""}"#))
        .await
        .unwrap();
    let mut rest = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), reader.read_to_end(&mut rest))
        .await
        .unwrap()
        .unwrap();
    String::from_utf8_lossy(&rest).into_owned()
}

/// Reads the status line and the header block. Returns the status line.
async fn read_headers(reader: &mut BufReader<OwnedReadHalf>) -> String {
    let mut status = String::new();
    reader.read_line(&mut status).await.unwrap();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await.unwrap() == 0 || line == "\r\n" {
            break;
        }
    }
    status
}

/// A client frame: a final text frame, masked with a zero key, as the spec requires from clients.
fn masked_text(payload: &[u8]) -> Vec<u8> {
    let length = u8::try_from(payload.len()).unwrap();
    let mut frame = vec![0x81, 0x80 | length, 0, 0, 0, 0];
    frame.extend_from_slice(payload);
    frame
}
