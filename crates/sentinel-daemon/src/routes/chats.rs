//! Chats and their attachments: the sidebar list, new chat, rename, pin, archive, delete, and the
//! plus button. A phone may do all of it except delete (`docs/mobile.md`, Security).

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post};
use serde::Deserialize;
use serde_json::Value;

use sentinel_core::api::{AddAttachment, CreateChat, UpdateChat};
use sentinel_core::{Attachment, AttachmentId, ChatId, ChatInfo, EventRecord};

use super::state::{PageQuery, page_limit};
use super::{ApiResult, blocking, okay};
use crate::auth::Computer;
use crate::controls::Caller;
use crate::engine::Engine;

/// The routes that carry file bytes. They take a bigger body than the rest of the API.
pub(super) fn upload_routes() -> Router<Engine> {
    Router::new().route("/api/chats/{id}/attachments", post(add_attachment))
}

pub(super) fn routes() -> Router<Engine> {
    Router::new()
        .route("/api/chats", get(list_chats).post(create_chat))
        .route("/api/chats/{id}", delete(delete_chat).put(update_chat))
        .route("/api/chats/{id}/events", get(chat_events))
        .route("/api/chats/{id}/attachments", get(pending_attachments))
        .route(
            "/api/chats/{id}/attachments/{attachment}",
            delete(remove_attachment),
        )
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    /// `true` for archived chats only, `false` for the rest. Both when absent.
    archived: Option<bool>,
}

async fn list_chats(
    State(engine): State<Engine>,
    Query(query): Query<ListQuery>,
) -> ApiResult<Vec<ChatInfo>> {
    blocking(engine, move |e| {
        e.list_chats().map(|chats| {
            chats
                .into_iter()
                .filter(|chat| query.archived.is_none_or(|only| chat.archived == only))
                .collect()
        })
    })
    .await
}

async fn create_chat(
    State(engine): State<Engine>,
    caller: Caller,
    Json(body): Json<CreateChat>,
) -> ApiResult<ChatInfo> {
    blocking(engine, move |e| {
        e.create_chat(body.title.as_deref(), &caller)
    })
    .await
}

async fn update_chat(
    State(engine): State<Engine>,
    caller: Caller,
    Path(id): Path<String>,
    Json(update): Json<UpdateChat>,
) -> ApiResult<ChatInfo> {
    blocking(engine, move |e| {
        e.update_chat(&ChatId::new(id), &update, &caller)
    })
    .await
}

async fn delete_chat(
    State(engine): State<Engine>,
    Computer(caller): Computer,
    Path(id): Path<String>,
) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.delete_chat(&ChatId::new(id), &caller).map(|()| okay())
    })
    .await
}

async fn chat_events(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    Query(query): Query<PageQuery>,
) -> ApiResult<Vec<EventRecord>> {
    let after = query.after.unwrap_or(0);
    let limit = u32::try_from(page_limit(query.limit)).unwrap_or(u32::MAX);
    blocking(engine, move |e| {
        e.chat_events(&ChatId::new(id), after, limit)
    })
    .await
}

async fn pending_attachments(
    State(engine): State<Engine>,
    Path(id): Path<String>,
) -> ApiResult<Vec<Attachment>> {
    blocking(engine, move |e| e.pending_attachments(&ChatId::new(id))).await
}

async fn add_attachment(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    Json(body): Json<AddAttachment>,
) -> ApiResult<Attachment> {
    blocking(engine, move |e| e.add_attachment(&ChatId::new(id), &body)).await
}

async fn remove_attachment(
    State(engine): State<Engine>,
    Path((id, attachment)): Path<(String, String)>,
) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.remove_attachment(&ChatId::new(id), &AttachmentId::new(attachment))
            .map(|()| okay())
    })
    .await
}
