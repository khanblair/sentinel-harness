use sentinel_core::{
    Approval, ApprovalId, ApprovalKind, ApprovalState, Capability, Decision, DecisionId,
    DecisionState, QualityKind, QualityNote, TaskId, TaskStatus, Tier, ToolId, ToolInfo,
    ToolStatus,
};
use sentinel_store::{CheckpointRecord, Store, VerificationRecord};
use time::{Duration, OffsetDateTime};

fn sample_time() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("valid fixed timestamp")
}

fn sample_approval(id: &str, kind: ApprovalKind) -> Approval {
    Approval {
        id: ApprovalId::new(id),
        kind,
        task: Some(TaskId::new("T-01")),
        agent: Some(ToolId::new("claude")),
        path: if kind == ApprovalKind::Extension {
            Some("src/main.rs".to_owned())
        } else {
            None
        },
        state: ApprovalState::Pending,
        decided_by: None,
    }
}

fn sample_decision(id: &str) -> Decision {
    Decision {
        id: DecisionId::new(id),
        prompt: "Choose one".to_owned(),
        tasks: vec![TaskId::new("T-01"), TaskId::new("T-02")],
        state: DecisionState::Pending,
        answer: None,
    }
}

fn sample_tool(id: &str) -> ToolInfo {
    ToolInfo {
        id: ToolId::new(id),
        name: id.to_owned(),
        path: format!("/usr/bin/{id}"),
        version: "1.0.0".to_owned(),
        adapter: "generic".to_owned(),
        status: ToolStatus::Ready,
        tier: Tier::Mid,
        enabled: true,
    }
}

#[test]
fn approval_round_trip() {
    let store = Store::open_in_memory().unwrap();
    let approval = sample_approval("A-01", ApprovalKind::Extension);
    store.insert_approval(&approval, sample_time()).unwrap();
    let fetched = store
        .get_approval(&ApprovalId::new("A-01"))
        .unwrap()
        .unwrap();
    assert_eq!(fetched.id.as_str(), "A-01");
    assert_eq!(fetched.kind, ApprovalKind::Extension);
    assert_eq!(fetched.path.as_deref(), Some("src/main.rs"));
}

#[test]
fn approval_answer_once_then_error() {
    let store = Store::open_in_memory().unwrap();
    let approval = sample_approval("A-02", ApprovalKind::Wave);
    store.insert_approval(&approval, sample_time()).unwrap();
    let answered = store
        .answer_approval(
            &ApprovalId::new("A-02"),
            ApprovalState::Allowed,
            "phone",
            sample_time(),
        )
        .unwrap();
    assert_eq!(answered.state, ApprovalState::Allowed);
    assert_eq!(answered.decided_by.as_deref(), Some("phone"));
    assert!(
        store
            .answer_approval(
                &ApprovalId::new("A-02"),
                ApprovalState::Denied,
                "phone",
                sample_time()
            )
            .is_err()
    );
}

#[test]
fn approval_cannot_answer_to_pending() {
    let store = Store::open_in_memory().unwrap();
    let approval = sample_approval("A-03", ApprovalKind::Plan);
    store.insert_approval(&approval, sample_time()).unwrap();
    assert!(
        store
            .answer_approval(
                &ApprovalId::new("A-03"),
                ApprovalState::Pending,
                "phone",
                sample_time()
            )
            .is_err()
    );
}

#[test]
fn list_approvals_filters_and_orders() {
    let store = Store::open_in_memory().unwrap();
    let t = sample_time();
    store
        .insert_approval(&sample_approval("A-10", ApprovalKind::Extension), t)
        .unwrap();
    store
        .insert_approval(
            &sample_approval("A-11", ApprovalKind::Wave),
            t + Duration::seconds(1),
        )
        .unwrap();
    store
        .insert_approval(
            &sample_approval("A-12", ApprovalKind::Plan),
            t + Duration::seconds(2),
        )
        .unwrap();
    let all = store.list_approvals(&[]).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].id.as_str(), "A-12");
    assert_eq!(all[1].id.as_str(), "A-11");
    assert_eq!(all[2].id.as_str(), "A-10");
    let only_pending = store.list_approvals(&[ApprovalState::Pending]).unwrap();
    assert_eq!(only_pending.len(), 3);
}

