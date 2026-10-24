use sentinel_core::{
    Cost, Dependency, DependencyKind, Event, ReservationState, RunId, Task, TaskId, TaskStatus,
    Tier, ToolId,
};
use sentinel_store::{Device, DeviceKind, RunRecord, Store};
use time::{Duration, OffsetDateTime};

fn sample_time() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("valid fixed timestamp")
}

fn sample_task(id: &str) -> Task {
    let mut task = Task::new(TaskId::new(id), format!("title {id}"), "body text");
    task.scope.insert("src/a.rs").expect("valid scope path");
    task.scope.insert("docs/").expect("valid scope path");
    task.flags.already_done = true;
    task.flags.contradicts.push(TaskId::new("T-99"));
    task.tier = Tier::Mid;
    task
}

fn run(id: &str, task: &str, cost: Cost) -> RunRecord {
    RunRecord {
        id: RunId::new(id),
        task: TaskId::new(task),
        tool: ToolId::new("claude"),
        model: "opus".into(),
        started_at: sample_time(),
        ended_at: None,
        exit_code: None,
        cost,
    }
}

fn device(id: &str, token_hash: &str) -> Device {
    Device {
        id: sentinel_core::DeviceId::new(id),
        name: format!("device {id}"),
        kind: DeviceKind::Phone,
        token_hash: token_hash.into(),
        created_at: sample_time(),
        revoked_at: None,
    }
}

#[test]
fn migrations_run_twice_on_the_same_file() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("state.db");
    {
        let store = Store::open(&path).unwrap();
        store.set_setting("theme", "dark").unwrap();
    }
    let store = Store::open(&path).unwrap();
    assert_eq!(store.get_setting("theme").unwrap().as_deref(), Some("dark"));
}

#[test]
fn task_upsert_and_read_back_keeps_every_field() {
    let store = Store::open_in_memory().unwrap();
    let task = sample_task("T-01");
    store.upsert_task(&task).unwrap();
    assert_eq!(store.get_task(&task.id).unwrap(), Some(task));
}

#[test]
fn get_task_returns_none_for_an_unknown_id() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.get_task(&TaskId::new("T-404")).unwrap(), None);
}

#[test]
fn list_tasks_keeps_insertion_order_and_updates_keep_their_place() {
    let store = Store::open_in_memory().unwrap();
    for id in ["T-01", "T-02", "T-03"] {
        store.upsert_task(&sample_task(id)).unwrap();
    }
    let mut changed = sample_task("T-02");
    changed.title = "renamed".into();
    store.upsert_task(&changed).unwrap();

    let tasks = store.list_tasks().unwrap();
    let ids: Vec<_> = tasks.iter().map(|task| task.id.as_str()).collect();
    assert_eq!(ids, ["T-01", "T-02", "T-03"]);
    assert_eq!(tasks[1].title, "renamed");
    assert_eq!(tasks.len(), 3);
}

#[test]
fn set_task_status_accepts_a_legal_move_and_rejects_the_rest() {
    let store = Store::open_in_memory().unwrap();
    let task = sample_task("T-01");
    store.upsert_task(&task).unwrap();

    store.set_task_status(&task.id, TaskStatus::Ready).unwrap();
    let stored = store.get_task(&task.id).unwrap().unwrap();
    assert_eq!(stored.status, TaskStatus::Ready);

    assert!(
        store
            .set_task_status(&task.id, TaskStatus::Verified)
            .is_err()
    );
    assert!(
        store
            .set_task_status(&TaskId::new("T-404"), TaskStatus::Ready)
            .is_err()
    );
    let unchanged = store.get_task(&task.id).unwrap().unwrap();
    assert_eq!(unchanged.status, TaskStatus::Ready);
}

#[test]
fn set_task_tier_changes_the_tier() {
    let store = Store::open_in_memory().unwrap();
    let task = sample_task("T-01");
    store.upsert_task(&task).unwrap();
    store.set_task_tier(&task.id, Tier::Smart).unwrap();
    assert_eq!(store.get_task(&task.id).unwrap().unwrap().tier, Tier::Smart);
}

