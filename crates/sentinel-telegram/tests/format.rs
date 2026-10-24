#![allow(clippy::unwrap_used, reason = "tests fail loudly on setup errors")]

use std::collections::BTreeSet;

use sentinel_core::{
    Approval, ApprovalId, ApprovalKind, ApprovalState, ConnectionState, Decision, DecisionId,
    DecisionState, Event, HarnessState, TaskId, TaskStatus, ToolId,
};
use sentinel_telegram::buttons::ButtonAction;
use sentinel_telegram::format::{
    Priority, RenderContext, Rendered, WaveTracker, render, render_wave,
};
use sentinel_telegram::levels::{ChatSettings, Detail, EventKind, Level};

fn settings(levels: &[Level], detail: Detail, events: &[EventKind]) -> ChatSettings {
    ChatSettings {
        levels: levels.iter().copied().collect::<BTreeSet<_>>(),
        detail,
        events: events.iter().copied().collect::<BTreeSet<_>>(),
        quiet_hours: None,
    }
}

fn everything(detail: Detail) -> ChatSettings {
    settings(
        &[Level::Notify, Level::Control, Level::Code, Level::Reports],
        detail,
        &[
            EventKind::ApprovalNeeded,
            EventKind::DecisionNeeded,
            EventKind::Budget70,
            EventKind::Budget95,
            EventKind::TaskBlocked,
            EventKind::WaveFinished,
            EventKind::RunFinished,
            EventKind::BrainProposals,
            EventKind::Errors,
        ],
    )
}

fn ctx(settings: &ChatSettings) -> RenderContext<'_> {
    RenderContext {
        settings,
        project: "fee-ledger-api",
        hackathon: false,
        cost_line: Some("Cost $0.88 of $2.00"),
    }
}

fn approval_event() -> Event {
    Event::ApprovalRequested {
        approval: Approval {
            id: ApprovalId::new("a1"),
            kind: ApprovalKind::Extension,
            task: Some(TaskId::new("T-05")),
            agent: Some(ToolId::new("codex")),
            path: Some("src/api/errors.ts".to_owned()),
            state: ApprovalState::Pending,
            decided_by: None,
        },
    }
}

fn text_of(rendered: Option<Rendered>) -> String {
    rendered.unwrap().text
}

fn labels(rendered: &Rendered) -> Vec<String> {
    rendered
        .buttons
        .iter()
        .flatten()
        .map(|button| button.label.clone())
        .collect()
}

#[test]
fn an_approval_at_summary_names_the_file_and_offers_allow_and_deny() {
    let all = everything(Detail::Summary);
    let rendered = render(&approval_event(), &ctx(&all)).unwrap();

    assert!(
        rendered
            .text
            .contains("T-05 wants to write src/api/errors.ts. Nobody has reserved it."),
        "{}",
        rendered.text
    );
    assert_eq!(labels(&rendered), ["Allow", "Deny"]);
    assert_eq!(rendered.priority, Priority::Urgent);
}

#[test]
fn an_approval_at_status_shows_the_task_and_no_file_name() {
    let all = everything(Detail::Status);
    let text = text_of(render(&approval_event(), &ctx(&all)));

    assert!(text.contains("T-05 needs approval."), "{text}");
    assert!(!text.contains("src/api/errors.ts"), "{text}");
}

#[test]
fn a_chat_without_control_gets_the_approval_text_but_no_buttons() {
    let notify = settings(
        &[Level::Notify],
        Detail::Summary,
        &[EventKind::ApprovalNeeded],
    );
    let rendered = render(&approval_event(), &ctx(&notify)).unwrap();

    assert_eq!(rendered.buttons.len(), 0);
}

#[test]
fn nothing_is_sent_without_the_notify_level() {
    let no_notify = settings(
        &[Level::Control],
        Detail::Summary,
        &[EventKind::ApprovalNeeded],
    );

    assert!(render(&approval_event(), &ctx(&no_notify)).is_none());
}

#[test]
fn a_switched_off_event_kind_is_not_sent() {
    let only_budget = settings(&[Level::Notify], Detail::Summary, &[EventKind::Budget70]);

    assert!(render(&approval_event(), &ctx(&only_budget)).is_none());
}