#[test]
fn decision_round_trip() {
    let store = Store::open_in_memory().unwrap();
    let decision = sample_decision("D-01");
    store.insert_decision(&decision, sample_time()).unwrap();
    let fetched = store.list_decisions(&[]).unwrap();
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].id.as_str(), "D-01");
    assert_eq!(fetched[0].tasks.len(), 2);
}

#[test]
fn decision_answer() {
    let store = Store::open_in_memory().unwrap();
    let decision = sample_decision("D-02");
    store.insert_decision(&decision, sample_time()).unwrap();
    let answered = store
        .answer_decision(
            &DecisionId::new("D-02"),
            DecisionState::Answered,
            Some("T-01"),
            "human",
            sample_time(),
        )
        .unwrap();
    assert_eq!(answered.state, DecisionState::Answered);
    assert_eq!(answered.answer.as_deref(), Some("T-01"));
}

#[test]
fn decision_cannot_answer_twice() {
    let store = Store::open_in_memory().unwrap();
    let decision = sample_decision("D-03");
    store.insert_decision(&decision, sample_time()).unwrap();
    store
        .answer_decision(
            &DecisionId::new("D-03"),
            DecisionState::Answered,
            Some("T-01"),
            "human",
            sample_time(),
        )
        .unwrap();
    assert!(
        store
            .answer_decision(
                &DecisionId::new("D-03"),
                DecisionState::Defaulted,
                Some("skip_both"),
                "default",
                sample_time(),
            )
            .is_err()
    );
}

#[test]
fn list_decisions_filters_and_orders() {
    let store = Store::open_in_memory().unwrap();
    let t = sample_time();
    store.insert_decision(&sample_decision("D-10"), t).unwrap();
    store
        .insert_decision(&sample_decision("D-11"), t + Duration::seconds(1))
        .unwrap();
    let all = store.list_decisions(&[]).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].id.as_str(), "D-10");
    assert_eq!(all[1].id.as_str(), "D-11");
    let only_pending = store.list_decisions(&[DecisionState::Pending]).unwrap();
    assert_eq!(only_pending.len(), 2);
}

#[test]
fn checkpoint_latest_per_task() {
    let store = Store::open_in_memory().unwrap();
    let task = TaskId::new("T-01");
    let first = CheckpointRecord {
        task: task.clone(),
        commit: "aaa".to_owned(),
        paths: vec!["src/a.rs".to_owned()],
        at: sample_time(),
    };
    store.insert_checkpoint(&first).unwrap();
    let second = CheckpointRecord {
        task: task.clone(),
        commit: "bbb".to_owned(),
        paths: vec!["src/b.rs".to_owned()],
        at: sample_time() + Duration::seconds(1),
    };
    store.insert_checkpoint(&second).unwrap();
    let latest = store.latest_checkpoint(&task).unwrap().unwrap();
    assert_eq!(latest.commit, "bbb");
    assert_eq!(latest.paths, vec!["src/b.rs"]);
    let missing = store.latest_checkpoint(&TaskId::new("T-99")).unwrap();
    assert_eq!(missing, None);
}

#[test]
fn list_checkpoints_oldest_first() {
    let store = Store::open_in_memory().unwrap();
    let task = TaskId::new("T-01");
    let first = CheckpointRecord {
        task: task.clone(),
        commit: "aaa".to_owned(),
        paths: vec!["src/a.rs".to_owned()],
        at: sample_time(),
    };
    store.insert_checkpoint(&first).unwrap();
    let second = CheckpointRecord {
        task: task.clone(),
        commit: "bbb".to_owned(),
        paths: vec!["src/b.rs".to_owned()],
        at: sample_time() + Duration::seconds(1),
    };
    store.insert_checkpoint(&second).unwrap();
    let list = store.list_checkpoints().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].commit, "aaa");
    assert_eq!(list[1].commit, "bbb");
}

