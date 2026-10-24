//! Chats end to end with the fake agents: create, rename, pin, archive, delete, a run from plain
//! words, a run from a backlog attachment, the busy-chat rule, and uploads.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use sentinel_core::api::{AddAttachment, RunRequest, UpdateChat};
use sentinel_core::{
    Attachment, AttachmentKind, ChatChange, ChatId, ChatRunState, Event, EventRecord,
};
use sentinel_daemon::controls::Caller;
use sentinel_daemon::engine::{Engine, EngineError};

mod common;
use common::{fake_engine, sample_repo, wait_done};

/// Add a repository path or a note to a chat, as the plus button does. `what` is the path, or the
/// text of a note.
fn attach(engine: &Engine, chat: &ChatId, kind: AttachmentKind, what: &str) -> Attachment {
    let is_note = kind == AttachmentKind::Note;
    let request = AddAttachment {
        kind,
        path: (!is_note).then(|| what.to_owned()),
        text: is_note.then(|| what.to_owned()),
        name: None,
        content_base64: None,
    };
    engine.add_attachment(chat, &request).unwrap()
}

fn computer() -> Caller {
    Caller::pc("You")
}

fn changes_of(events: &[EventRecord]) -> Vec<ChatChange> {
    events
        .iter()
        .filter_map(|record| {
            if let Event::ChatChanged { change, .. } = &record.event {
                Some(change.clone())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn a_new_project_has_one_empty_chat() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    let chats = engine.list_chats().unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].title, "New chat");
    assert_eq!(chats[0].run, ChatRunState::Idle);
}

#[test]
fn a_chat_is_renamed_pinned_archived_and_deleted() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    let chat = engine.create_chat(None, &computer()).unwrap();
    let rename = UpdateChat {
        title: Some("Rate limit".to_owned()),
        pinned: Some(true),
        ..UpdateChat::default()
    };
    let updated = engine.update_chat(&chat.id, &rename, &computer()).unwrap();
    assert_eq!(updated.title, "Rate limit");
    assert!(updated.pinned);

    let archive = UpdateChat {
        archived: Some(true),
        ..UpdateChat::default()
    };
    let archived = engine.update_chat(&chat.id, &archive, &computer()).unwrap();
    assert!(archived.archived);
    // Archived chats sort last.
    assert_eq!(engine.list_chats().unwrap().last().unwrap().id, chat.id);

    engine.delete_chat(&chat.id, &computer()).unwrap();
    assert!(engine.get_chat(&chat.id).is_err());
    let all = engine.events_after(0, 1000).unwrap();
    assert!(changes_of(&all).contains(&ChatChange::Deleted));
}

#[test]
fn an_empty_name_is_refused() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    let chat = engine.list_chats().unwrap().remove(0);
    let blank = UpdateChat {
        title: Some("  ".to_owned()),
        ..UpdateChat::default()
    };
    let error = engine
        .update_chat(&chat.id, &blank, &computer())
        .unwrap_err();
    assert!(matches!(error, EngineError::Invalid(_)));
}