#[test]
fn increment_attempts_counts_up() {
    let store = Store::open_in_memory().unwrap();
    let task = sample_task("T-01");
    store.upsert_task(&task).unwrap();
    assert_eq!(store.increment_attempts(&task.id).unwrap(), 1);
    assert_eq!(store.increment_attempts(&task.id).unwrap(), 2);
    assert_eq!(store.get_task(&task.id).unwrap().unwrap().attempts, 2);
}

#[test]
fn dependencies_round_trip() {
    let store = Store::open_in_memory().unwrap();
    let deps = [
        Dependency {
            task: TaskId::new("T-02"),
            depends_on: TaskId::new("T-01"),
            kind: DependencyKind::Order,
        },
        Dependency {
            task: TaskId::new("T-03"),
            depends_on: TaskId::new("T-01"),
            kind: DependencyKind::ReadAfterWrite,
        },
    ];
    for dep in &deps {
        store.add_dependency(dep).unwrap();
    }
    assert_eq!(store.list_dependencies().unwrap(), deps);
}

#[test]
fn reservation_moves_from_requested_to_granted_to_released() {
    let store = Store::open_in_memory().unwrap();
    let task = TaskId::new("T-01");
    let placed = store
        .insert_reservation(
            &task,
            "src/a.rs",
            ReservationState::Requested,
            sample_time(),
        )
        .unwrap();
    assert_eq!(placed.state, ReservationState::Requested);
    assert_eq!(placed.released_at, None);

    store
        .set_reservation_state(placed.id, ReservationState::Granted, sample_time())
        .unwrap();
    let released_at = sample_time() + Duration::seconds(30);
    store
        .set_reservation_state(placed.id, ReservationState::Released, released_at)
        .unwrap();

    let stored = store.list_reservations(&[]).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].state, ReservationState::Released);
    assert_eq!(stored[0].released_at, Some(released_at));
}

#[test]
fn reservation_rejects_an_illegal_move() {
    let store = Store::open_in_memory().unwrap();
    let task = TaskId::new("T-01");
    let placed = store
        .insert_reservation(
            &task,
            "src/a.rs",
            ReservationState::Requested,
            sample_time(),
        )
        .unwrap();
    let error = store.set_reservation_state(
        placed.id,
        ReservationState::Released,
        sample_time() + Duration::seconds(1),
    );
    assert!(error.is_err());
    assert_eq!(
        store.list_reservations(&[]).unwrap()[0].state,
        ReservationState::Requested
    );
}

#[test]
fn list_reservations_filters_by_state_and_task() {
    let store = Store::open_in_memory().unwrap();
    let first = TaskId::new("T-01");
    let second = TaskId::new("T-02");
    let granted = store
        .insert_reservation(&first, "src/a.rs", ReservationState::Granted, sample_time())
        .unwrap();
    store
        .insert_reservation(
            &second,
            "src/b.rs",
            ReservationState::Requested,
            sample_time(),
        )
        .unwrap();

    let only_granted = store
        .list_reservations(&[ReservationState::Granted])
        .unwrap();
    assert_eq!(only_granted.len(), 1);
    assert_eq!(only_granted[0].id, granted.id);

    let of_first = store.reservations_of_task(&first).unwrap();
    assert_eq!(of_first.len(), 1);
    assert_eq!(of_first[0].path, "src/a.rs");
}

#[test]
fn runs_sum_cost_and_carry_the_estimated_flag() {
    let store = Store::open_in_memory().unwrap();
    store
        .insert_run(&run("R-01", "T-01", Cost::from_micro_usd(1_000_000, false)))
        .unwrap();
    store
        .insert_run(&run("R-02", "T-02", Cost::from_micro_usd(500_000, true)))
        .unwrap();

    let total = store.total_cost().unwrap();
    assert_eq!(total.micro_usd(), 1_500_000);
    assert!(total.is_estimated());

    let first = store.task_cost(&TaskId::new("T-01")).unwrap();
    assert_eq!(first.micro_usd(), 1_000_000);
    assert!(!first.is_estimated());

    let missing = store.task_cost(&TaskId::new("T-99")).unwrap();
    assert_eq!(missing, Cost::ZERO);
}

