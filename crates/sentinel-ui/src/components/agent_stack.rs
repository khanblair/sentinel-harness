//! `AgentStack`: avatars of the ready agents in the top bar, each with a state dot. Opens the
//! agents popover.

use leptos::prelude::*;
use sentinel_core::{Event, Snapshot, TaskStatus, ToolId, ToolInfo};

use crate::components::popover::toggle_popover;
use crate::prefs::SWATCHES;
use crate::state::{Align, PopoverKind, use_ui};

/// Enabled agents whose tool is ready, in the order the daemon lists them.
pub fn ready_agents(snapshot: &Snapshot) -> Vec<&ToolInfo> {
    snapshot
        .tools
        .iter()
        .filter(|tool| tool.enabled && tool.status.is_ready())
        .collect()
}

/// `running` when the agent's latest output belongs to a task that is running, else `ready`.
pub fn agent_state(snapshot: &Snapshot, agent: &ToolId) -> &'static str {
    let latest_task = snapshot.events.iter().rev().find_map(|record| {
        if let Event::AgentOutput { task, agent: who, .. } = &record.event {
            (who == agent).then_some(task)
        } else {
            None
        }
    });
    let running = latest_task
        .and_then(|task| snapshot.tasks.iter().find(|candidate| candidate.id == *task))
        .is_some_and(|candidate| candidate.status == TaskStatus::Running);
    if running { "running" } else { "ready" }
}

/// The avatar letter: the first character of the agent id, in capitals.
pub fn avatar_letter(agent: &ToolId) -> char {
    agent
        .as_str()
        .chars()
        .next()
        .map_or('?', |letter| letter.to_ascii_uppercase())
}

/// A stable swatch for an agent, so its avatar keeps one color across renders.
pub fn avatar_color(agent: &ToolId) -> &'static str {
    let sum: usize = agent.as_str().bytes().map(usize::from).sum();
    SWATCHES[sum % SWATCHES.len()]
}

#[component]
pub fn AgentStack() -> impl IntoView {
    let ui = use_ui();
    view! {
        <button class="stack" id="agentStack" aria-label="Agents"
            on:click=move |_| toggle_popover(ui, "agentStack", Align::Left, PopoverKind::Agents)>
            {move || {
                let chips = ui.snapshot.with(|snapshot| {
                    ready_agents(snapshot)
                        .into_iter()
                        .map(|tool| Chip {
                            letter: avatar_letter(&tool.id),
                            color: avatar_color(&tool.id),
                            name: tool.name.clone(),
                            state: agent_state(snapshot, &tool.id),
                        })
                        .collect::<Vec<_>>()
                });
                chips.into_iter().map(avatar).collect_view()
            }}
        </button>
    }
}

/// One avatar, reduced to owned values so the view does not borrow the snapshot.
struct Chip {
    letter: char,
    color: &'static str,
    name: String,
    state: &'static str,
}

fn avatar(chip: Chip) -> impl IntoView {
    view! {
        <span class="av-a" style=format!("background:{}", chip.color) title=chip.name>
            {chip.letter.to_string()}
            <i class=chip.state></i>
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{EventRecord, Task, TaskId};
    use time::OffsetDateTime;

    fn output(task: &str, agent: &str, id: i64) -> EventRecord {
        EventRecord {
            id,
            at: OffsetDateTime::UNIX_EPOCH,
            event: Event::AgentOutput {
                task: TaskId::new(task),
                agent: ToolId::new(agent),
                line: "working".to_owned(),
            },
        }
    }

    fn task_with_status(id: &str, status: TaskStatus) -> Task {
        let mut task = Task::new(TaskId::new(id), "Title", "Body");
        task.status = status;
        task
    }

    #[test]
    fn agent_is_running_when_its_latest_output_task_runs() {
        let snapshot = Snapshot {
            events: vec![output("T1", "claude", 1)],
            tasks: vec![task_with_status("T1", TaskStatus::Running)],
            ..Snapshot::empty()
        };
        assert_eq!(agent_state(&snapshot, &ToolId::new("claude")), "running");
    }

    #[test]
    fn agent_is_ready_when_its_task_has_stopped_running() {
        let snapshot = Snapshot {
            events: vec![output("T1", "claude", 1)],
            tasks: vec![task_with_status("T1", TaskStatus::Verified)],
            ..Snapshot::empty()
        };
        assert_eq!(agent_state(&snapshot, &ToolId::new("claude")), "ready");
    }

    #[test]
    fn agent_with_no_output_is_ready() {
        assert_eq!(
            agent_state(&Snapshot::empty(), &ToolId::new("codex")),
            "ready"
        );
    }

    #[test]
    fn avatar_letter_is_the_capitalised_first_character() {
        assert_eq!(avatar_letter(&ToolId::new("claude")), 'C');
        assert_eq!(avatar_letter(&ToolId::new("")), '?');
    }

    #[test]
    fn avatar_color_is_stable_and_comes_from_the_swatches() {
        let first = avatar_color(&ToolId::new("claude"));
        assert_eq!(first, avatar_color(&ToolId::new("claude")));
        assert!(SWATCHES.contains(&first));
    }
}
