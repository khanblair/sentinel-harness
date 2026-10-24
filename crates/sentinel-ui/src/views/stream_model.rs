//! Turns the daemon's events into stream items. Pure: no signals, no DOM, so every mapping has a
//! unit test. Events the stream does not show map to nothing.

use sentinel_core::{
    ApprovalId, DecisionId, Event, EventRecord, Snapshot, TaskId, TaskStatus, ToolId,
};

/// Where an agent's attempt stands, read from the events after it started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentState {
    Running,
    Done,
    Failed,
    Killed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BarrierState {
    Running,
    Pass,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Item {
    User {
        from: String,
        to: Option<ToolId>,
        text: String,
    },
    System {
        text: String,
    },
    Plan {
        tasks: u32,
        contradictions: u32,
        already_done: u32,
    },
    Wave {
        wave: u32,
        tasks: Vec<TaskId>,
    },
    /// One card per attempt.
    Agent {
        task: TaskId,
        agent: ToolId,
        state: AgentState,
    },
    Checkpoint {
        task: TaskId,
        commit: String,
        paths: Vec<String>,
    },
    Approval {
        id: ApprovalId,
    },
    Decision {
        id: DecisionId,
    },
    BlockedWrite {
        path: String,
        restored: bool,
    },
    Barrier {
        wave: u32,
        state: BarrierState,
    },
    Verdict {
        task: TaskId,
        status: TaskStatus,
        reason: String,
    },
    Budget {
        percent: u8,
    },
    Ceiling {
        task: TaskId,
    },
    Compacted {
        task: TaskId,
        before: u32,
        after: u32,
    },
    Report {
        verified: u32,
        total: u32,
    },
}

/// The stream items for a snapshot, keyed by event id, oldest first. With a focused agent, the
/// agent's own items and the messages that name it stay; the rest are hidden.
pub fn build_items(snapshot: &Snapshot, focus: Option<&ToolId>) -> Vec<(i64, Item)> {
    let events = snapshot.events.as_slice();
    let mut items: Vec<(i64, Item)> = Vec::new();
    // Each barrier start, with its slot in `items`, so the end can update it in place.
    let mut barrier_starts: Vec<(u32, usize)> = Vec::new();
    for (index, record) in events.iter().enumerate() {
        if let Event::BarrierStart { wave } = &record.event {
            barrier_starts.push((*wave, items.len()));
        }
        if let Event::BarrierEnd { wave, passed } = &record.event
            && barrier_end(&mut items, &barrier_starts, *wave, *passed)
        {
            continue;
        }
        let Some(item) = item_for(events, index) else {
            continue;
        };
        if is_visible(events, index, focus) {
            items.push((record.id, item));
        }
    }
    items
}

fn barrier_end(
    items: &mut [(i64, Item)],
    starts: &[(u32, usize)],
    wave: u32,
    passed: bool,
) -> bool {
    let Some(&(_, slot)) = starts.iter().rev().find(|(start, _)| *start == wave) else {
        return false;
    };
    items[slot].1 = Item::Barrier {
        wave,
        state: barrier_result(passed),
    };
    true
}

fn barrier_result(passed: bool) -> BarrierState {
    if passed {
        BarrierState::Pass
    } else {
        BarrierState::Fail
    }
}

/// The stream item one event becomes, if the stream shows it.
fn item_for(events: &[EventRecord], index: usize) -> Option<Item> {
    let event = &events[index].event;
    if let Event::TaskStatus {
        task,
        status: TaskStatus::Running,
    } = event
    {
        return running_item(events, index, task);
    }
    run_item(event).or_else(|| review_item(event))
}

/// Run, plan, and message events.
fn run_item(event: &Event) -> Option<Item> {
    if let Event::RunStarted { backlog, .. } = event {
        return Some(Item::System {
            text: format!("Run started: {backlog}."),
        });
    }
    if let Event::Message { from, to, text } = event {
        // The agent card already says who took the task, so the routing note adds nothing.
        let routing = from == "sentinel" && to.is_some() && text.contains(" goes to ");
        return (!routing).then(|| message_item(from, to.as_ref(), text));
    }
    if let Event::PlanReady {
        tasks,
        contradictions,
        already_done,
    } = event
    {
        return Some(Item::Plan {
            tasks: *tasks,
            contradictions: *contradictions,
            already_done: *already_done,
        });
    }
    wave_item(event)
}

/// Wave and checkpoint events.
fn wave_item(event: &Event) -> Option<Item> {
    if let Event::WaveStarted { wave, tasks } = event {
        return Some(Item::Wave {
            wave: *wave,
            tasks: tasks.clone(),
        });
    }
    if let Event::Checkpoint {
        task,
        commit,
        paths,
    } = event
    {
        return Some(Item::Checkpoint {
            task: task.clone(),
            commit: commit.clone(),
            paths: paths.clone(),
        });
    }
    None
}

/// Approval, decision, write, and barrier events.
fn review_item(event: &Event) -> Option<Item> {
    if let Event::ApprovalRequested { approval } = event {
        return Some(Item::Approval {
            id: approval.id.clone(),
        });
    }
    if let Event::DecisionRequested { decision } = event {
        return Some(Item::Decision {
            id: decision.id.clone(),
        });
    }
    if let Event::WriteBlocked { path, restored, .. } = event {
        return Some(Item::BlockedWrite {
            path: path.clone(),
            restored: *restored,
        });
    }
    if let Event::BarrierStart { wave } = event {
        return Some(Item::Barrier {
            wave: *wave,
            state: BarrierState::Running,
        });
    }
    if let Event::BarrierEnd { wave, passed } = event {
        return Some(Item::Barrier {
            wave: *wave,
            state: barrier_result(*passed),
        });
    }
    verdict_item(event)
}

fn verdict_item(event: &Event) -> Option<Item> {
    if let Event::Verdict {
        task,
        status,
        reason,
    } = event
    {
        return Some(Item::Verdict {
            task: task.clone(),
            status: *status,
            reason: reason.clone(),
        });
    }
    summary_item(event)
}

/// Budget, ceiling, context, and report events.
fn summary_item(event: &Event) -> Option<Item> {
    if let Event::BudgetThreshold { percent } = event {
        return Some(Item::Budget { percent: *percent });
    }
    if let Event::TaskCeilingHit { task, .. } = event {
        return Some(Item::Ceiling { task: task.clone() });
    }
    if let Event::ContextCompacted {
        task,
        tokens_before,
        tokens_after,
    } = event
    {
        return Some(Item::Compacted {
            task: task.clone(),
            before: *tokens_before,
            after: *tokens_after,
        });
    }
    if let Event::RunFinished { verified, total } = event {
        return Some(Item::Report {
            verified: *verified,
            total: *total,
        });
    }
    None
}

fn message_item(from: &str, to: Option<&ToolId>, text: &str) -> Item {
    if from == "sentinel" {
        Item::System {
            text: text.to_owned(),
        }
    } else {
        Item::User {
            from: from.to_owned(),
            to: to.cloned(),
            text: text.to_owned(),
        }
    }
}

/// A task that starts running gets an agent card, when the message that routes it names an agent.
fn running_item(events: &[EventRecord], index: usize, task: &TaskId) -> Option<Item> {
    let rest = &events[index + 1..];
    let agent = agent_after(rest, task)?;
    Some(Item::Agent {
        task: task.clone(),
        agent,
        state: agent_state(rest, task),
    })
}

/// The agent a message such as "T-01 goes to claude" names for this task.
pub fn goes_to_agent(events: &[EventRecord], task: &TaskId) -> Option<ToolId> {
    events
        .iter()
        .rev()
        .find_map(|record| routed_agent(&record.event, task))
}

fn agent_after(rest: &[EventRecord], task: &TaskId) -> Option<ToolId> {
    rest.iter()
        .find_map(|record| routed_agent(&record.event, task))
}

fn routed_agent(event: &Event, task: &TaskId) -> Option<ToolId> {
    let Event::Message {
        to: Some(agent),
        text,
        ..
    } = event
    else {
        return None;
    };
    let prefix = format!("{task} goes to ");
    text.starts_with(&prefix).then(|| agent.clone())
}

fn agent_state(rest: &[EventRecord], task: &TaskId) -> AgentState {
    rest.iter()
        .find_map(|record| attempt_end(&record.event, task))
        .unwrap_or(AgentState::Running)
}

fn attempt_end(event: &Event, task: &TaskId) -> Option<AgentState> {
    if let Event::Checkpoint { task: done, .. } = event {
        return (done == task).then_some(AgentState::Done);
    }
    if let Event::Message { from, text, .. } = event {
        let kill = from == "sentinel" && text.contains(task.as_str()) && text.contains("killed");
        return kill.then_some(AgentState::Killed);
    }
    if let Event::TaskStatus {
        task: moved,
        status,
    } = event
    {
        let failed = matches!(status, TaskStatus::Ready | TaskStatus::Blocked);
        return (moved == task && failed).then_some(AgentState::Failed);
    }
    None
}

/// Whether an event belongs in the stream for the focused agent. Events without an agent always
/// show.
fn is_visible(events: &[EventRecord], index: usize, focus: Option<&ToolId>) -> bool {
    let Some(focus) = focus else {
        return true;
    };
    event_agent(events, index).is_none_or(|agent| agent == *focus)
}

fn event_agent(events: &[EventRecord], index: usize) -> Option<ToolId> {
    let event = &events[index].event;
    if let Event::Message { to, .. } = event {
        return to.clone();
    }
    if let Event::Checkpoint { task, .. } = event {
        return goes_to_agent(&events[..index], task);
    }
    if let Event::TaskStatus {
        task,
        status: TaskStatus::Running,
    } = event
    {
        return agent_after(&events[index + 1..], task);
    }
    None
}

/// A short token count, such as `52k` or `900`.
pub fn token_label(tokens: u32) -> String {
    if tokens >= 1000 {
        format!("{}k", tokens / 1000)
    } else {
        tokens.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{Approval, ApprovalKind, ApprovalState, Decision, DecisionState};
    use time::OffsetDateTime;

    fn record(id: i64, event: Event) -> EventRecord {
        EventRecord {
            chat: None,
            id,
            at: OffsetDateTime::UNIX_EPOCH,
            event,
        }
    }

    fn snapshot(events: Vec<Event>) -> Snapshot {
        let mut snapshot = Snapshot::empty();
        snapshot.events = events
            .into_iter()
            .zip(1..)
            .map(|(event, id)| record(id, event))
            .collect();
        snapshot
    }

    fn routed(task: &str, agent: &str) -> Event {
        Event::Message {
            from: "sentinel".to_owned(),
            to: Some(ToolId::new(agent)),
            text: format!("{task} goes to Claude Code (easy task)."),
        }
    }

    fn running(task: &str) -> Event {
        Event::TaskStatus {
            task: TaskId::new(task),
            status: TaskStatus::Running,
        }
    }

    fn items(events: Vec<Event>, focus: Option<&str>) -> Vec<Item> {
        let focus = focus.map(ToolId::new);
        build_items(&snapshot(events), focus.as_ref())
            .into_iter()
            .map(|(_, item)| item)
            .collect()
    }

    #[test]
    fn run_start_becomes_a_system_line_with_the_backlog() {
        let shown = items(
            vec![Event::RunStarted {
                run: sentinel_core::RunId::new("run-1"),
                backlog: "fee-ledger-api/backlog.md".to_owned(),
            }],
            None,
        );
        assert_eq!(
            shown,
            vec![Item::System {
                text: "Run started: fee-ledger-api/backlog.md.".to_owned()
            }]
        );
    }

    #[test]
    fn a_user_message_is_a_user_item_and_a_sentinel_message_is_a_system_item() {
        let shown = items(
            vec![
                Event::Message {
                    from: "Blair".to_owned(),
                    to: Some(ToolId::new("codex")),
                    text: "Keep the API stable.".to_owned(),
                },
                Event::Message {
                    from: "sentinel".to_owned(),
                    to: None,
                    text: "Noted.".to_owned(),
                },
            ],
            None,
        );
        assert_eq!(
            shown,
            vec![
                Item::User {
                    from: "Blair".to_owned(),
                    to: Some(ToolId::new("codex")),
                    text: "Keep the API stable.".to_owned(),
                },
                Item::System {
                    text: "Noted.".to_owned()
                },
            ]
        );
    }

    #[test]
    fn plan_and_wave_events_map_to_their_items() {
        let shown = items(
            vec![
                Event::PlanReady {
                    tasks: 12,
                    contradictions: 1,
                    already_done: 2,
                },
                Event::WaveStarted {
                    wave: 1,
                    tasks: vec![TaskId::new("T-01")],
                },
            ],
            None,
        );
        assert_eq!(
            shown[0],
            Item::Plan {
                tasks: 12,
                contradictions: 1,
                already_done: 2
            }
        );
        assert_eq!(
            shown[1],
            Item::Wave {
                wave: 1,
                tasks: vec![TaskId::new("T-01")]
            }
        );
    }

    #[test]
    fn a_running_task_with_a_routed_agent_gets_a_running_card() {
        let shown = items(vec![running("T-01"), routed("T-01", "claude")], None);
        assert_eq!(
            shown[0],
            Item::Agent {
                task: TaskId::new("T-01"),
                agent: ToolId::new("claude"),
                state: AgentState::Running,
            }
        );
    }

    #[test]
    fn a_checkpoint_after_the_attempt_marks_the_card_done() {
        let shown = items(
            vec![
                running("T-01"),
                routed("T-01", "claude"),
                Event::Checkpoint {
                    task: TaskId::new("T-01"),
                    commit: "abc1234".to_owned(),
                    paths: vec!["src/a.rs".to_owned()],
                },
            ],
            None,
        );
        assert!(matches!(
            shown[0],
            Item::Agent {
                state: AgentState::Done,
                ..
            }
        ));
        // The routing note is not shown: the agent card says it.
        assert!(matches!(shown[1], Item::Checkpoint { .. }));
    }

    #[test]
    fn a_ready_or_blocked_status_after_the_attempt_marks_the_card_failed() {
        let shown = items(
            vec![
                running("T-01"),
                routed("T-01", "claude"),
                Event::TaskStatus {
                    task: TaskId::new("T-01"),
                    status: TaskStatus::Ready,
                },
            ],
            None,
        );
        assert!(matches!(
            shown[0],
            Item::Agent {
                state: AgentState::Failed,
                ..
            }
        ));
    }

    #[test]
    fn a_kill_message_marks_the_card_killed() {
        let shown = items(
            vec![
                running("T-01"),
                routed("T-01", "claude"),
                Event::Message {
                    from: "sentinel".to_owned(),
                    to: None,
                    text: "T-01 was killed by the user.".to_owned(),
                },
            ],
            None,
        );
        assert!(matches!(
            shown[0],
            Item::Agent {
                state: AgentState::Killed,
                ..
            }
        ));
    }

    #[test]
    fn running_without_a_routed_agent_shows_no_card() {
        let shown = items(vec![running("T-01")], None);
        assert_eq!(shown, Vec::new());
    }

    #[test]
    fn approvals_and_decisions_keep_their_ids() {
        let approval = Approval {
            id: ApprovalId::new("ap-1"),
            kind: ApprovalKind::Extension,
            task: None,
            agent: None,
            path: Some("src/b.rs".to_owned()),
            state: ApprovalState::Pending,
            decided_by: None,
        };
        let decision = Decision {
            id: DecisionId::new("dc-1"),
            prompt: "Pick one".to_owned(),
            tasks: vec![TaskId::new("T-01")],
            state: DecisionState::Pending,
            answer: None,
        };
        let shown = items(
            vec![
                Event::ApprovalRequested { approval },
                Event::DecisionRequested { decision },
            ],
            None,
        );
        assert_eq!(
            shown,
            vec![
                Item::Approval {
                    id: ApprovalId::new("ap-1")
                },
                Item::Decision {
                    id: DecisionId::new("dc-1")
                },
            ]
        );
    }

    #[test]
    fn a_blocked_write_keeps_its_path_and_restore_state() {
        let shown = items(
            vec![Event::WriteBlocked {
                task: None,
                path: "src/c.rs".to_owned(),
                restored: true,
            }],
            None,
        );
        assert_eq!(
            shown,
            vec![Item::BlockedWrite {
                path: "src/c.rs".to_owned(),
                restored: true
            }]
        );
    }

    #[test]
    fn barrier_start_and_end_make_one_item_that_takes_the_result() {
        let shown = items(
            vec![
                Event::BarrierStart { wave: 1 },
                Event::BarrierEnd {
                    wave: 1,
                    passed: true,
                },
            ],
            None,
        );
        assert_eq!(
            shown,
            vec![Item::Barrier {
                wave: 1,
                state: BarrierState::Pass
            }]
        );
    }

    #[test]
    fn a_failed_barrier_end_marks_the_item_failed() {
        let shown = items(
            vec![
                Event::BarrierStart { wave: 2 },
                Event::BarrierEnd {
                    wave: 2,
                    passed: false,
                },
            ],
            None,
        );
        assert_eq!(
            shown,
            vec![Item::Barrier {
                wave: 2,
                state: BarrierState::Fail
            }]
        );
    }

    #[test]
    fn verdict_and_budget_map_to_their_items() {
        let shown = items(
            vec![
                Event::Verdict {
                    task: TaskId::new("T-01"),
                    status: TaskStatus::Verified,
                    reason: "Tests pass.".to_owned(),
                },
                Event::BudgetThreshold { percent: 70 },
            ],
            None,
        );
        assert_eq!(shown.len(), 2);
        assert!(matches!(shown[0], Item::Verdict { .. }));
        assert_eq!(shown[1], Item::Budget { percent: 70 });
    }

    #[test]
    fn ceiling_and_compaction_map_to_their_items() {
        let shown = items(
            vec![
                Event::TaskCeilingHit {
                    task: TaskId::new("T-02"),
                    cost: sentinel_core::Cost::ZERO,
                    ceiling: sentinel_core::Cost::ZERO,
                },
                Event::ContextCompacted {
                    task: TaskId::new("T-03"),
                    tokens_before: 52_000,
                    tokens_after: 9_000,
                },
            ],
            None,
        );
        assert_eq!(shown.len(), 2);
        assert_eq!(
            shown[0],
            Item::Ceiling {
                task: TaskId::new("T-02")
            }
        );
        assert_eq!(
            shown[1],
            Item::Compacted {
                task: TaskId::new("T-03"),
                before: 52_000,
                after: 9_000
            }
        );
    }

    #[test]
    fn run_finished_maps_to_the_report_item() {
        let shown = items(
            vec![Event::RunFinished {
                verified: 9,
                total: 12,
            }],
            None,
        );
        assert_eq!(
            shown,
            vec![Item::Report {
                verified: 9,
                total: 12
            }]
        );
    }

    #[test]
    fn events_the_stream_does_not_show_are_skipped() {
        let shown = items(
            vec![
                Event::HostHeartbeat,
                Event::CostUpdate {
                    total: sentinel_core::Cost::ZERO,
                    cap: sentinel_core::Cost::ZERO,
                },
            ],
            None,
        );
        assert_eq!(shown, Vec::new());
    }

    #[test]
    fn focus_hides_other_agents_cards_and_their_messages() {
        let events = vec![
            running("T-01"),
            routed("T-01", "claude"),
            running("T-02"),
            routed("T-02", "codex"),
            Event::Checkpoint {
                task: TaskId::new("T-02"),
                commit: "def5678".to_owned(),
                paths: vec![],
            },
            Event::Message {
                from: "Blair".to_owned(),
                to: Some(ToolId::new("codex")),
                text: "Use a smaller diff.".to_owned(),
            },
        ];
        let shown = items(events, Some("codex"));
        assert!(shown.iter().all(|item| !matches!(
            item,
            Item::Agent { agent, .. } if agent.as_str() == "claude"
        )));
        assert!(shown.iter().any(|item| matches!(
            item,
            Item::Agent { agent, .. } if agent.as_str() == "codex"
        )));
        assert!(
            shown
                .iter()
                .any(|item| matches!(item, Item::Checkpoint { .. }))
        );
        assert!(shown.iter().any(|item| matches!(item, Item::User { .. })));
        assert!(!shown.iter().any(|item| matches!(
            item,
            Item::System { text } if text.starts_with("T-01 goes to")
        )));
    }

    #[test]
    fn token_counts_show_thousands_with_k() {
        assert_eq!(token_label(52_000), "52k");
        assert_eq!(token_label(900), "900");
    }
}