#[test]
fn a_decision_has_one_button_per_task_and_skip_both() {
    let decision = Decision {
        id: DecisionId::new("d1"),
        prompt: "Two tasks touch one file".to_owned(),
        tasks: vec![TaskId::new("T-02"), TaskId::new("T-05")],
        state: DecisionState::Pending,
        answer: None,
    };
    let all = everything(Detail::Summary);
    let rendered = render(&Event::DecisionRequested { decision }, &ctx(&all)).unwrap();

    assert_eq!(labels(&rendered), ["T-02", "T-05", "Skip both"]);
    let skip = rendered.buttons[0].last().unwrap();
    assert_eq!(
        skip.action,
        ButtonAction::Decision {
            decision: DecisionId::new("d1"),
            answer: "skip_both".to_owned(),
        }
    );
    assert!(rendered.text.contains("Decision needed for T-02, T-05."));
}

#[test]
fn budget_at_seventy_is_urgent_and_at_ninety_five_uses_its_own_switch() {
    let all = everything(Detail::Summary);
    let seventy = render(&Event::BudgetThreshold { percent: 70 }, &ctx(&all)).unwrap();
    assert_eq!(seventy.priority, Priority::Urgent);
    assert!(seventy.text.contains("Budget at 70%."), "{}", seventy.text);
    assert!(
        seventy.text.contains("Cost $0.88 of $2.00"),
        "{}",
        seventy.text
    );

    let ninety_five = render(&Event::BudgetThreshold { percent: 95 }, &ctx(&all)).unwrap();
    assert!(ninety_five.text.contains("Budget at 95%."));
    let only_seventy = settings(&[Level::Notify], Detail::Summary, &[EventKind::Budget70]);
    assert!(render(&Event::BudgetThreshold { percent: 95 }, &ctx(&only_seventy)).is_none());
}

#[test]
fn other_budget_thresholds_are_not_sent() {
    let all = everything(Detail::Summary);

    assert!(render(&Event::BudgetThreshold { percent: 50 }, &ctx(&all)).is_none());
}

#[test]
fn a_task_that_is_blocked_is_announced_and_a_verified_one_is_not() {
    let all = everything(Detail::Status);
    let blocked = Event::TaskStatus {
        task: TaskId::new("T-04"),
        status: TaskStatus::Blocked,
    };
    assert!(text_of(render(&blocked, &ctx(&all))).contains("T-04 blocked."));

    let verified = Event::TaskStatus {
        task: TaskId::new("T-04"),
        status: TaskStatus::Verified,
    };
    assert!(render(&verified, &ctx(&all)).is_none());
}

#[test]
fn a_finished_run_offers_report_only_to_the_reports_level() {
    let run = Event::RunFinished {
        verified: 3,
        total: 3,
    };
    let reports = settings(
        &[Level::Notify, Level::Reports],
        Detail::Summary,
        &[EventKind::RunFinished],
    );
    let rendered = render(&run, &ctx(&reports)).unwrap();
    assert!(rendered.text.contains("Run finished. 3 of 3 verified."));
    assert_eq!(labels(&rendered), ["Report"]);

    let notify_only = settings(&[Level::Notify], Detail::Summary, &[EventKind::RunFinished]);
    assert_eq!(render(&run, &ctx(&notify_only)).unwrap().buttons.len(), 0);
}

#[test]
fn a_crash_and_a_failed_connection_are_error_events() {
    let errors = settings(&[Level::Notify], Detail::Status, &[EventKind::Errors]);
    let crashed = Event::HarnessState {
        state: HarnessState::Crashed,
    };
    assert_eq!(
        text_of(render(&crashed, &ctx(&errors))),
        "Sentinel, fee-ledger-api\nSentinel crashed. Check the PC."
    );
    let failed = Event::ConnectionChanged {
        name: "claude".to_owned(),
        state: ConnectionState::Failed,
    };
    assert!(text_of(render(&failed, &ctx(&errors))).contains("claude connection failed."));
    let healthy = Event::ConnectionChanged {
        name: "claude".to_owned(),
        state: ConnectionState::Connected,
    };
    assert!(render(&healthy, &ctx(&errors)).is_none());
}

#[test]
fn the_host_back_message_is_sent_even_with_no_event_switches() {
    let bare = settings(&[Level::Notify], Detail::Status, &[]);
    let event = Event::HostBackOnline {
        gap_seconds: 185 * 60,
    };

    assert_eq!(
        text_of(render(&event, &ctx(&bare))),
        "Sentinel is back online after 185 minutes."
    );
}

