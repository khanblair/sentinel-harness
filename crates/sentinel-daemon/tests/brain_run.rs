//! The Brain during a run: pinned notes reach the stream as "loaded", and the journal is written
//! in learn mode only.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use sentinel_core::api::RunRequest;
use sentinel_core::brain_api::{NewNote, PinNote};
use sentinel_core::{BrainMode, Event, NoteType};
use sentinel_daemon::controls::Caller;
use sentinel_daemon::engine::Engine;

mod common;
use common::{fake_engine, sample_repo, wait_done};

fn pc() -> Caller {
    Caller::pc("test")
}

async fn run_with_mode(mode: BrainMode, pinned: bool) -> (tempfile::TempDir, Engine) {
    let dir = sample_repo();
    let engine = fake_engine(dir.path());
    engine.scan_tools().await.unwrap();
    engine.set_brain_mode(mode, &pc()).unwrap();
    if pinned {
        let note = engine
            .new_note(
                NewNote {
                    note_type: NoteType::Convention,
                    title: "Errors use thiserror".to_owned(),
                    body: "Library errors use thiserror.".to_owned(),
                },
                &pc(),
            )
            .unwrap();
        engine
            .pin_note(&note.summary.id, PinNote { pinned: true }, &pc())
            .unwrap();
    }
    engine.start_run(RunRequest::default()).await.unwrap();
    wait_done(&engine).await;
    (dir, engine)
}

fn loaded_lines(engine: &Engine) -> Vec<String> {
    engine
        .events_after(0, 100_000)
        .unwrap()
        .into_iter()
        .filter_map(|record| {
            let Event::Message { text, .. } = record.event else {
                return None;
            };
            text.starts_with("Brain loaded").then_some(text)
        })
        .collect()
}

fn note_types(engine: &Engine) -> Vec<NoteType> {
    engine
        .brain_notes(None, None)
        .unwrap()
        .notes
        .into_iter()
        .map(|note| note.note_type)
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn learn_mode_loads_pinned_notes_and_writes_the_journal() {
    let (_dir, engine) = run_with_mode(BrainMode::Learn, true).await;

    let lines = loaded_lines(&engine);
    let first = lines
        .first()
        .expect("the stream says which notes were loaded");
    assert!(first.contains("1 pinned"), "{lines:?}");

    let types = note_types(&engine);
    for wanted in [NoteType::Run, NoteType::Task, NoteType::File] {
        assert!(
            types.contains(&wanted),
            "{wanted:?} note missing: {types:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn read_only_mode_loads_notes_but_writes_no_journal() {
    let (_dir, engine) = run_with_mode(BrainMode::ReadOnly, true).await;

    assert_ne!(loaded_lines(&engine), Vec::<String>::new());
    assert_eq!(note_types(&engine), vec![NoteType::Convention]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn off_mode_loads_nothing_and_writes_nothing() {
    let (_dir, engine) = run_with_mode(BrainMode::Off, false).await;

    assert_eq!(loaded_lines(&engine), Vec::<String>::new());
    assert_eq!(note_types(&engine), Vec::<NoteType>::new());
}
