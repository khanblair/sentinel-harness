//! Typed HTTP client for the daemon API (`docs/architecture.md`, API). Request and response
//! bodies come from `sentinel_core::api`, so the UI and the daemon share one definition.

use std::fmt;

use gloo_net::http::{Request, RequestBuilder, Response};
use gloo_storage::{LocalStorage, Storage};
use sentinel_core::api::{AddAttachment, ApiError, CreateChat, RunRequest, UpdateChat};
use sentinel_core::{Attachment, AttachmentId, ChatId, ChatInfo, Snapshot, TaskId};
use serde::Serialize;
use serde::de::DeserializeOwned;

const TOKEN_KEY: &str = "sentinel.token";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiFailure {
    /// The daemon could not be reached.
    Network(String),
    /// The daemon answered with an error status.
    Status { code: u16, message: String },
    /// The answer was not the expected shape.
    Decode(String),
}

impl fmt::Display for ApiFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiFailure::Network(text) => write!(f, "Could not reach the daemon. {text}"),
            ApiFailure::Status { code, message } => write!(f, "{message} (status {code})"),
            ApiFailure::Decode(text) => {
                write!(f, "The daemon sent an answer this page cannot read. {text}")
            }
        }
    }
}

impl ApiFailure {
    /// A 401 means this device has no valid token.
    pub fn is_unauthorized(&self) -> bool {
        matches!(self, ApiFailure::Status { code: 401, .. })
    }
}

/// The device token, kept in this browser only.
pub fn token() -> Option<String> {
    LocalStorage::get::<String>(TOKEN_KEY)
        .ok()
        .filter(|t| !t.is_empty())
}

pub fn set_token(token: &str) {
    // Storage can be blocked (private window). The page then works until it is reloaded.
    let _ = LocalStorage::set(TOKEN_KEY, token);
}

pub fn clear_token() {
    LocalStorage::delete(TOKEN_KEY);
}

/// The token in a `#t=TOKEN` fragment, if the fragment has one. `sentinel open` builds this link.
pub fn token_from_fragment(fragment: &str) -> Option<String> {
    let value = fragment.trim_start_matches('#').strip_prefix("t=")?;
    (!value.is_empty() && !value.contains(['&', '=', ' '])).then(|| value.to_owned())
}

/// Keep a token from the address bar, then remove it from the address so it is not shared or
/// bookmarked. The fragment never goes to the server.
pub fn capture_token_from_url() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let location = window.location();
    let Some(token) = location
        .hash()
        .ok()
        .and_then(|hash| token_from_fragment(&hash))
    else {
        return;
    };
    set_token(&token);
    if let Ok(history) = window.history() {
        let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some("/"));
    }
}

fn authorize(builder: RequestBuilder) -> RequestBuilder {
    match token() {
        Some(token) => builder.header("Authorization", &format!("Bearer {token}")),
        None => builder,
    }
}

async fn read<T: DeserializeOwned>(response: Response) -> Result<T, ApiFailure> {
    if response.ok() {
        return response
            .json::<T>()
            .await
            .map_err(|e| ApiFailure::Decode(e.to_string()));
    }
    let code = response.status();
    let message = match response.json::<ApiError>().await {
        Ok(body) => body.error,
        Err(_) => response.status_text(),
    };
    Err(ApiFailure::Status { code, message })
}

pub async fn get<T: DeserializeOwned>(path: &str) -> Result<T, ApiFailure> {
    let response = authorize(Request::get(path))
        .send()
        .await
        .map_err(|e| ApiFailure::Network(e.to_string()))?;
    read(response).await
}

pub async fn send<B: Serialize, T: DeserializeOwned>(
    method: &str,
    path: &str,
    body: &B,
) -> Result<T, ApiFailure> {
    let builder = match method {
        "PUT" => Request::put(path),
        "DELETE" => Request::delete(path),
        _ => Request::post(path),
    };
    let request = authorize(builder)
        .json(body)
        .map_err(|e| ApiFailure::Decode(e.to_string()))?;
    let response = request
        .send()
        .await
        .map_err(|e| ApiFailure::Network(e.to_string()))?;
    read(response).await
}

pub async fn post<B: Serialize, T: DeserializeOwned>(
    path: &str,
    body: &B,
) -> Result<T, ApiFailure> {
    send("POST", path, body).await
}

/// A POST with an empty JSON object body, for routes that take no input.
pub async fn post_empty<T: DeserializeOwned>(path: &str) -> Result<T, ApiFailure> {
    post(path, &serde_json::json!({})).await
}

pub async fn snapshot() -> Result<Snapshot, ApiFailure> {
    get("/api/state").await
}

/// The snapshot for `chat`: its stream events come with it. Without a chat the daemon picks the
/// live run's chat, or else the first chat that is not archived.
pub async fn snapshot_for(chat: Option<&ChatId>) -> Result<Snapshot, ApiFailure> {
    match chat {
        Some(id) => get(&format!("/api/state?chat={}", id.as_str())).await,
        None => snapshot().await,
    }
}

pub async fn list_chats() -> Result<Vec<ChatInfo>, ApiFailure> {
    get("/api/chats").await
}

pub async fn create_chat(title: Option<String>) -> Result<ChatInfo, ApiFailure> {
    post("/api/chats", &CreateChat { title }).await
}

pub async fn update_chat(id: &ChatId, update: &UpdateChat) -> Result<ChatInfo, ApiFailure> {
    send("PUT", &chat_path(id), update).await
}

pub async fn delete_chat(id: &ChatId) -> Result<serde_json::Value, ApiFailure> {
    send("DELETE", &chat_path(id), &serde_json::json!({})).await
}

fn chat_path(id: &ChatId) -> String {
    format!("/api/chats/{}", id.as_str())
}

pub async fn pending_attachments(chat: &ChatId) -> Result<Vec<Attachment>, ApiFailure> {
    get(&format!("{}/attachments", chat_path(chat))).await
}

pub async fn add_attachment(chat: &ChatId, body: &AddAttachment) -> Result<Attachment, ApiFailure> {
    post(&format!("{}/attachments", chat_path(chat)), body).await
}

pub async fn remove_attachment(
    chat: &ChatId,
    attachment: &AttachmentId,
) -> Result<serde_json::Value, ApiFailure> {
    let path = format!("{}/attachments/{}", chat_path(chat), attachment.as_str());
    send("DELETE", &path, &serde_json::json!({})).await
}

pub async fn start_run(request: &RunRequest) -> Result<serde_json::Value, ApiFailure> {
    post("/api/run", request).await
}

pub async fn kill_task(task: &TaskId) -> Result<serde_json::Value, ApiFailure> {
    post_empty(&format!("/api/tasks/{task}/kill")).await
}

#[cfg(test)]
mod fragment_tests {
    use super::token_from_fragment;

    #[test]
    fn a_token_fragment_gives_the_token() {
        assert_eq!(token_from_fragment("#t=abc123"), Some("abc123".to_owned()));
    }

    #[test]
    fn other_fragments_give_nothing() {
        assert_eq!(token_from_fragment(""), None);
        assert_eq!(token_from_fragment("#t="), None);
        assert_eq!(token_from_fragment("#x=abc"), None);
        assert_eq!(token_from_fragment("#t=a&b=c"), None);
    }
}