#[test]
fn barrier_start_and_finish() {
    let store = Store::open_in_memory().unwrap();
    let id = store.start_barrier(1, sample_time()).unwrap();
    assert!(id > 0);
    store
        .finish_barrier(
            id,
            sample_time() + Duration::minutes(5),
            true,
            "suite-output.txt",
        )
        .unwrap();
    let barriers = store.list_barriers().unwrap();
    assert_eq!(barriers.len(), 1);
    assert_eq!(barriers[0].id, id);
    assert_eq!(barriers[0].wave, 1);
    assert_eq!(barriers[0].passed, Some(true));
    assert_eq!(barriers[0].output_ref.as_deref(), Some("suite-output.txt"));
    assert!(barriers[0].ended_at.is_some());
}

#[test]
fn list_barriers_oldest_first() {
    let store = Store::open_in_memory().unwrap();
    let id1 = store.start_barrier(1, sample_time()).unwrap();
    let id2 = store
        .start_barrier(2, sample_time() + Duration::minutes(10))
        .unwrap();
    store
        .finish_barrier(id1, sample_time() + Duration::minutes(5), true, "out1.txt")
        .unwrap();
    let barriers = store.list_barriers().unwrap();
    assert_eq!(barriers.len(), 2);
    assert_eq!(barriers[0].id, id1);
    assert_eq!(barriers[1].id, id2);
}

#[test]
fn verification_upsert_replaces_and_verified_needs_all_checks() {
    let store = Store::open_in_memory().unwrap();
    let task = TaskId::new("T-01");
    let record = VerificationRecord {
        task: task.clone(),
        tests_pass: true,
        new_test: true,
        reviewer_ok: true,
        tamper_clear: true,
        verdict: TaskStatus::Verified,
        reason: "all good".to_owned(),
        receipt_json: "{}".to_owned(),
    };
    store.upsert_verification(&record).unwrap();
    let fetched = store.get_verification(&task).unwrap().unwrap();
    assert_eq!(fetched.verdict, TaskStatus::Verified);
    let mut incomplete = record.clone();
    incomplete.tamper_clear = false;
    incomplete.verdict = TaskStatus::Verified;
    assert!(store.upsert_verification(&incomplete).is_err());
    incomplete.verdict = TaskStatus::Failed;
    store.upsert_verification(&incomplete).unwrap();
    let replaced = store.get_verification(&task).unwrap().unwrap();
    assert_eq!(replaced.verdict, TaskStatus::Failed);
    assert!(!replaced.tamper_clear);
}

#[test]
fn list_verifications_oldest_first() {
    let store = Store::open_in_memory().unwrap();
    let task1 = TaskId::new("T-01");
    let task2 = TaskId::new("T-02");
    let r1 = VerificationRecord {
        task: task1,
        tests_pass: true,
        new_test: true,
        reviewer_ok: true,
        tamper_clear: true,
        verdict: TaskStatus::Verified,
        reason: String::new(),
        receipt_json: "{}".to_owned(),
    };
    let r2 = VerificationRecord {
        task: task2,
        tests_pass: false,
        new_test: false,
        reviewer_ok: false,
        tamper_clear: false,
        verdict: TaskStatus::Failed,
        reason: "failed".to_owned(),
        receipt_json: "{}".to_owned(),
    };
    store.upsert_verification(&r1).unwrap();
    store.upsert_verification(&r2).unwrap();
    let list = store.list_verifications().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].task.as_str(), "T-01");
    assert_eq!(list[1].task.as_str(), "T-02");
}

#[test]
fn quality_notes_by_task_and_all() {
    let store = Store::open_in_memory().unwrap();
    let task1 = TaskId::new("T-01");
    let task2 = TaskId::new("T-02");
    let note1 = QualityNote {
        task: task1.clone(),
        kind: QualityKind::Size,
        location: "src/a.rs:10".to_owned(),
        detail_code: "long_function".to_owned(),
    };
    let note2 = QualityNote {
        task: task1.clone(),
        kind: QualityKind::Slop,
        location: "src/b.rs:5".to_owned(),
        detail_code: "placeholder_comment".to_owned(),
    };
    let note3 = QualityNote {
        task: task2.clone(),
        kind: QualityKind::Duplication,
        location: "src/c.rs:20".to_owned(),
        detail_code: "copied_block".to_owned(),
    };
    store.insert_quality_note(&note1, sample_time()).unwrap();
    store
        .insert_quality_note(&note2, sample_time() + Duration::seconds(1))
        .unwrap();
    store
        .insert_quality_note(&note3, sample_time() + Duration::seconds(2))
        .unwrap();
    let for_task1 = store.list_quality_notes(Some(&task1)).unwrap();
    assert_eq!(for_task1.len(), 2);
    assert_eq!(for_task1[0].detail_code, "long_function");
    assert_eq!(for_task1[1].detail_code, "placeholder_comment");
    let all = store.list_quality_notes(None).unwrap();
    assert_eq!(all.len(), 3);
}

