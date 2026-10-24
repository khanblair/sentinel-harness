//! The Brain routes under `/api/brain`. A phone may read notes and the graph, review lessons, and
//! answer proposals. Writing or pinning a note, the care settings, and a care run need the computer.
//! `POST /api/brain/mode` lives in `run.rs`.

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::{Value, json};

use sentinel_core::brain_api::{
    AnswerProposal, CareSettings, GraphView, NewNote, NoteDetail, NoteList, PinNote, ProposalView,
    ReviewLesson, SaveNote,
};
use sentinel_core::{NoteId, NoteType};

use super::{ApiResult, blocking, okay, require_computer};
use crate::controls::Caller;
use crate::engine::Engine;

pub(super) fn routes() -> Router<Engine> {
    Router::new()
        .route("/api/brain/notes", get(list_notes).post(create_note))
        .route("/api/brain/notes/{id}", get(read_note).put(save_note))
        .route("/api/brain/notes/{id}/pin", post(pin_note))
        .route("/api/brain/graph", get(graph))
        .route("/api/brain/lessons/{id}/review", post(review_lesson))
        .route("/api/brain/proposals", get(proposals))
        .route("/api/brain/proposals/{id}", post(answer_proposal))
        .route("/api/brain/care", get(read_care).put(save_care))
        .route("/api/brain/care/run", post(run_care))
}

#[derive(Debug, Deserialize)]
struct NotesQuery {
    q: Option<String>,
    #[serde(rename = "type")]
    note_type: Option<NoteType>,
}

async fn list_notes(
    State(engine): State<Engine>,
    Query(query): Query<NotesQuery>,
) -> ApiResult<NoteList> {
    blocking(engine, move |e| {
        e.brain_notes(query.q.as_deref(), query.note_type)
    })
    .await
}

async fn create_note(
    State(engine): State<Engine>,
    caller: Caller,
    Json(body): Json<NewNote>,
) -> ApiResult<NoteDetail> {
    require_computer(&caller)?;
    blocking(engine, move |e| e.new_note(body, &caller)).await
}

async fn read_note(State(engine): State<Engine>, Path(id): Path<String>) -> ApiResult<NoteDetail> {
    blocking(engine, move |e| e.brain_note(&NoteId::new(id))).await
}

async fn save_note(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    caller: Caller,
    Json(body): Json<SaveNote>,
) -> ApiResult<NoteDetail> {
    require_computer(&caller)?;
    blocking(engine, move |e| {
        e.save_note(&NoteId::new(id), body, &caller)
    })
    .await
}

async fn pin_note(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    caller: Caller,
    Json(body): Json<PinNote>,
) -> ApiResult<NoteDetail> {
    require_computer(&caller)?;
    blocking(engine, move |e| e.pin_note(&NoteId::new(id), body, &caller)).await
}

async fn graph(State(engine): State<Engine>) -> ApiResult<GraphView> {
    blocking(engine, Engine::brain_graph).await
}

async fn review_lesson(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    Json(body): Json<ReviewLesson>,
) -> ApiResult<NoteDetail> {
    blocking(engine, move |e| e.review_lesson(&NoteId::new(id), body)).await
}

async fn proposals(State(engine): State<Engine>) -> ApiResult<Vec<ProposalView>> {
    blocking(engine, Engine::brain_proposals).await
}

async fn answer_proposal(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    Json(body): Json<AnswerProposal>,
) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.answer_proposal(&id, body).map(|()| okay())
    })
    .await
}

async fn read_care(State(engine): State<Engine>) -> ApiResult<CareSettings> {
    blocking(engine, Engine::care_settings).await
}

async fn save_care(
    State(engine): State<Engine>,
    caller: Caller,
    Json(body): Json<CareSettings>,
) -> ApiResult<CareSettings> {
    require_computer(&caller)?;
    blocking(engine, move |e| e.set_care_settings(body, &caller)).await
}

async fn run_care(State(engine): State<Engine>, caller: Caller) -> ApiResult<Value> {
    require_computer(&caller)?;
    blocking(engine, move |e| {
        e.run_care(&caller)
            .map(|count| json!({ "proposals": count }))
    })
    .await
}
