use sentinel_core::{AgentCapabilities, ApprovalKind, Capability, PermissionMode, TaskId, ToolId};
use sentinel_gate::{
    ApprovalBook, ExtensionAction, WaveGate, capability_allows, default_capabilities,
    extension_action, next_mode_after_plan_approval as next_mode, plan_gate, wave_gate,
    write_is_never_an_approval,
};
use time::{Duration, OffsetDateTime};

fn now() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("valid fixed timestamp")
}

#[test]
fn extension_action_matches_mode() {
    assert_eq!(
        extension_action(PermissionMode::Bypass),
        ExtensionAction::GrantAndLog
    );
    assert_eq!(
        extension_action(PermissionMode::PlanOnly),
        ExtensionAction::AskCard
    );
    assert_eq!(
        extension_action(PermissionMode::AskEachWave),
        ExtensionAction::AskCard
    );
    assert_eq!(
        extension_action(PermissionMode::AutoInsideReserved),
        ExtensionAction::AskCard
    );
}

#[test]
fn wave_gate_matches_mode() {
    assert_eq!(wave_gate(PermissionMode::AskEachWave), WaveGate::Card);
    assert_eq!(wave_gate(PermissionMode::PlanOnly), WaveGate::None);
    assert_eq!(
        wave_gate(PermissionMode::AutoInsideReserved),
        WaveGate::None
    );
    assert_eq!(wave_gate(PermissionMode::Bypass), WaveGate::None);
}

#[test]
fn plan_gate_only_for_plan_only() {
    assert!(plan_gate(PermissionMode::PlanOnly));
    assert!(!plan_gate(PermissionMode::AskEachWave));
    assert!(!plan_gate(PermissionMode::AutoInsideReserved));
    assert!(!plan_gate(PermissionMode::Bypass));
}

#[test]
fn next_mode_after_plan_approval_test() {
    assert_eq!(
        next_mode(PermissionMode::PlanOnly),
        PermissionMode::AutoInsideReserved
    );
    assert_eq!(
        next_mode(PermissionMode::AskEachWave),
        PermissionMode::AskEachWave
    );
    assert_eq!(
        next_mode(PermissionMode::AutoInsideReserved),
        PermissionMode::AutoInsideReserved
    );
    assert_eq!(next_mode(PermissionMode::Bypass), PermissionMode::Bypass);
}

#[test]
fn capability_allows_off_wins_over_bypass() {
    let agent = ToolId::new("claude");
    let caps = vec![AgentCapabilities {
        agent: agent.clone(),
        allowed: vec![Capability::Read, Capability::Write],
    }];
    assert!(!capability_allows(&caps, &agent, Capability::Ext));
    assert!(capability_allows(&caps, &agent, Capability::Read));
}

#[test]
fn capability_allows_no_row_uses_defaults() {
    let agent = ToolId::new("unknown");
    let caps = vec![];
    assert!(capability_allows(&caps, &agent, Capability::Read));
    assert!(capability_allows(&caps, &agent, Capability::Write));
    assert!(capability_allows(&caps, &agent, Capability::Ext));
    assert!(capability_allows(&caps, &agent, Capability::Tests));
    assert!(!capability_allows(&caps, &agent, Capability::Shell));
    assert!(!capability_allows(&caps, &agent, Capability::Net));
}

#[test]
fn default_capabilities_order_and_values() {
    let agent = ToolId::new("claude");
    let caps = default_capabilities(&agent);
    assert_eq!(caps.agent, agent);
    assert_eq!(
        caps.allowed,
        vec![
            Capability::Read,
            Capability::Write,
            Capability::Ext,
            Capability::Tests,
        ]
    );
}

#[test]
fn write_is_never_an_approval_is_true() {
    assert!(write_is_never_an_approval());
}

#[test]
fn book_duplicate_extension_returns_same_card() {
    let mut book = ApprovalBook::new();
    let task = TaskId::new("T-01");
    let agent = ToolId::new("claude");
    let first = book.request_extension(&task, &agent, "src/main.rs", now());
    let second = book.request_extension(&task, &agent, "src/main.rs", now());
    assert_eq!(first.id, second.id);
}

#[test]
fn book_answer_once_then_none() {
    let mut book = ApprovalBook::new();
    let task = TaskId::new("T-01");
    let agent = ToolId::new("claude");
    let card = book.request_extension(&task, &agent, "src/main.rs", now());
    let answered = book.answer(&card.id, true, "phone").unwrap();
    assert_eq!(answered.state, sentinel_core::ApprovalState::Allowed);
    assert_eq!(book.answer(&card.id, false, "phone"), None);
}

#[test]
fn book_default_stale_after_limit_not_before() {
    let mut book = ApprovalBook::new();
    let task = TaskId::new("T-01");
    let agent = ToolId::new("claude");
    let _card = book.request_extension(&task, &agent, "src/main.rs", now());
    let later = now() + Duration::minutes(11);
    let defaulted = book.default_stale(later, Duration::minutes(10), &[ApprovalKind::Extension]);
    assert_eq!(defaulted.len(), 1);
    assert_eq!(defaulted[0].state, sentinel_core::ApprovalState::Defaulted);
    assert_eq!(defaulted[0].decided_by.as_deref(), Some("default"));
}

#[test]
fn book_default_stale_respects_kind_filter() {
    let mut book = ApprovalBook::new();
    let task = TaskId::new("T-01");
    let agent = ToolId::new("claude");
    book.request_extension(&task, &agent, "src/main.rs", now());
    book.request_wave(1, now());
    let later = now() + Duration::minutes(11);
    let defaulted = book.default_stale(later, Duration::minutes(10), &[ApprovalKind::Extension]);
    assert_eq!(defaulted.len(), 1);
    assert_eq!(defaulted[0].kind, ApprovalKind::Extension);
}
