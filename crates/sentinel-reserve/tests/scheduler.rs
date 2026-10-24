//! Scheduler behaviour against a real in-memory store.

use sentinel_core::{Dependency, DependencyKind, ReservationState, Task, TaskId, TaskStatus};
use sentinel_reserve::{Grant, ReservationTable, Scheduler, Wave};
use sentinel_store::Store;
use time::OffsetDateTime;

fn now() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
}

fn fresh_store() -> Store {
    Store::open_in_memory().expect("test setup")
}

fn id(name: &str) -> TaskId {
    TaskId::new(name)
}

fn build_task(name: &str, paths: &[&str], status: TaskStatus) -> Task {
    let mut task = Task::new(id(name), name, name);
    task.status = status;
    for path in paths {
        task.scope.insert(path).expect("test setup");
    }
    task
}

fn seed(store: &Store, name: &str, paths: &[&str], status: TaskStatus) {
    store
        .upsert_task(&build_task(name, paths, status))
        .expect("test setup");
}

fn seed_ready(store: &Store, name: &str, paths: &[&str]) {
    seed(store, name, paths, TaskStatus::Ready);
}

fn depend(store: &Store, task: &str, on: &str) {
    store
        .add_dependency(&Dependency {
            task: id(task),
            depends_on: id(on),
            kind: DependencyKind::Order,
        })
        .expect("test setup");
}

fn status_of(store: &Store, name: &str) -> TaskStatus {
    store
        .get_task(&id(name))
        .expect("test setup")
        .expect("test setup")
        .status
}

fn assert_wave(wave: &Wave, expected: &[&str]) {
    let expected: Vec<TaskId> = expected.iter().map(|name| id(name)).collect();
    assert_eq!(wave.tasks, expected, "wave {}", wave.number);
}

fn assert_in_step(store: &Store, scheduler: &Scheduler) {
    let reloaded = ReservationTable::load(store.clone()).expect("test setup");
    let live = scheduler.table();
    assert_eq!(reloaded.all_held(), live.all_held(), "held rows drift");
    for name in ["A", "B", "C", "D"] {
        assert_eq!(
            reloaded.blockers_of(&id(name)),
            live.blockers_of(&id(name)),
            "waiting rows drift for {name}"
        );
    }
}

fn reserve_and_start(scheduler: &mut Scheduler, store: &Store, name: &str) {
    let wave = scheduler.next_wave(1, 10, now()).expect("test setup");
    assert!(wave.tasks.contains(&id(name)), "wave must hold {name}");
    scheduler.start_task(&id(name)).expect("test setup");
    assert_eq!(status_of(store, name), TaskStatus::Running);
}

#[test]
fn a_dependent_task_starts_only_after_its_dependency_checkpoints() {
    let store = fresh_store();
    seed_ready(&store, "A", &["src/a.rs"]);
    seed_ready(&store, "B", &["src/b.rs"]);
    depend(&store, "B", "A");
    let mut scheduler = Scheduler::new(store.clone()).expect("test setup");

    let first = scheduler.next_wave(1, 10, now()).expect("test setup");
    assert_wave(&first, &["A"]);

    scheduler.start_task(&id("A")).expect("test setup");
    assert_eq!(status_of(&store, "A"), TaskStatus::Running);
    scheduler
        .on_checkpoint(&id("A"), now())
        .expect("test setup");
    assert_eq!(status_of(&store, "A"), TaskStatus::Checkpointed);
    assert_eq!(status_of(&store, "B"), TaskStatus::Ready);

    let second = scheduler.next_wave(2, 10, now()).expect("test setup");
    assert_wave(&second, &["B"]);
}

#[test]
fn disjoint_tasks_share_a_wave_and_an_overlap_waits_for_the_next_one() {
    let store = fresh_store();
    seed_ready(&store, "A", &["src/a.rs"]);
    seed_ready(&store, "B", &["src/b.rs"]);
    seed_ready(&store, "C", &["src/a.rs"]);
    let mut scheduler = Scheduler::new(store.clone()).expect("test setup");

    let first = scheduler.next_wave(1, 10, now()).expect("test setup");
    assert_wave(&first, &["A", "B"]);

    let blocked = scheduler.next_wave(2, 10, now()).expect("test setup");
    assert_wave(&blocked, &[]);
    assert_eq!(status_of(&store, "C"), TaskStatus::Ready);

    scheduler.start_task(&id("A")).expect("test setup");
    scheduler
        .on_checkpoint(&id("A"), now())
        .expect("test setup");

    let third = scheduler.next_wave(3, 10, now()).expect("test setup");
    assert_wave(&third, &["C"]);
}

#[test]
fn max_tasks_caps_the_wave() {
    let store = fresh_store();
    seed_ready(&store, "A", &["src/a.rs"]);
    seed_ready(&store, "B", &["src/b.rs"]);
    seed_ready(&store, "C", &["src/c.rs"]);
    let mut scheduler = Scheduler::new(store.clone()).expect("test setup");

    let wave = scheduler.next_wave(1, 2, now()).expect("test setup");
    assert_wave(&wave, &["A", "B"]);
    assert_eq!(status_of(&store, "C"), TaskStatus::Ready);

    let none = scheduler.next_wave(2, 0, now()).expect("test setup");
    assert_wave(&none, &[]);
}

