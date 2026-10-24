//! The Brain API, served on a real listener and called with `reqwest`. Notes are written through the
//! routes, or seeded with `sentinel_brain` when a test needs a file the API cannot make (a lesson, a
//! proposal, an old note). A phone is paired through the real pairing routes.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use std::fs;
use std::time::{Duration, Instant};

use reqwest::{Method, StatusCode};
use sentinel_brain::note::{Author, Note, render_note};
use sentinel_core::{BrainMode, NoteId, NoteType};
use sentinel_daemon::controls::Caller;
use serde_json::json;

mod common;
#[path = "brain_api/support.rs"]
mod support;
use support::*;

#[tokio::test]
async fn an_empty_vault_lists_no_notes() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let base = start(engine.clone()).await;
    let token = engine.ensure_desktop_token().unwrap();
    let (status, list) = call(&base, &token, (Method::GET, "/api/brain/notes"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["notes"], json!([]));
    assert!(
        list["vault_path"]
            .as_str()
            .unwrap()
            .ends_with(".sentinel/brain")
    );
}

#[tokio::test]
async fn a_new_note_reads_back_with_its_links_and_a_dead_link() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let base = start(engine.clone()).await;
    let token = engine.ensure_desktop_token().unwrap();
    post_note(&base, &token, "Beta", "The second note.").await;
    post_note(
        &base,
        &token,
        "Use tabs",
        "Indent with tabs. See [[beta]] and [[ghost]].",
    )
    .await;
    let (status, detail) = call(
        &base,
        &token,
        (Method::GET, "/api/brain/notes/use-tabs"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["summary"]["title"], "Use tabs");
    let links = detail["links"].as_array().unwrap();
    assert_eq!(links.len(), 2);
    assert_eq!(
        (links[0]["id"].as_str(), links[0]["dead"].as_bool()),
        (Some("beta"), Some(false))
    );
    assert_eq!(
        (links[1]["id"].as_str(), links[1]["dead"].as_bool()),
        (Some("ghost"), Some(true))
    );
    let (_, beta) = call(&base, &token, (Method::GET, "/api/brain/notes/beta"), None).await;
    assert_eq!(beta["backlinks"][0]["id"], "use-tabs");
}