#[test]
fn deleting_the_last_chat_leaves_a_fresh_one() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    let only = engine.list_chats().unwrap().remove(0);
    engine.delete_chat(&only.id, &computer()).unwrap();
    let chats = engine.list_chats().unwrap();
    assert_eq!(chats.len(), 1);
    assert_ne!(chats[0].id, only.id);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_message_starts_a_run_in_its_chat_and_names_the_chat() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    engine.scan_tools().await.unwrap();
    let chat = engine.list_chats().unwrap().remove(0);
    let request = RunRequest {
        chat: Some(chat.id.clone()),
        text: Some("Add a health check function\nA health() function returns ok.".to_owned()),
        ..RunRequest::default()
    };
    let started = engine.start_run(request).await.unwrap();
    assert_eq!(started.chat, chat.id);
    wait_done(&engine).await;

    let after = engine.get_chat(&chat.id).unwrap();
    assert_eq!(after.title, "Add a health check function");
    assert_eq!(after.total, 1);
    let events = engine.chat_events(&chat.id, 0, 1000).unwrap();
    assert!(events.iter().any(
        |r| matches!(&r.event, Event::Message { text, .. } if text.starts_with("Add a health"))
    ));
    assert!(
        events
            .iter()
            .any(|r| matches!(r.event, Event::RunStarted { .. }))
    );
    assert!(
        events
            .iter()
            .any(|r| matches!(r.event, Event::RunFinished { .. }))
    );
    assert!(events.iter().all(|r| r.chat.as_ref() == Some(&chat.id)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_second_chat_cannot_start_a_run_while_the_first_is_live() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    engine.scan_tools().await.unwrap();
    let first = engine.list_chats().unwrap().remove(0);
    let second = engine.create_chat(None, &computer()).unwrap();
    let start = RunRequest {
        chat: Some(first.id.clone()),
        text: Some("Add a health check function".to_owned()),
        ..RunRequest::default()
    };
    engine.start_run(start).await.unwrap();

    let again = RunRequest {
        chat: Some(second.id.clone()),
        text: Some("Another thing".to_owned()),
        ..RunRequest::default()
    };
    let error = engine.start_run(again).await.unwrap_err();
    let EngineError::Refused(text) = error else {
        panic!("expected a refusal");
    };
    assert!(text.contains("Add a health check function"), "{text}");
    // The first chat has a live run, so it cannot be deleted.
    let live = engine.delete_chat(&first.id, &computer());
    assert!(matches!(live, Err(EngineError::Refused(_))));
    engine.stop_run().unwrap();
    wait_done(&engine).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_backlog_attachment_and_a_note_reach_the_tasks() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    engine.scan_tools().await.unwrap();
    let chat = engine.list_chats().unwrap().remove(0);
    let backlog = attach(&engine, &chat.id, AttachmentKind::Backlog, "backlog.md");
    let note = attach(
        &engine,
        &chat.id,
        AttachmentKind::Note,
        "Keep functions short.",
    );
    assert_eq!(engine.pending_attachments(&chat.id).unwrap().len(), 2);

    let request = RunRequest {
        chat: Some(chat.id.clone()),
        attachments: vec![backlog.id.clone(), note.id.clone()],
        ..RunRequest::default()
    };
    engine.start_run(request).await.unwrap();
    wait_done(&engine).await;

    assert_eq!(engine.pending_attachments(&chat.id).unwrap().len(), 0);
    let task = engine.store().list_tasks().unwrap().remove(0);
    assert!(task.body.contains("Keep functions short."), "{}", task.body);
    let events = engine.chat_events(&chat.id, 0, 1000).unwrap();
    let sent = events.iter().find_map(|r| {
        if let Event::AttachmentsSent { attachments } = &r.event {
            Some(attachments.len())
        } else {
            None
        }
    });
    assert_eq!(sent, Some(2));
}

#[test]
fn an_upload_is_kept_outside_the_tracked_tree_and_removed_with_its_chip() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    let chat = engine.list_chats().unwrap().remove(0);
    let upload = engine
        .add_attachment(
            &chat.id,
            &AddAttachment {
                kind: AttachmentKind::Upload,
                path: None,
                text: None,
                name: Some("notes.txt".to_owned()),
                content_base64: Some(STANDARD.encode("hello")),
            },
        )
        .unwrap();
    let stored = upload.path.clone().unwrap();
    assert!(stored.starts_with(".sentinel/chats/"), "{stored}");
    assert!(repo.path().join(&stored).is_file());
    engine.remove_attachment(&chat.id, &upload.id).unwrap();
    assert!(!repo.path().join(&stored).exists());
}

#[test]
fn a_file_outside_the_project_and_a_big_upload_are_refused() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    let chat = engine.list_chats().unwrap().remove(0);
    let outside = AddAttachment {
        kind: AttachmentKind::File,
        path: Some("../secret.txt".to_owned()),
        text: None,
        name: None,
        content_base64: None,
    };
    assert!(matches!(
        engine.add_attachment(&chat.id, &outside),
        Err(EngineError::Refused(_))
    ));
    let big = AddAttachment {
        kind: AttachmentKind::Upload,
        path: None,
        text: None,
        name: Some("big.bin".to_owned()),
        content_base64: Some(STANDARD.encode(vec![0_u8; 1_000_001])),
    };
    assert!(matches!(
        engine.add_attachment(&chat.id, &big),
        Err(EngineError::Invalid(_))
    ));
}

#[test]
fn an_archived_chat_takes_no_attachments() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    let chat = engine.create_chat(None, &computer()).unwrap();
    let archive = UpdateChat {
        archived: Some(true),
        ..UpdateChat::default()
    };
    engine.update_chat(&chat.id, &archive, &computer()).unwrap();
    let note = AddAttachment {
        kind: AttachmentKind::Note,
        path: None,
        text: Some("x".to_owned()),
        name: None,
        content_base64: None,
    };
    assert!(matches!(
        engine.add_attachment(&chat.id, &note),
        Err(EngineError::Refused(_))
    ));
}

#[allow(dead_code)]
fn engine_is_send_and_sync(engine: Engine) -> impl Send + Sync {
    engine
}