#[test]
fn brain_proposals_and_care_are_sent_when_switched_on() {
    let brain = settings(
        &[Level::Notify],
        Detail::Status,
        &[EventKind::BrainProposals],
    );
    let proposal = Event::ProposalCreated {
        proposal: sentinel_core::NoteId::new("n1"),
    };
    assert!(text_of(render(&proposal, &ctx(&brain))).contains("Brain has a new proposal."));
    let care = Event::CareFinished {
        summary: "Two lessons".to_owned(),
    };
    assert!(text_of(render(&care, &ctx(&brain))).contains("Brain care finished."));
}

#[test]
fn an_event_that_is_not_covered_renders_nothing() {
    let all = everything(Detail::Detail);

    assert!(render(&Event::HostHeartbeat, &ctx(&all)).is_none());
}

#[test]
fn hackathon_mode_forces_status_and_hides_files_and_cost() {
    let all = everything(Detail::Detail);
    let hackathon = RenderContext {
        hackathon: true,
        ..ctx(&all)
    };
    let text = text_of(render(&approval_event(), &hackathon));

    assert!(text.contains("T-05 needs approval."), "{text}");
    assert!(!text.contains("src/api/errors.ts"), "{text}");
    let budget = text_of(render(&Event::BudgetThreshold { percent: 70 }, &hackathon));
    assert!(!budget.contains("Cost"), "{budget}");
}

#[test]
fn a_key_in_a_reason_is_hidden_before_sending() {
    let all = everything(Detail::Detail);
    let mut tracker = WaveTracker::default();
    tracker.observe(&Event::WaveStarted {
        wave: 1,
        tasks: vec![TaskId::new("T-01")],
    });
    let verdict = Event::Verdict {
        task: TaskId::new("T-01"),
        status: TaskStatus::Verified,
        reason: "used sk-abcdefghijklmnopqrstuvwx for the call".to_owned(),
    };
    let summary = tracker.observe(&verdict).unwrap();
    let text = render_wave(&summary, &ctx(&all)).unwrap().text;

    assert!(!text.contains("sk-abcdef"), "{text}");
    assert!(text.contains("[hidden]"), "{text}");
}

fn wave_of_one(detail: Detail) -> Option<Rendered> {
    let all = everything(detail);
    let mut tracker = WaveTracker::default();
    tracker.observe(&Event::WaveStarted {
        wave: 2,
        tasks: vec![TaskId::new("T-04"), TaskId::new("T-02")],
    });
    tracker.observe(&Event::Checkpoint {
        task: TaskId::new("T-04"),
        commit: "abc".to_owned(),
        paths: vec!["src/ledger/post.ts".to_owned()],
    });
    tracker.observe(&Event::Verdict {
        task: TaskId::new("T-04"),
        status: TaskStatus::Verified,
        reason: "tests pass\nlint clean".to_owned(),
    });
    let summary = tracker.observe(&Event::Verdict {
        task: TaskId::new("T-02"),
        status: TaskStatus::Verified,
        reason: "ok".to_owned(),
    });
    render_wave(&summary.unwrap(), &ctx(&all))
}

#[test]
fn a_wave_summary_at_status_lists_task_ids_and_states_only() {
    let text = wave_of_one(Detail::Status).unwrap().text;

    assert_eq!(
        text,
        "Sentinel, fee-ledger-api\nWave 2 done. 2 of 2 verified.\nT-04 Verified\nT-02 Verified"
    );
}

#[test]
fn a_wave_summary_at_summary_adds_file_names_and_cost() {
    let text = wave_of_one(Detail::Summary).unwrap().text;

    assert!(text.contains("T-04 Verified: src/ledger/post.ts"), "{text}");
    assert!(text.contains("Cost $0.88 of $2.00"), "{text}");
    assert!(!text.contains("tests pass"), "{text}");
}

