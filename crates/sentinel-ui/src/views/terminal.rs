//! The Terminal panel: raw output per agent, with a switcher and a load-earlier action
//! (`docs/ui-rules.md`, Views).

use std::collections::HashMap;

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::{Event, EventRecord, ReservationState, TaskId, ToolId, ToolInfo};

use crate::api;
use crate::components::kill_button::KillButton;
use crate::components::output_view::OutputView;
use crate::state::{ToastKind, Ui, use_ui};

/// The class of a line in `.term`. `Plain` has no class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineClass {
    Plain,
    Ok,
    Warn,
    Danger,
    Muted,
}

impl LineClass {
    /// The span class from `theme.css`, `.term .g` and friends.
    pub fn class_name(self) -> &'static str {
        match self {
            LineClass::Plain => "",
            LineClass::Ok => "g",
            LineClass::Warn => "y",
            LineClass::Danger => "r",
            LineClass::Muted => "m",
        }
    }
}

/// Picks a color from the words in a line. Errors win over warnings, and a `$ ` prompt is plain.
pub fn line_class(line: &str) -> LineClass {
    let lower = line.to_ascii_lowercase();
    if lower.contains("error") || lower.contains("failed") || lower.contains("panicked") {
        LineClass::Danger
    } else if lower.contains("warn") {
        LineClass::Warn
    } else if lower.contains("passed") || lower.starts_with("ok") {
        LineClass::Ok
    } else if line.starts_with("[sentinel]") {
        LineClass::Muted
    } else {
        LineClass::Plain
    }
}

/// Enabled tools that can take work, in list order.
pub fn ready_agents(tools: &[ToolInfo]) -> Vec<String> {
    tools
        .iter()
        .filter(|tool| tool.enabled && tool.status.is_ready())
        .map(|tool| tool.id.to_string())
        .collect()
}

/// The task that last printed output for this agent.
pub fn latest_task_for(agent: &ToolId, events: &[EventRecord]) -> Option<TaskId> {
    events.iter().rev().find_map(|record| {
        let Event::AgentOutput {
            task, agent: from, ..
        } = &record.event
        else {
            return None;
        };
        (from == agent).then(|| task.clone())
    })
}

/// The output lines of one agent, from loaded events, oldest first.
pub fn output_lines(records: &[EventRecord], agent: &str) -> Vec<String> {
    records
        .iter()
        .filter_map(|record| {
            let Event::AgentOutput {
                agent: from, line, ..
            } = &record.event
            else {
                return None;
            };
            (from.as_str() == agent).then(|| line.clone())
        })
        .collect()
}

/// The lines that came before the live buffer. The live buffer is the tail of the same output,
/// so the earlier part is the loaded list without its last `live.len()` lines.
pub fn earlier_lines(loaded: &[String], live: &[String]) -> Vec<String> {
    let keep = loaded.len().saturating_sub(live.len());
    loaded[..keep].to_vec()
}

/// Reservations this task holds right now. Shown on the kill confirmation.
pub fn held_files(task: &TaskId, reservations: &[sentinel_core::Reservation]) -> usize {
    reservations
        .iter()
        .filter(|r| &r.task_id == task && r.state == ReservationState::Granted)
        .count()
}