#[test]
fn a_wave_reserves_the_files_of_its_tasks() {
    let store = fresh_store();
    seed_ready(&store, "A", &["src/a.rs"]);
    seed_ready(&store, "B", &["src/b.rs"]);
    let mut scheduler = Scheduler::new(store.clone()).expect("test setup");

    scheduler.next_wave(1, 10, now()).expect("test setup");

    assert_eq!(status_of(&store, "A"), TaskStatus::Reserved);
    assert!(scheduler.table().may_write(&id("A"), "src/a.rs"));
    assert!(!scheduler.table().may_write(&id("A"), "src/b.rs"));
    assert!(scheduler.table().may_write(&id("B"), "src/b.rs"));
    assert_in_step(&store, &scheduler);
}

#[test]
fn a_checkpoint_releases_the_files_for_the_next_wave() {
    let store = fresh_store();
    seed_ready(&store, "A", &["src/a.rs"]);
    seed_ready(&store, "B", &["src/a.rs"]);
    let mut scheduler = Scheduler::new(store.clone()).expect("test setup");

    scheduler.next_wave(1, 10, now()).expect("test setup");
    scheduler.start_task(&id("A")).expect("test setup");
    assert!(scheduler.table().may_write(&id("A"), "src/a.rs"));

    scheduler
        .on_checkpoint(&id("A"), now())
        .expect("test setup");
    assert!(!scheduler.table().may_write(&id("A"), "src/a.rs"));

    let next = scheduler.next_wave(2, 10, now()).expect("test setup");
    assert_wave(&next, &["B"]);
    assert!(scheduler.table().may_write(&id("B"), "src/a.rs"));
    assert_in_step(&store, &scheduler);
}

#[test]
fn two_failures_retry_and_the_third_blocks() {
    let store = fresh_store();
    seed_ready(&store, "A", &["src/a.rs"]);
    let mut scheduler = Scheduler::new(store.clone()).expect("test setup");

    for attempt in 1..=2 {
        reserve_and_start(&mut scheduler, &store, "A");
        let next = scheduler.on_failure(&id("A"), now()).expect("test setup");
        assert_eq!(next, TaskStatus::Ready, "attempt {attempt}");
        assert_eq!(status_of(&store, "A"), TaskStatus::Ready);
        assert!(!scheduler.table().may_write(&id("A"), "src/a.rs"));
        let rows = store.reservations_of_task(&id("A")).expect("test setup");
        assert!(
            rows.iter()
                .all(|row| row.state == ReservationState::Released)
        );
    }

    reserve_and_start(&mut scheduler, &store, "A");
    let final_status = scheduler.on_failure(&id("A"), now()).expect("test setup");
    assert_eq!(final_status, TaskStatus::Blocked);
    assert_eq!(status_of(&store, "A"), TaskStatus::Blocked);
    let rows = store.reservations_of_task(&id("A")).expect("test setup");
    assert!(
        rows.iter()
            .all(|row| row.state == ReservationState::Released)
    );
    assert_in_step(&store, &scheduler);
}

#[test]
fn a_retry_limit_of_one_blocks_on_the_first_failure() {
    let store = fresh_store();
    seed_ready(&store, "A", &["src/a.rs"]);
    let mut scheduler = Scheduler::new(store.clone())
        .expect("test setup")
        .with_retry_limit(1);

    reserve_and_start(&mut scheduler, &store, "A");
    let next = scheduler.on_failure(&id("A"), now()).expect("test setup");

    assert_eq!(next, TaskStatus::Blocked);
    assert_eq!(status_of(&store, "A"), TaskStatus::Blocked);
    let rows = store.reservations_of_task(&id("A")).expect("test setup");
    assert!(
        rows.iter()
            .all(|row| row.state == ReservationState::Released)
    );
}

#[test]
fn request_files_grants_a_free_path_and_waits_for_a_held_one() {
    let store = fresh_store();
    seed_ready(&store, "A", &["src/a.rs"]);
    seed_ready(&store, "B", &["src/b.rs"]);
    let mut scheduler = Scheduler::new(store.clone()).expect("test setup");
    scheduler.next_wave(1, 10, now()).expect("test setup");

    let granted = scheduler
        .request_files(&id("A"), "src/free.rs", now())
        .expect("test setup");
    assert!(matches!(granted, Grant::Granted(_)));
    assert!(scheduler.table().may_write(&id("A"), "src/free.rs"));
    assert_eq!(status_of(&store, "A"), TaskStatus::Reserved);

    let waiting = scheduler
        .request_files(&id("A"), "src/b.rs", now())
        .expect("test setup");
    match waiting {
        Grant::Waiting { blocked_by } => assert_eq!(blocked_by, vec![id("B")]),
        other @ Grant::Granted(_) => panic!("expected waiting, got {other:?}"),
    }
    assert_eq!(status_of(&store, "A"), TaskStatus::Reserved);
    assert_in_step(&store, &scheduler);
}