#[test]
fn a_wave_summary_at_detail_adds_at_most_six_reason_lines() {
    let all = everything(Detail::Detail);
    let mut tracker = WaveTracker::default();
    tracker.observe(&Event::WaveStarted {
        wave: 1,
        tasks: vec![TaskId::new("T-01")],
    });
    let reason = (1..=10)
        .map(|n| format!("reason line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    let summary = tracker
        .observe(&Event::Verdict {
            task: TaskId::new("T-01"),
            status: TaskStatus::Verified,
            reason,
        })
        .unwrap();
    let text = render_wave(&summary, &ctx(&all)).unwrap().text;

    let indented: Vec<&str> = text.lines().filter(|line| line.starts_with("  ")).collect();
    assert_eq!(indented.len(), 6, "{text}");
    assert_eq!(indented[0], "  reason line 1", "{text}");
    assert!(!text.contains("reason line 7"), "{text}");
}

#[test]
fn the_f20_wave_example_matches_the_doc_exactly() {
    let all = everything(Detail::Summary);
    let mut tracker = WaveTracker::default();
    tracker.observe(&Event::WaveStarted {
        wave: 2,
        tasks: vec![
            TaskId::new("T-04"),
            TaskId::new("T-02"),
            TaskId::new("T-06"),
        ],
    });
    let files = [
        ("T-04", "src/ledger/post.ts"),
        ("T-02", "src/fees/rules.ts"),
        ("T-06", "src/limits/daily.ts"),
    ];
    let mut summary = None;
    for (task, path) in files {
        tracker.observe(&Event::Checkpoint {
            task: TaskId::new(task),
            commit: String::new(),
            paths: vec![path.to_owned()],
        });
        let verdict = Event::Verdict {
            task: TaskId::new(task),
            status: TaskStatus::Verified,
            reason: String::new(),
        };
        summary = tracker.observe(&verdict).or(summary);
    }
    let rendered = render_wave(&summary.unwrap(), &ctx(&all)).unwrap();

    assert_eq!(
        rendered.text,
        "Sentinel, fee-ledger-api\nWave 2 done. 3 of 3 verified.\nT-04 Verified: src/ledger/post.ts\nT-02 Verified: src/fees/rules.ts\nT-06 Verified: src/limits/daily.ts\nCost $0.88 of $2.00"
    );
    assert_eq!(labels(&rendered), ["Pause", "Diff summary"]);
}

#[test]
fn the_f20_approval_example_matches_the_doc_exactly() {
    let all = everything(Detail::Summary);
    let rendered = render(&approval_event(), &ctx(&all)).unwrap();

    assert_eq!(
        rendered.text,
        "codex asks for another file\nT-05 wants to write src/api/errors.ts. Nobody has reserved it."
    );
    assert_eq!(labels(&rendered), ["Allow", "Deny"]);
}

#[test]
fn a_wave_summary_offers_pause_and_diff_summary() {
    let rendered = wave_of_one(Detail::Summary).unwrap();

    assert_eq!(labels(&rendered), ["Pause", "Diff summary"]);
}

#[test]
fn the_wave_summary_waits_for_every_verdict() {
    let mut tracker = WaveTracker::default();
    tracker.observe(&Event::WaveStarted {
        wave: 1,
        tasks: vec![
            TaskId::new("T-01"),
            TaskId::new("T-02"),
            TaskId::new("T-03"),
        ],
    });

    assert!(
        tracker
            .observe(&Event::Verdict {
                task: TaskId::new("T-01"),
                status: TaskStatus::Verified,
                reason: String::new(),
            })
            .is_none()
    );
    assert!(
        tracker
            .observe(&Event::Verdict {
                task: TaskId::new("T-02"),
                status: TaskStatus::Blocked,
                reason: String::new(),
            })
            .is_none()
    );
    let summary = tracker
        .observe(&Event::Verdict {
            task: TaskId::new("T-03"),
            status: TaskStatus::Verified,
            reason: String::new(),
        })
        .unwrap();

    assert_eq!(summary.verified(), 2);
    assert_eq!(summary.total(), 3);
}

#[test]
fn a_long_wave_is_cut_at_a_line_end_under_4000_characters() {
    let all = everything(Detail::Summary);
    let tasks: Vec<TaskId> = (0..120).map(|n| TaskId::new(format!("T-{n:03}"))).collect();
    let mut tracker = WaveTracker::default();
    tracker.observe(&Event::WaveStarted {
        wave: 9,
        tasks: tasks.clone(),
    });
    let mut summary = None;
    for task in &tasks {
        let path = format!("src/module_module_module/{task}_handler_module.rs");
        tracker.observe(&Event::Checkpoint {
            task: task.clone(),
            commit: String::new(),
            paths: vec![path],
        });
        let verdict = Event::Verdict {
            task: task.clone(),
            status: TaskStatus::Verified,
            reason: String::new(),
        };
        summary = tracker.observe(&verdict).or(summary);
    }
    let rendered = render_wave(&summary.unwrap(), &ctx(&all)).unwrap();

    assert!(
        rendered.text.chars().count() <= 4000,
        "{}",
        rendered.text.len()
    );
    let body = rendered.text.strip_suffix("...").unwrap();
    let last_line = body.lines().last().unwrap();
    assert_eq!(last_line.split('.').next_back(), Some("rs"), "{body}");
}