fn load_earlier(ui: Ui, earlier: RwSignal<HashMap<String, Vec<String>>>, agent: String) {
    let latest = ui
        .snapshot
        .with_untracked(|s| latest_task_for(&ToolId::new(agent.clone()), &s.events));
    let Some(task) = latest else {
        ui.show_toast(
            "No task has printed output for this agent yet.",
            ToastKind::Info,
        );
        return;
    };
    spawn_local(async move {
        let path = format!("/api/tasks/{task}/output?after=0&limit=2000");
        match api::get::<Vec<EventRecord>>(&path).await {
            Ok(records) => store_earlier(ui, earlier, &agent, &records),
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

/// Keeps the loaded lines that are not in the live buffer yet.
fn store_earlier(
    ui: Ui,
    earlier: RwSignal<HashMap<String, Vec<String>>>,
    agent: &str,
    records: &[EventRecord],
) {
    let loaded = output_lines(records, agent);
    let live = live_lines(ui, agent);
    let extra = earlier_lines(&loaded, &live);
    earlier.update(|map| {
        map.insert(agent.to_owned(), extra);
    });
}

fn live_lines(ui: Ui, agent: &str) -> Vec<String> {
    ui.terminal
        .with_untracked(|map| map.get(&ToolId::new(agent)).cloned().unwrap_or_default())
}

#[component]
pub fn TerminalPanel() -> impl IntoView {
    let ui = use_ui();
    let selected: RwSignal<Option<String>> = RwSignal::new(None);
    let earlier: RwSignal<HashMap<String, Vec<String>>> = RwSignal::new(HashMap::new());
    let agents = Memo::new(move |_| ui.snapshot.with(|s| ready_agents(&s.tools)));
    let current = Memo::new(move |_| {
        let list = agents.get();
        selected
            .get()
            .filter(|agent| list.contains(agent))
            .or_else(|| list.first().cloned())
    });
    let lines = Memo::new(move |_| {
        let Some(agent) = current.get() else {
            return Vec::new();
        };
        let mut all = earlier.with(|map| map.get(&agent).cloned().unwrap_or_default());
        all.extend(ui.terminal.with(|map| {
            map.get(&ToolId::new(agent.as_str()))
                .cloned()
                .unwrap_or_default()
        }));
        all
    });
    view! {
        <AgentChips agents=agents current=current selected=selected />
        <TerminalActions ui=ui current=current earlier=earlier />
        <OutputView lines=lines />
    }
}

/// The agent switcher: one chip per enabled, ready tool.
#[component]
fn AgentChips(
    agents: Memo<Vec<String>>,
    current: Memo<Option<String>>,
    selected: RwSignal<Option<String>>,
) -> impl IntoView {
    view! {
        <div class="row wrap" style="gap:.4em;margin-bottom:.7em">
            {move || {
                let active = current.get();
                agents.get()
                    .into_iter()
                    .map(|agent| {
                        let pressed = active.as_deref() == Some(agent.as_str());
                        let style = if pressed { "border-color:var(--accent)" } else { "" };
                        let label = agent.clone();
                        view! {
                            <button class="btn sm" style=style aria-pressed=pressed.to_string()
                                on:click=move |_| selected.set(Some(label.clone()))>
                                {agent}
                            </button>
                        }
                    })
                    .collect_view()
            }}
        </div>
    }
}

#[component]
fn TerminalActions(
    ui: Ui,
    current: Memo<Option<String>>,
    earlier: RwSignal<HashMap<String, Vec<String>>>,
) -> impl IntoView {
    view! {
        <div class="row wrap" style="gap:.4em;margin-bottom:.7em;justify-content:space-between">
            <button class="btn sm" on:click=move |_| {
                if let Some(agent) = current.get_untracked() {
                    load_earlier(ui, earlier, agent);
                }
            }>"Load earlier output"</button>
            {move || {
                let agent = current.get()?;
                let task = ui.snapshot.with(|s| latest_task_for(&ToolId::new(agent), &s.events))?;
                let files = ui.snapshot.with(|s| held_files(&task, &s.reservations));
                Some(view! { <KillButton task=task.to_string() files=files /> })
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{Tier, ToolStatus};
    use time::OffsetDateTime;

    fn output(id: i64, task: &str, agent: &str, line: &str) -> EventRecord {
        EventRecord {
            chat: None,
            id,
            at: OffsetDateTime::UNIX_EPOCH,
            event: Event::AgentOutput {
                task: TaskId::new(task),
                agent: ToolId::new(agent),
                line: line.to_owned(),
            },
        }
    }

    fn tool(id: &str, enabled: bool, status: ToolStatus) -> ToolInfo {
        ToolInfo {
            id: ToolId::new(id),
            name: id.to_owned(),
            path: String::new(),
            version: String::new(),
            adapter: "generic".to_owned(),
            status,
            tier: Tier::Cheap,
            enabled,
        }
    }

    #[test]
    fn errors_are_red_and_warnings_yellow() {
        assert_eq!(line_class("error[E0308]: mismatch"), LineClass::Danger);
        assert_eq!(line_class("warning: unused"), LineClass::Warn);
        assert_eq!(line_class("ok. 3 passed"), LineClass::Ok);
        assert_eq!(line_class("plain text"), LineClass::Plain);
    }

    #[test]
    fn a_line_with_error_and_warning_is_red() {
        assert_eq!(line_class("warning: error count 2"), LineClass::Danger);
    }

    #[test]
    fn only_enabled_ready_tools_are_switchable() {
        let tools = vec![
            tool("claude", true, ToolStatus::Ready),
            tool("codex", false, ToolStatus::Ready),
            tool("gemini", true, ToolStatus::NeedsLogin),
        ];
        assert_eq!(ready_agents(&tools), vec!["claude".to_owned()]);
    }

    #[test]
    fn latest_task_is_the_newest_output_for_the_agent() {
        let events = vec![
            output(1, "T-01", "claude", "a"),
            output(2, "T-02", "codex", "b"),
            output(3, "T-03", "claude", "c"),
        ];
        assert_eq!(
            latest_task_for(&ToolId::new("claude"), &events),
            Some(TaskId::new("T-03"))
        );
        assert_eq!(latest_task_for(&ToolId::new("gemini"), &events), None);
    }

    #[test]
    fn earlier_lines_drop_the_live_tail() {
        let loaded: Vec<String> = ["1", "2", "3", "4"].map(str::to_owned).to_vec();
        let live: Vec<String> = ["3", "4"].map(str::to_owned).to_vec();
        assert_eq!(earlier_lines(&loaded, &live), vec!["1", "2"]);
        assert_eq!(earlier_lines(&live, &loaded).len(), 0);
    }

    #[test]
    fn output_lines_keep_only_the_agent() {
        let records = vec![
            output(1, "T-01", "claude", "hi"),
            output(2, "T-01", "codex", "no"),
        ];
        assert_eq!(output_lines(&records, "claude"), vec!["hi".to_owned()]);
    }

    #[test]
    fn held_files_counts_only_granted_reservations() {
        let reservation = |state| sentinel_core::Reservation {
            id: sentinel_core::ReservationId::new(1),
            task_id: TaskId::new("T-01"),
            path: "src/a.rs".to_owned(),
            state,
            reserved_at: OffsetDateTime::UNIX_EPOCH,
            released_at: None,
        };
        let rows = [
            reservation(ReservationState::Granted),
            reservation(ReservationState::Waiting),
            reservation(ReservationState::Released),
        ];
        assert_eq!(held_files(&TaskId::new("T-01"), &rows), 1);
    }
}