#[test]
fn upsert_tool_keeps_enabled_across_rescan() {
    let store = Store::open_in_memory().unwrap();
    let mut tool = sample_tool("claude");
    tool.enabled = true;
    store.upsert_tool(&tool).unwrap();
    store
        .set_tool_enabled(&ToolId::new("claude"), false)
        .unwrap();
    tool.enabled = true;
    store.upsert_tool(&tool).unwrap();
    let listed = store.list_tools().unwrap();
    assert_eq!(listed.len(), 1);
    assert!(!listed[0].enabled);
    let mut tool2 = sample_tool("claude");
    tool2.enabled = false;
    store.upsert_tool(&tool2).unwrap();
    let listed = store.list_tools().unwrap();
    assert!(!listed[0].enabled);
}

#[test]
fn set_tool_enabled_changes_flag() {
    let store = Store::open_in_memory().unwrap();
    let tool = sample_tool("codex");
    store.upsert_tool(&tool).unwrap();
    store
        .set_tool_enabled(&ToolId::new("codex"), false)
        .unwrap();
    let listed = store.list_tools().unwrap();
    assert!(!listed[0].enabled);
    store.set_tool_enabled(&ToolId::new("codex"), true).unwrap();
    let listed = store.list_tools().unwrap();
    assert!(listed[0].enabled);
}

#[test]
fn capabilities_grouped_by_agent_sorted_and_ordered() {
    let store = Store::open_in_memory().unwrap();
    let claude = ToolId::new("claude");
    let codex = ToolId::new("codex");
    store
        .set_capability(&claude, Capability::Read, true)
        .unwrap();
    store
        .set_capability(&claude, Capability::Write, true)
        .unwrap();
    store
        .set_capability(&claude, Capability::Shell, false)
        .unwrap();
    store
        .set_capability(&codex, Capability::Read, true)
        .unwrap();
    store.set_capability(&codex, Capability::Net, true).unwrap();
    let caps = store.list_capabilities().unwrap();
    assert_eq!(caps.len(), 2);
    assert_eq!(caps[0].agent.as_str(), "claude");
    assert_eq!(caps[0].allowed, vec![Capability::Read, Capability::Write]);
    assert_eq!(caps[1].agent.as_str(), "codex");
    assert_eq!(caps[1].allowed, vec![Capability::Read, Capability::Net]);
}

#[test]
fn migration_2_applies_on_migration_1_db() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("state.db");
    {
        let store = Store::open(&path).unwrap();
        store.set_setting("theme", "dark").unwrap();
    }
    let store = Store::open(&path).unwrap();
    assert_eq!(store.get_setting("theme").unwrap().as_deref(), Some("dark"));
    let tool = sample_tool("claude");
    store.upsert_tool(&tool).unwrap();
    let listed = store.list_tools().unwrap();
    assert!(listed[0].enabled);
}

#[test]
fn reset_for_new_run_clears_tasks_but_keeps_events_and_settings() {
    let store = Store::open_in_memory().unwrap();
    store
        .upsert_task(&sentinel_core::Task::new(
            sentinel_core::TaskId::new("T-01"),
            "t",
            "b",
        ))
        .unwrap();
    store.set_setting("k", "v").unwrap();
    store
        .append_event(
            &sentinel_core::Event::ReportReady,
            time::OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap();
    store.reset_for_new_run().unwrap();
    assert_eq!(store.list_tasks().unwrap(), Vec::new());
    assert_eq!(store.get_setting("k").unwrap().as_deref(), Some("v"));
    assert_eq!(store.last_event_id().unwrap(), 1);
}