#[test]
fn recover_after_crash_requeues_reserved_and_running_tasks() {
    let store = fresh_store();
    seed_ready(&store, "A", &["src/a.rs"]);
    seed_ready(&store, "B", &["src/b.rs"]);
    seed(&store, "V", &["src/v.rs"], TaskStatus::Verified);
    let mut scheduler = Scheduler::new(store.clone()).expect("test setup");

    let wave = scheduler.next_wave(1, 10, now()).expect("test setup");
    assert_wave(&wave, &["A", "B"]);
    scheduler.start_task(&id("A")).expect("test setup");

    let requeued = scheduler.recover_after_crash(now()).expect("test setup");

    assert_eq!(requeued, vec![id("A"), id("B")]);
    assert_eq!(status_of(&store, "A"), TaskStatus::Ready);
    assert_eq!(status_of(&store, "B"), TaskStatus::Ready);
    assert_eq!(status_of(&store, "V"), TaskStatus::Verified);
    assert_eq!(scheduler.table().all_held(), Vec::new());
    for name in ["A", "B"] {
        let rows = store.reservations_of_task(&id(name)).expect("test setup");
        assert!(
            rows.iter()
                .all(|row| row.state == ReservationState::Revoked),
            "{name} rows"
        );
    }
    assert_in_step(&store, &scheduler);
}

#[test]
fn unrunnable_tasks_lists_a_ready_task_whose_dependency_is_blocked() {
    let store = fresh_store();
    seed(&store, "A", &["src/a.rs"], TaskStatus::Blocked);
    seed_ready(&store, "B", &["src/b.rs"]);
    seed_ready(&store, "C", &["src/c.rs"]);
    depend(&store, "B", "A");
    let scheduler = Scheduler::new(store.clone()).expect("test setup");

    assert_eq!(
        scheduler.unrunnable_tasks().expect("test setup"),
        vec![id("B")]
    );
    let ready = scheduler.ready_tasks().expect("test setup");
    assert_eq!(ready.len(), 1, "only C can run");
    assert_eq!(ready[0].id, id("C"));
}

#[test]
fn the_store_view_equals_the_live_table_through_reservations_and_checkpoints() {
    let store = fresh_store();
    seed_ready(&store, "A", &["src/a.rs"]);
    seed_ready(&store, "B", &["src/b.rs"]);
    seed_ready(&store, "C", &["src/a.rs"]);
    let mut scheduler = Scheduler::new(store.clone()).expect("test setup");

    let first = scheduler.next_wave(1, 2, now()).expect("test setup");
    assert_wave(&first, &["A", "B"]);
    assert_in_step(&store, &scheduler);

    scheduler.start_task(&id("A")).expect("test setup");
    assert_in_step(&store, &scheduler);

    scheduler
        .on_checkpoint(&id("A"), now())
        .expect("test setup");
    assert_in_step(&store, &scheduler);

    let second = scheduler.next_wave(2, 2, now()).expect("test setup");
    assert_wave(&second, &["C"]);
    assert_in_step(&store, &scheduler);

    scheduler.start_task(&id("C")).expect("test setup");
    assert_in_step(&store, &scheduler);

    scheduler
        .on_checkpoint(&id("C"), now())
        .expect("test setup");
    assert_in_step(&store, &scheduler);
}

#[test]
fn the_store_view_equals_the_live_table_through_extension_failure_and_crash() {
    let store = fresh_store();
    seed_ready(&store, "A", &["src/a.rs"]);
    seed_ready(&store, "B", &["src/b.rs"]);
    seed_ready(&store, "C", &["src/a.rs"]);
    let mut scheduler = Scheduler::new(store.clone()).expect("test setup");

    let first = scheduler.next_wave(1, 2, now()).expect("test setup");
    assert_wave(&first, &["A", "B"]);
    assert_in_step(&store, &scheduler);

    let free = scheduler
        .request_files(&id("A"), "src/c.rs", now())
        .expect("test setup");
    assert!(matches!(free, Grant::Granted(_)));
    assert_in_step(&store, &scheduler);

    let held = scheduler
        .request_files(&id("A"), "src/b.rs", now())
        .expect("test setup");
    assert!(matches!(held, Grant::Waiting { .. }));
    assert_in_step(&store, &scheduler);

    let next = scheduler.on_failure(&id("A"), now()).expect("test setup");
    assert_eq!(next, TaskStatus::Ready);
    assert_in_step(&store, &scheduler);

    let second = scheduler.next_wave(2, 2, now()).expect("test setup");
    assert_wave(&second, &["A"]);
    assert_in_step(&store, &scheduler);

    let requeued = scheduler.recover_after_crash(now()).expect("test setup");
    assert_eq!(requeued, vec![id("A"), id("B")]);
    assert_in_step(&store, &scheduler);
}
