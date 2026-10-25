//! Which agent runs a task and how its attempt ended, read from the events. Pure, like the stream
//! model that uses it.

use sentinel_core::{Event, EventRecord, TaskId, TaskStatus, ToolId};

use super::stream_model::AgentState;

/// The agent a message such as "T-01 goes to claude" names for this task.
pub fn goes_to_agent(events: &[EventRecord], task: &TaskId) -> Option<ToolId> {
    events
        .iter()
        .rev()
        .find_map(|record| routed_agent(&record.event, task))
}

pub(super) fn agent_after(rest: &[EventRecord], task: &TaskId) -> Option<ToolId> {
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

pub(super) fn agent_state(rest: &[EventRecord], task: &TaskId) -> AgentState {
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
