//! The Board panel: tasks grouped by status (`docs/ui-rules.md`, Views).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::BacklogTask;
use sentinel_core::{Event, EventRecord, QualityNote, Task, TaskId, ToolId};

use crate::api;
use crate::components::attach_menu::backlog_name;
use crate::components::board_column::{BoardColumn, Column, PlannedColumn, planned_line};
use crate::components::task_card::CardModel;
use crate::state::{Ui, use_ui};
use crate::views::live_fetch::keep_live;

/// Column order from the prototype. Each label groups several statuses (`TaskStatus::label`).
const COLUMN_ORDER: [&str; 9] = [
    "Running",
    "Waiting",
    "Verifying",
    "Ready",
    "Verified",
    "Blocked",
    "Contradiction",
    "Skipped",
    "Partial",
];

/// The agent a task was sent to, from the latest `X goes to AGENT` message.
pub fn owner_of(task: &TaskId, events: &[EventRecord]) -> Option<ToolId> {
    let prefix = format!("{task} goes to ");
    events.iter().rev().find_map(|record| {
        let Event::Message {
            to: Some(tool),
            text,
            ..
        } = &record.event
        else {
            return None;
        };
        text.starts_with(&prefix).then(|| tool.clone())
    })
}

/// How many quality notes the event log holds for one task.
pub fn quality_count(task: &TaskId, events: &[EventRecord]) -> usize {
    events
        .iter()
        .filter(|record| {
            matches!(&record.event, Event::QualityNote { note: QualityNote { task: owner, .. } } if owner == task)
        })
        .count()
}

/// A short flag for the card: a suspected duplicate, or a contradiction.
pub fn flag_text(task: &Task) -> Option<String> {
    if task.flags.already_done {
        return Some("Already done?".to_owned());
    }
    match task.flags.contradicts.len() {
        0 => None,
        count => Some(format!("Contradicts {count}")),
    }
}

pub fn card_model(task: &Task, events: &[EventRecord]) -> CardModel {
    CardModel {
        id: task.id.to_string(),
        title: task.title.clone(),
        body: task.body.clone(),
        status: task.status,
        tier: task.tier.ui_label(),
        owner: owner_of(&task.id, events).map(|tool| tool.to_string()),
        attempt: task.attempts,
        paths: task.scope.entries().map(str::to_owned).collect(),
        notes: quality_count(&task.id, events),
        flag: flag_text(task),
    }
}

/// The non-empty status groups, in prototype order.
pub fn columns(tasks: &[Task], events: &[EventRecord]) -> Vec<Column> {
    COLUMN_ORDER
        .iter()
        .filter_map(|label| {
            let cards: Vec<CardModel> = tasks
                .iter()
                .filter(|task| task.status.label() == *label)
                .map(|task| card_model(task, events))
                .collect();
            (!cards.is_empty()).then_some(Column { label, cards })
        })
        .collect()
}

/// Loads the backlog tasks that the Planned column shows before a run plans them.
fn load_planned(planned: RwSignal<Vec<BacklogTask>>) {
    spawn_local(async move {
        if let Ok(list) = api::get::<Vec<BacklogTask>>("/api/backlog/tasks").await {
            planned.set(list);
        }
    });
}

/// What the Board shows while no run has planned tasks: the backlog as Planned cards, or a note
/// when the backlog has no tasks.
fn planned_board(ui: Ui, planned: RwSignal<Vec<BacklogTask>>) -> impl IntoView {
    move || {
        let tasks = planned.get();
        if tasks.is_empty() {
            view! { <div class="note">"No tasks yet. Start a run to plan the backlog."</div> }
                .into_any()
        } else {
            let backlog = ui.snapshot.with(|s| backlog_name(&s.settings));
            let line = planned_line(&backlog);
            view! { <PlannedColumn line=line tasks=tasks /> }.into_any()
        }
    }
}

#[component]
pub fn BoardPanel() -> impl IntoView {
    let ui = use_ui();
    let planned = RwSignal::new(Vec::<BacklogTask>::new());
    keep_live(ui, move || load_planned(planned));
    let has_tasks = move || ui.snapshot.with(|snapshot| !snapshot.tasks.is_empty());
    view! {
        <Show when=has_tasks fallback=move || planned_board(ui, planned)>
            {move || {
                ui.snapshot
                    .with(|snapshot| columns(&snapshot.tasks, &snapshot.events))
                    .into_iter()
                    .map(|column| view! { <BoardColumn column=column /> })
                    .collect_view()
            }}
        </Show>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::TaskStatus;
    use time::OffsetDateTime;

    fn task(id: &str, status: TaskStatus) -> Task {
        let mut task = Task::new(TaskId::new(id), "title", "body");
        task.status = status;
        task
    }

    fn record(id: i64, event: Event) -> EventRecord {
        EventRecord {
            chat: None,
            id,
            at: OffsetDateTime::UNIX_EPOCH,
            event,
        }
    }

    fn goes_to(task: &str, agent: &str) -> EventRecord {
        record(
            1,
            Event::Message {
                from: "sentinel".to_owned(),
                to: Some(ToolId::new(agent)),
                text: format!("{task} goes to Agent (easy task)."),
            },
        )
    }

    #[test]
    fn owner_is_the_latest_agent_the_task_went_to() {
        let events = vec![goes_to("T-01", "claude"), goes_to("T-01", "codex")];
        assert_eq!(
            owner_of(&TaskId::new("T-01"), &events),
            Some(ToolId::new("codex"))
        );
    }

    #[test]
    fn owner_ignores_messages_about_other_tasks() {
        let events = vec![goes_to("T-02", "claude")];
        assert_eq!(owner_of(&TaskId::new("T-01"), &events), None);
    }

    #[test]
    fn planned_and_ready_share_the_ready_column() {
        let tasks = vec![
            task("T-01", TaskStatus::Planned),
            task("T-02", TaskStatus::Ready),
        ];
        let cols = columns(&tasks, &[]);
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].label, "Ready");
        assert_eq!(cols[0].cards.len(), 2);
    }

    #[test]
    fn reserved_and_running_share_the_running_column() {
        let tasks = vec![
            task("T-01", TaskStatus::Reserved),
            task("T-02", TaskStatus::Running),
        ];
        let cols = columns(&tasks, &[]);
        assert_eq!(cols[0].label, "Running");
        assert_eq!(cols[0].cards.len(), 2);
    }

    #[test]
    fn failed_tasks_land_in_blocked() {
        let tasks = vec![task("T-01", TaskStatus::Failed)];
        let cols = columns(&tasks, &[]);
        assert_eq!(cols[0].label, "Blocked");
    }

    #[test]
    fn empty_groups_are_left_out_and_order_follows_the_prototype() {
        let tasks = vec![
            task("T-01", TaskStatus::Verified),
            task("T-02", TaskStatus::Waiting),
        ];
        let labels: Vec<&str> = columns(&tasks, &[]).iter().map(|c| c.label).collect();
        assert_eq!(labels, vec!["Waiting", "Verified"]);
    }

    #[test]
    fn quality_notes_count_per_task() {
        let note = QualityNote {
            task: TaskId::new("T-01"),
            kind: sentinel_core::QualityKind::Size,
            location: "src/a.rs:4".to_owned(),
            detail_code: "long_function".to_owned(),
        };
        let events = vec![record(1, Event::QualityNote { note })];
        assert_eq!(quality_count(&TaskId::new("T-01"), &events), 1);
        assert_eq!(quality_count(&TaskId::new("T-02"), &events), 0);
    }
}
