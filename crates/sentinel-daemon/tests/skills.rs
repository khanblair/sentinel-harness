//! Skills on disk, end to end through the engine: create, list, read, switches, and the phone
//! rules, bad input, and found and import from another tool's folder.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use std::fs;
use std::path::Path;
use std::sync::Arc;

use reqwest::StatusCode;
use reqwest::header::AUTHORIZATION;
use sentinel_core::AttachmentKind;
use sentinel_core::api::AddAttachment;
use sentinel_core::skills::{EditSkill, ImportSkill, NewSkill, SkillMode, SkillScope};
use sentinel_daemon::cmd::SystemRunner;
use sentinel_daemon::controls::Caller;
use sentinel_daemon::engine::{Engine, EngineError, EngineOptions};
use sentinel_daemon::secrets::MemoryStore;
use sentinel_daemon::server;
use serde_json::Value;
use tokio::net::TcpListener;

mod common;
use common::sample_repo;

fn engine_with_home(repo: &Path, home: &Path) -> Engine {
    Engine::open(EngineOptions {
        repo_root: repo.to_path_buf(),
        home: home.to_path_buf(),
        runner: Arc::new(SystemRunner),
        secrets: Arc::new(MemoryStore::new()),
        fake_agents: None,
        in_memory: true,
    })
    .unwrap()
}

fn computer() -> Caller {
    Caller::pc("You")
}

fn phone() -> Caller {
    Caller {
        name: "Phone".to_owned(),
        from_pc: false,
    }
}

fn new_skill(name: &str, description: &str) -> NewSkill {
    NewSkill {
        name: name.to_owned(),
        description: description.to_owned(),
        body: "Check every join.".to_owned(),
        scope: SkillScope::Project,
    }
}

#[test]
fn a_created_skill_is_listed_and_read_back() {
    let repo = sample_repo();
    let home = tempfile::tempdir().unwrap();
    let engine = engine_with_home(repo.path(), home.path());
    engine
        .create_skill(new_skill("sql-review", "Reviews SQL joins."), &computer())
        .unwrap();
    let listed = engine.list_skills().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "sql-review");
    assert_eq!(listed[0].mode, SkillMode::Auto);
    let detail = engine.skill_detail("sql-review").unwrap();
    assert_eq!(detail.body, "Check every join.");
    assert_eq!(detail.info.description, "Reviews SQL joins.");
}

#[test]
fn a_phone_cannot_create_a_skill() {
    let repo = sample_repo();
    let home = tempfile::tempdir().unwrap();
    let engine = engine_with_home(repo.path(), home.path());
    let result = engine.create_skill(new_skill("sql-review", "Reviews SQL."), &phone());
    assert!(matches!(result, Err(EngineError::Refused(_))));
    assert_eq!(engine.list_skills().unwrap().len(), 0);
}

#[test]
fn a_phone_may_move_a_switch_toward_off_but_not_up() {
    let repo = sample_repo();
    let home = tempfile::tempdir().unwrap();
    let engine = engine_with_home(repo.path(), home.path());
    engine
        .create_skill(new_skill("sql-review", "Reviews SQL."), &computer())
        .unwrap();
    let off = EditSkill {
        mode: Some(SkillMode::Off),
        ..EditSkill::default()
    };
    assert_eq!(
        engine
            .edit_skill("sql-review", off, &phone())
            .unwrap()
            .info
            .mode,
        SkillMode::Off
    );
    let up = EditSkill {
        mode: Some(SkillMode::Always),
        ..EditSkill::default()
    };
    assert!(matches!(
        engine.edit_skill("sql-review", up, &phone()),
        Err(EngineError::Refused(_))
    ));
}

#[test]
fn a_bad_name_and_an_over_long_description_are_refused() {
    let repo = sample_repo();
    let home = tempfile::tempdir().unwrap();
    let engine = engine_with_home(repo.path(), home.path());
    let bad_name = engine.create_skill(new_skill("Bad Name", "Fine."), &computer());
    assert!(matches!(bad_name, Err(EngineError::Invalid(_))));
    let long = "d".repeat(301);
    let long_text = engine.create_skill(new_skill("long-one", &long), &computer());
    assert!(matches!(long_text, Err(EngineError::Invalid(_))));
    assert_eq!(engine.list_skills().unwrap().len(), 0);
}

#[test]
fn a_skill_from_another_tools_folder_is_found_and_imported_without_touching_the_original() {
    let repo = sample_repo();
    let home = tempfile::tempdir().unwrap();
    let engine = engine_with_home(repo.path(), home.path());
    let original = repo.path().join(".claude/skills/review");
    fs::create_dir_all(&original).unwrap();
    let text = "---\nname: review\ndescription: Reviews diffs.\n---\nRead the diff first.\n";
    fs::write(original.join("SKILL.md"), text).unwrap();

    let found = engine.found_skills().unwrap();
    let review = found.iter().find(|s| s.name == "review").unwrap();
    assert_eq!(review.source, ".claude/skills");
    assert!(!review.already_imported);

    let request = ImportSkill {
        path: review.path.clone(),
        scope: SkillScope::Personal,
    };
    let imported = engine.import_skill(request, &computer()).unwrap();
    assert_eq!(imported.scope, SkillScope::Personal);
    assert_eq!(imported.mode, SkillMode::Auto);
    assert!(
        home.path()
            .join(".sentinel/skills/review/SKILL.md")
            .is_file()
    );
    assert_eq!(fs::read_to_string(original.join("SKILL.md")).unwrap(), text);
}

#[test]
fn a_skill_chip_keeps_the_skill_name_and_its_body() {
    let repo = sample_repo();
    let home = tempfile::tempdir().unwrap();
    let engine = engine_with_home(repo.path(), home.path());
    engine
        .create_skill(new_skill("sql-review", "Reviews SQL."), &computer())
        .unwrap();
    let chat = engine.list_chats().unwrap()[0].id.clone();
    let request = AddAttachment {
        kind: AttachmentKind::Skill,
        path: None,
        text: None,
        name: Some("sql-review".to_owned()),
        content_base64: None,
    };
    let chip = engine.add_attachment(&chat, &request).unwrap();
    assert_eq!(chip.kind, AttachmentKind::Skill);
    assert_eq!(chip.path.as_deref(), Some("sql-review"));
    assert_eq!(chip.text.as_deref(), Some("Check every join."));
}

#[tokio::test]
async fn the_found_route_is_not_read_as_a_skill_name() {
    let repo = sample_repo();
    let home = tempfile::tempdir().unwrap();
    let engine = engine_with_home(repo.path(), home.path());
    let token = engine.ensure_desktop_token().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, server::router(engine)).await;
    });
    let response = reqwest::Client::new()
        .get(format!("http://{addr}/api/skills/found"))
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert!(body.is_array(), "{body}");
}

#[test]
fn an_import_refuses_a_folder_the_scan_does_not_cover() {
    let repo = sample_repo();
    let home = tempfile::tempdir().unwrap();
    let engine = engine_with_home(repo.path(), home.path());
    let elsewhere = repo.path().join("elsewhere/review");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::write(
        elsewhere.join("SKILL.md"),
        "---\ndescription: x\n---\nBody\n",
    )
    .unwrap();
    let request = ImportSkill {
        path: elsewhere.to_string_lossy().into_owned(),
        scope: SkillScope::Project,
    };
    let result = engine.import_skill(request, &computer());
    assert!(matches!(result, Err(EngineError::Refused(_))));
    assert_eq!(engine.list_skills().unwrap().len(), 0);
}