#[test]
fn finish_run_records_the_end_and_new_cost() {
    let store = Store::open_in_memory().unwrap();
    store
        .insert_run(&run("R-01", "T-01", Cost::from_micro_usd(100, false)))
        .unwrap();
    let ended_at = sample_time() + Duration::minutes(2);
    store
        .finish_run(
            &RunId::new("R-01"),
            ended_at,
            0,
            Cost::from_micro_usd(2_500, true),
        )
        .unwrap();

    let stored = store.list_runs().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].ended_at, Some(ended_at));
    assert_eq!(stored[0].exit_code, Some(0));
    assert_eq!(stored[0].cost, Cost::from_micro_usd(2_500, true));
}

#[test]
fn events_get_increasing_ids_and_page_in_order() {
    let store = Store::open_in_memory().unwrap();
    let first = store
        .append_event(
            &Event::TaskStatus {
                task: TaskId::new("T-01"),
                status: TaskStatus::Ready,
            },
            sample_time(),
        )
        .unwrap();
    let second = store
        .append_event(&Event::HostHeartbeat, sample_time())
        .unwrap();
    assert!(second.id > first.id);

    let page = store.events_after(first.id - 1, 10).unwrap();
    assert_eq!(page, vec![first.clone(), second.clone()]);

    let tail = store.events_after(first.id, 10).unwrap();
    assert_eq!(tail, vec![second]);
}

#[test]
fn devices_are_found_by_hash_until_revoked() {
    let store = Store::open_in_memory().unwrap();
    let phone = device("D-01", "hash-a");
    store.insert_device(&phone).unwrap();
    store.insert_device(&device("D-02", "hash-b")).unwrap();

    assert_eq!(
        store.device_by_token_hash("hash-a").unwrap(),
        Some(phone.clone())
    );

    let revoked_at = sample_time() + Duration::hours(1);
    store.revoke_device(&phone.id, revoked_at).unwrap();
    assert_eq!(store.device_by_token_hash("hash-a").unwrap(), None);

    let listed = store.list_devices().unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].revoked_at, Some(revoked_at));
    assert_eq!(listed[1].revoked_at, None);
}

#[test]
fn settings_overwrite_and_missing_keys_are_none() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.get_setting("cap").unwrap(), None);
    store.set_setting("cap", "5.00").unwrap();
    assert_eq!(store.get_setting("cap").unwrap().as_deref(), Some("5.00"));
    store.set_setting("cap", "2.00").unwrap();
    assert_eq!(store.get_setting("cap").unwrap().as_deref(), Some("2.00"));
}

#[test]
fn file_store_keeps_data_across_reopen() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("state.db");
    let task = sample_task("T-07");
    {
        let store = Store::open(&path).unwrap();
        store.upsert_task(&task).unwrap();
    }
    let reopened = Store::open(&path).unwrap();
    assert_eq!(reopened.get_task(&task.id).unwrap(), Some(task));
}

#[test]
fn last_event_id_and_recent_stream_events_skip_agent_output() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.last_event_id().unwrap(), 0);
    let at = time::OffsetDateTime::UNIX_EPOCH;
    store
        .append_event(&sentinel_core::Event::BarrierStart { wave: 1 }, at)
        .unwrap();
    store
        .append_event(
            &sentinel_core::Event::AgentOutput {
                task: sentinel_core::TaskId::new("T-01"),
                agent: sentinel_core::ToolId::new("a"),
                line: "x".into(),
            },
            at,
        )
        .unwrap();
    store
        .append_event(
            &sentinel_core::Event::BarrierEnd {
                wave: 1,
                passed: true,
            },
            at,
        )
        .unwrap();
    assert_eq!(store.last_event_id().unwrap(), 3);
    let recent = store.recent_stream_events(10).unwrap();
    let kinds: Vec<_> = recent.iter().map(|r| r.event.kind()).collect();
    assert_eq!(kinds, ["BarrierStart", "BarrierEnd"]);
}
