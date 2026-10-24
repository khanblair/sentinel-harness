//! Routines end to end: the two built-in routines, who may change what, the input rules, and the
//! brief that a send-now stores. Built on a real engine over a sample repository.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use sentinel_core::routines::{EVERY_DAY, EditRoutine, NewRoutine, Routine, RoutineKind, WEEKDAYS};
use sentinel_core::{RoutineId, Task, TaskId, TaskStatus};
use sentinel_daemon::controls::Caller;
use sentinel_daemon::engine::{Engine, EngineError};

mod common;
use common::{fake_engine, sample_repo};

fn computer() -> Caller {
    Caller::pc("You")
}

fn phone() -> Caller {
    Caller::phone("Blair phone")
}

fn new_reminder(text: &str) -> NewRoutine {
    NewRoutine {
        name: "Stand up".to_owned(),
        kind: RoutineKind::Reminder,
        minute_of_day: 9 * 60,
        days: WEEKDAYS,
        to_telegram: false,
        text: text.to_owned(),
        backlog: String::new(),
    }
}

fn built_in(engine: &Engine, id: &str) -> Routine {
    let routines = engine.list_routines().unwrap();
    routines
        .into_iter()
        .find(|routine| routine.id == RoutineId::new(id))
        .unwrap()
}

fn turn_off() -> EditRoutine {
    EditRoutine {
        enabled: Some(false),
        ..EditRoutine::default()
    }
}

fn turn_on() -> EditRoutine {
    EditRoutine {
        enabled: Some(true),
        ..EditRoutine::default()
    }
}

#[test]
fn the_two_built_in_routines_start_off_and_send_to_telegram() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    let morning = built_in(&engine, "morning");
    let sundown = built_in(&engine, "sundown");
    assert_eq!(engine.list_routines().unwrap().len(), 2);
    assert_eq!(morning.kind, RoutineKind::Morning);
    assert_eq!((morning.minute_of_day, morning.days), (8 * 60, WEEKDAYS));
    assert_eq!((sundown.minute_of_day, sundown.days), (18 * 60, EVERY_DAY));
    assert!(!morning.enabled && !sundown.enabled);
    assert!(morning.to_telegram && sundown.to_telegram);
}

#[test]
fn a_phone_cannot_create_or_delete_a_routine() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    let created = engine.create_routine(&new_reminder("Water"), &phone());
    assert!(matches!(created, Err(EngineError::Refused(_))));
    let deleted = engine.delete_routine(&RoutineId::new("morning"), &phone());
    assert!(matches!(deleted, Err(EngineError::Refused(_))));
}

#[test]
fn a_built_in_routine_cannot_be_deleted_even_by_the_computer() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    let deleted = engine.delete_routine(&RoutineId::new("sundown"), &computer());
    assert!(matches!(deleted, Err(EngineError::Refused(_))));
    assert_eq!(engine.list_routines().unwrap().len(), 2);
}

#[test]
fn a_phone_may_turn_a_routine_off_but_not_on_or_rename_it() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    let morning = RoutineId::new("morning");
    engine
        .edit_routine(&morning, turn_on(), &computer())
        .unwrap();
    let off = engine.edit_routine(&morning, turn_off(), &phone()).unwrap();
    assert!(!off.enabled);
    let on = engine.edit_routine(&morning, turn_on(), &phone());
    assert!(matches!(on, Err(EngineError::Refused(_))));
    let renamed = EditRoutine {
        name: Some("Early".to_owned()),
        ..EditRoutine::default()
    };
    let refused = engine.edit_routine(&morning, renamed, &phone());
    assert!(matches!(refused, Err(EngineError::Refused(_))));
}

#[tokio::test]
async fn a_phone_sends_a_brief_now_and_the_day_is_not_used_up() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    engine
        .send_routine_now(&RoutineId::new("morning"), &phone())
        .await
        .unwrap();
    let briefs = engine.store().list_briefs(14).unwrap();
    assert_eq!(briefs.len(), 1);
    assert_eq!(briefs[0].kind, RoutineKind::Morning);
    assert_eq!(built_in(&engine, "morning").last_fired_on, None);
}

#[tokio::test]
async fn hackathon_mode_keeps_titles_out_of_a_sent_brief_but_keeps_ids_and_reasons() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    engine.update_config(|c| c.hackathon_mode = true).unwrap();
    let mut blocked = Task::new(TaskId::new("T-09"), "Secret feature", "");
    blocked.status = TaskStatus::Blocked;
    engine.store().upsert_task(&blocked).unwrap();
    engine
        .send_routine_now(&RoutineId::new("sundown"), &phone())
        .await
        .unwrap();
    let text = engine.store().list_briefs(1).unwrap()[0].body.clone();
    assert!(text.contains("T-09"), "{text}");
    assert!(!text.contains("Secret feature"), "{text}");
    assert!(
        text.contains("The run ended before this task finished."),
        "{text}"
    );
}

#[test]
fn a_routine_with_bad_input_is_refused_before_it_is_saved() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    let mut empty_name = new_reminder("Water");
    empty_name.name = String::new();
    let mut late = new_reminder("Water");
    late.minute_of_day = 1440;
    let mut no_days = new_reminder("Water");
    no_days.days = 0;
    let mut no_text = new_reminder("");
    no_text.text.clear();
    let mut long_name = new_reminder("Water");
    long_name.name = "x".repeat(61);
    for bad in [empty_name, late, no_days, no_text, long_name] {
        let result = engine.create_routine(&bad, &computer());
        assert!(matches!(result, Err(EngineError::Invalid(_))), "{bad:?}");
    }
    assert_eq!(engine.list_routines().unwrap().len(), 2);
}

#[test]
fn the_computer_makes_edits_and_deletes_a_made_routine() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    let made = engine
        .create_routine(&new_reminder("Water"), &computer())
        .unwrap();
    assert!(made.id.as_str().starts_with("r-"));
    assert!(made.enabled);
    let renamed = EditRoutine {
        name: Some("Break".to_owned()),
        ..EditRoutine::default()
    };
    let edited = engine.edit_routine(&made.id, renamed, &computer()).unwrap();
    assert_eq!(edited.name, "Break");
    engine.delete_routine(&made.id, &computer()).unwrap();
    assert_eq!(engine.list_routines().unwrap().len(), 2);
}
