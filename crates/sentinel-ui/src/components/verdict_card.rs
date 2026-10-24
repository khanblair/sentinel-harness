//! A verdict card: the task, its status, the reason, and the quality notes for the task.
//! Warnings never change the verdict, and the card says so.

use leptos::prelude::*;
use sentinel_core::{Event, EventRecord, Snapshot, TaskId, TaskStatus};

use crate::components::icon::Icon;
use crate::components::status_badge::StatusBadge;
use crate::state::use_ui;

pub fn quality_note_count(events: &[EventRecord], task: &TaskId) -> usize {
    events
        .iter()
        .filter(|record| is_note_for(&record.event, task))
        .count()
}

fn is_note_for(event: &Event, task: &TaskId) -> bool {
    matches!(event, Event::QualityNote { note } if note.task == *task)
}

pub fn note_label(count: usize) -> String {
    if count == 1 {
        "1 quality note".to_owned()
    } else {
        format!("{count} quality notes")
    }
}

fn task_title(snapshot: &Snapshot, task: &TaskId) -> String {
    snapshot
        .tasks
        .iter()
        .find(|item| item.id == *task)
        .map_or_else(String::new, |item| item.title.clone())
}

#[component]
pub fn VerdictCard(task: TaskId, status: TaskStatus, reason: String) -> impl IntoView {
    let ui = use_ui();
    let title = ui.snapshot.with(|s| task_title(s, &task));
    let label = task.to_string();
    view! {
        <div class="card">
            <div class="row sp wrap">
                <span class="row"><span class="mono">{label}</span><span>{title}</span></span>
                <StatusBadge status=status />
            </div>
            {move || notes_block(ui.snapshot.with(|s| quality_note_count(&s.events, &task)))}
            {(!reason.is_empty()).then(|| view! { <div class="sub" style="margin-top:.4em">{reason}</div> })}
        </div>
    }
}

fn notes_block(count: usize) -> Option<AnyView> {
    (count > 0).then(|| {
        view! {
            <div class="row wrap" style="margin-top:.5em;gap:.35em">
                <span class="chip" style="color:var(--warn)">
                    <Icon name="alert" size=11 />{format!(" {}", note_label(count))}
                </span>
            </div>
            <div class="sub" style="margin-top:.3em">"Warnings never change the verdict."</div>
        }
        .into_any()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::QualityKind;
    use sentinel_core::QualityNote;
    use time::OffsetDateTime;

    fn note_for(task: &str) -> EventRecord {
        EventRecord {
            id: 1,
            at: OffsetDateTime::UNIX_EPOCH,
            event: Event::QualityNote {
                note: QualityNote {
                    task: TaskId::new(task),
                    kind: QualityKind::Size,
                    location: "src/a.rs:10".to_owned(),
                    detail_code: "long_function".to_owned(),
                },
            },
        }
    }

    #[test]
    fn notes_count_only_the_given_task() {
        let events = vec![note_for("T-01"), note_for("T-01"), note_for("T-02")];
        assert_eq!(quality_note_count(&events, &TaskId::new("T-01")), 2);
        assert_eq!(quality_note_count(&events, &TaskId::new("T-09")), 0);
    }

    #[test]
    fn note_label_uses_the_singular_for_one() {
        assert_eq!(note_label(1), "1 quality note");
        assert_eq!(note_label(3), "3 quality notes");
    }

    #[test]
    fn a_task_without_a_title_in_the_snapshot_gets_an_empty_title() {
        assert_eq!(task_title(&Snapshot::empty(), &TaskId::new("T-01")), "");
    }
}
