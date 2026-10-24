//! The Skills routes under `/api/skills`. A phone may list and read skills, see the found ones,
//! and turn a switch toward Off. Creating, deleting, editing text, and importing need the
//! computer. The engine checks the same rules again.

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use serde_json::Value;

use sentinel_core::skills::{EditSkill, FoundSkill, ImportSkill, NewSkill, SkillDetail, SkillInfo};

use super::{ApiResult, blocking, okay};
use crate::auth::Computer;
use crate::controls::Caller;
use crate::engine::Engine;

pub(super) fn routes() -> Router<Engine> {
    Router::new()
        .route("/api/skills", get(list_skills).post(create_skill))
        .route("/api/skills/found", get(found_skills))
        .route("/api/skills/import", post(import_skill))
        .route(
            "/api/skills/{name}",
            get(read_skill).put(edit_skill).delete(delete_skill),
        )
}

async fn list_skills(State(engine): State<Engine>) -> ApiResult<Vec<SkillInfo>> {
    blocking(engine, Engine::list_skills).await
}

async fn read_skill(
    State(engine): State<Engine>,
    Path(name): Path<String>,
) -> ApiResult<SkillDetail> {
    blocking(engine, move |e| e.skill_detail(&name)).await
}

async fn create_skill(
    State(engine): State<Engine>,
    Computer(caller): Computer,
    Json(body): Json<NewSkill>,
) -> ApiResult<SkillDetail> {
    blocking(engine, move |e| e.create_skill(body, &caller)).await
}

async fn edit_skill(
    State(engine): State<Engine>,
    Path(name): Path<String>,
    caller: Caller,
    Json(body): Json<EditSkill>,
) -> ApiResult<SkillDetail> {
    blocking(engine, move |e| e.edit_skill(&name, body, &caller)).await
}

async fn delete_skill(
    State(engine): State<Engine>,
    Path(name): Path<String>,
    Computer(caller): Computer,
) -> ApiResult<Value> {
    blocking(engine, move |e| {
        e.delete_skill(&name, &caller).map(|()| okay())
    })
    .await
}

/// The folders it reads are on this computer, and the answer names their paths: the computer only.
async fn found_skills(
    State(engine): State<Engine>,
    Computer(_): Computer,
) -> ApiResult<Vec<FoundSkill>> {
    blocking(engine, Engine::found_skills).await
}

async fn import_skill(
    State(engine): State<Engine>,
    Computer(caller): Computer,
    Json(body): Json<ImportSkill>,
) -> ApiResult<SkillInfo> {
    blocking(engine, move |e| e.import_skill(body, &caller)).await
}