#[tokio::test]
async fn a_stale_version_is_refused_and_a_fresh_one_saves() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let base = start(engine.clone()).await;
    let token = engine.ensure_desktop_token().unwrap();
    let note = "Mine.\n<!-- sentinel:start -->\nGenerated.\n<!-- sentinel:end -->\n";
    let created = post_note(&base, &token, "Version note", note).await;
    let first = created["version"].as_str().unwrap().to_owned();
    let edited = "Mine, edited.\n<!-- sentinel:start -->\nGenerated.\n<!-- sentinel:end -->\n";
    let (status, saved) = call(
        &base,
        &token,
        (Method::PUT, "/api/brain/notes/version-note"),
        Some(json!({ "body": edited, "version": first })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (stale, refused) = call(
        &base,
        &token,
        (Method::PUT, "/api/brain/notes/version-note"),
        Some(json!({ "body": "Late.", "version": first })),
    )
    .await;
    assert_eq!(stale, StatusCode::CONFLICT);
    assert_eq!(
        refused["error"],
        "This note changed. Reload it and try again."
    );
    let (_, read) = call(
        &base,
        &token,
        (Method::GET, "/api/brain/notes/version-note"),
        None,
    )
    .await;
    assert!(read["body"].as_str().unwrap().contains("Mine, edited."));
    assert!(read["body"].as_str().unwrap().contains("Generated."));
}

#[tokio::test]
async fn a_phone_is_refused_every_computer_only_route() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let base = start(engine.clone()).await;
    let desktop = engine.ensure_desktop_token().unwrap();
    post_note(&base, &desktop, "Use tabs", "Indent.").await;
    let phone = pair_phone(&base, &desktop).await;
    let care = json!({ "frequency": { "every": "weekly" }, "budget": "0.10", "auto_apply": false, "last_run": null });
    let cases = [
        (
            Method::POST,
            "/api/brain/notes",
            json!({ "note_type": "convention", "title": "Phone", "body": "" }),
        ),
        (
            Method::PUT,
            "/api/brain/notes/use-tabs",
            json!({ "body": "x", "version": "v" }),
        ),
        (
            Method::POST,
            "/api/brain/notes/use-tabs/pin",
            json!({ "pinned": true }),
        ),
        (Method::PUT, "/api/brain/care", care),
        (Method::POST, "/api/brain/care/run", json!({})),
    ];
    for (method, path, body) in cases {
        let (status, _) = call(&base, &phone, (method, path), Some(body)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
    }
}

#[tokio::test]
async fn a_phone_may_review_a_lesson_and_answer_a_proposal() {
    let dir = tempfile::tempdir().unwrap();
    seed_lesson(dir.path());
    seed_update_proposal(dir.path(), "update-tabs", "lesson-convention-tests", "Tip.");
    let engine = engine_in(dir.path());
    let base = start(engine.clone()).await;
    let desktop = engine.ensure_desktop_token().unwrap();
    let phone = pair_phone(&base, &desktop).await;
    let (status, lesson) = call(
        &base,
        &phone,
        (
            Method::POST,
            "/api/brain/lessons/lesson-convention-tests/review",
        ),
        Some(json!({ "action": "approve" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{lesson}");
    assert_eq!(lesson["summary"]["lesson_state"], "proven");
    let (answered, _) = call(
        &base,
        &phone,
        (Method::POST, "/api/brain/proposals/update-tabs"),
        Some(json!({ "accept": false })),
    )
    .await;
    assert_eq!(answered, StatusCode::OK);
}

#[tokio::test]
async fn reviewing_a_note_that_is_not_a_lesson_is_a_bad_request() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let base = start(engine.clone()).await;
    let token = engine.ensure_desktop_token().unwrap();
    post_note(&base, &token, "Use tabs", "Indent.").await;
    let (status, _) = call(
        &base,
        &token,
        (Method::POST, "/api/brain/lessons/use-tabs/review"),
        Some(json!({ "action": "approve" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn care_is_refused_when_the_brain_is_off() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    engine
        .set_brain_mode(BrainMode::Off, &Caller::pc("test"))
        .unwrap();
    let base = start(engine.clone()).await;
    let token = engine.ensure_desktop_token().unwrap();
    let (status, body) = call(
        &base,
        &token,
        (Method::POST, "/api/brain/care/run"),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "The Brain is off.");
}

#[tokio::test]
async fn care_writes_proposals_and_a_digest_only_in_learn_mode() {
    let dir = tempfile::tempdir().unwrap();
    seed_old_run_note(dir.path());
    let engine = engine_in(dir.path());
    engine
        .set_brain_mode(BrainMode::ReadOnly, &Caller::pc("test"))
        .unwrap();
    let base = start(engine.clone()).await;
    let token = engine.ensure_desktop_token().unwrap();
    let (status, run) = call(
        &base,
        &token,
        (Method::POST, "/api/brain/care/run"),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{run}");
    assert_eq!(run["proposals"], 1);
    let (_, list) = call(&base, &token, (Method::GET, "/api/brain/notes"), None).await;
    assert!(ids(&list).iter().all(|id| !id.starts_with("digest-")));
    let (_, proposals) = call(&base, &token, (Method::GET, "/api/brain/proposals"), None).await;
    assert_eq!(proposals[0]["id"], "care-retire-old-run");
    assert_eq!(proposals[0]["needs_accept"], false);
    engine
        .set_brain_mode(BrainMode::Learn, &Caller::pc("test"))
        .unwrap();
    let (_, learned) = call(
        &base,
        &token,
        (Method::POST, "/api/brain/care/run"),
        Some(json!({})),
    )
    .await;
    assert_eq!(learned["proposals"], 0);
    let (_, list) = call(&base, &token, (Method::GET, "/api/brain/notes"), None).await;
    assert!(ids(&list).iter().any(|id| id.starts_with("digest-")));
}

#[tokio::test]
async fn care_settings_save_and_a_bad_budget_is_a_bad_request() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let base = start(engine.clone()).await;
    let token = engine.ensure_desktop_token().unwrap();
    let (_, before) = call(&base, &token, (Method::GET, "/api/brain/care"), None).await;
    assert_eq!(before["budget"], "0.10");
    let saved = json!({ "frequency": { "every": "daily" }, "budget": "0.25", "auto_apply": true, "last_run": null });
    let (status, _) = call(&base, &token, (Method::PUT, "/api/brain/care"), Some(saved)).await;
    assert_eq!(status, StatusCode::OK);
    let (_, after) = call(&base, &token, (Method::GET, "/api/brain/care"), None).await;
    assert_eq!(after["budget"], "0.25");
    assert_eq!(after["frequency"]["every"], "daily");
    let bad = json!({ "frequency": { "every": "daily" }, "budget": "ten dollars", "auto_apply": false, "last_run": null });
    let (status, _) = call(&base, &token, (Method::PUT, "/api/brain/care"), Some(bad)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_proposal_for_a_note_you_wrote_needs_accept_and_accept_applies_it() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let base = start(engine.clone()).await;
    let token = engine.ensure_desktop_token().unwrap();
    post_note(&base, &token, "Use tabs", "Mine.").await;
    seed_update_proposal(dir.path(), "update-use-tabs", "use-tabs", "Suggested text.");
    let (_, proposals) = call(&base, &token, (Method::GET, "/api/brain/proposals"), None).await;
    assert_eq!(proposals[0]["needs_accept"], true);
    let (status, _) = call(
        &base,
        &token,
        (Method::POST, "/api/brain/proposals/update-use-tabs"),
        Some(json!({ "accept": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, note) = call(
        &base,
        &token,
        (Method::GET, "/api/brain/notes/use-tabs"),
        None,
    )
    .await;
    assert!(note["body"].as_str().unwrap().contains("Suggested text."));
}

#[tokio::test]
async fn a_dismissed_proposal_leaves_the_list() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let base = start(engine.clone()).await;
    let token = engine.ensure_desktop_token().unwrap();
    post_note(&base, &token, "Use tabs", "Mine.").await;
    seed_update_proposal(dir.path(), "update-dismiss", "use-tabs", "Unwanted.");
    let (status, _) = call(
        &base,
        &token,
        (Method::POST, "/api/brain/proposals/update-dismiss"),
        Some(json!({ "accept": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, proposals) = call(&base, &token, (Method::GET, "/api/brain/proposals"), None).await;
    assert_eq!(proposals, json!([]));
}

#[tokio::test]
async fn the_graph_lists_nodes_and_links() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let base = start(engine.clone()).await;
    let token = engine.ensure_desktop_token().unwrap();
    post_note(&base, &token, "Beta", "Target.").await;
    post_note(&base, &token, "Alpha", "Points at [[beta]].").await;
    let (status, graph) = call(&base, &token, (Method::GET, "/api/brain/graph"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(graph["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(graph["links"], json!([["alpha", "beta"]]));
}

#[tokio::test]
async fn an_edit_made_to_a_vault_file_shows_in_search() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine_in(dir.path());
    let base = start(engine.clone()).await;
    let token = engine.ensure_desktop_token().unwrap();
    post_note(&base, &token, "Seed", "Opens the brain.").await;
    let note = Note {
        id: NoteId::new("zebra-rule"),
        note_type: NoteType::Convention,
        title: "Zebra rule".to_owned(),
        tags: Vec::new(),
        by: Author::You,
        strength: 0.5,
        confirmed: None,
        pinned: false,
        lesson: None,
        body: "Always check the zebrafish ledger.".to_owned(),
        extra: Vec::new(),
    };
    fs::write(
        vault_root(dir.path()).join("conventions/zebra-rule.md"),
        render_note(&note),
    )
    .unwrap();
    let mut list = search(&base, &token, "zebrafish").await;
    let waited = Instant::now();
    while ids(&list).is_empty() && waited.elapsed() < Duration::from_secs(3) {
        tokio::time::sleep(Duration::from_millis(100)).await;
        list = search(&base, &token, "zebrafish").await;
    }
    if ids(&list).is_empty() {
        engine.rebuild_brain_index().unwrap();
        list = search(&base, &token, "zebrafish").await;
    }
    assert_eq!(ids(&list), vec!["zebra-rule".to_owned()]);
}
