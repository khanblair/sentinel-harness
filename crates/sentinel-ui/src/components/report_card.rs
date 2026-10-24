//! The report card: counts by status from the snapshot, and buttons to the Report and the Brain.

use leptos::prelude::*;
use sentinel_core::{Task, TaskStatus};

use crate::components::icon::Icon;
use crate::components::status_badge::StatusBadge;
use crate::layout::PanelId;
use crate::state::use_ui;

/// How many tasks have each status, in the order each status first appears.
pub fn status_counts(tasks: &[Task]) -> Vec<(TaskStatus, usize)> {
    let mut counts: Vec<(TaskStatus, usize)> = Vec::new();
    for task in tasks {
        match counts.iter_mut().find(|(status, _)| *status == task.status) {
            Some((_, count)) => *count += 1,
            None => counts.push((task.status, 1)),
        }
    }
    counts
}

#[component]
pub fn ReportCard(verified: u32, total: u32) -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="card">
            <h4><Icon name="list" size=16 />" Honest report"</h4>
            <div class="row wrap" style="gap:.9em;margin:.2em 0 .6em">
                {move || ui.snapshot.with(|s| {
                    status_counts(&s.tasks)
                        .into_iter()
                        .map(|(status, count)| view! {
                            <span><StatusBadge status=status />" "<b>{count}</b></span>
                        })
                        .collect_view()
                })}
            </div>
            <div class="sub">{format!("{verified} of {total} tasks verified. The report gives each reason.")}</div>
            <div class="acts" style="display:flex;gap:.5em;margin-top:.7em">
                <button class="btn pri" on:click=move |_| ui.open_panel(PanelId::Report)>"Open report"</button>
                <button class="btn" on:click=move |_| ui.open_panel(PanelId::Brain)>"Open the Brain"</button>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::TaskId;

    fn task(id: &str, status: TaskStatus) -> Task {
        let mut task = Task::new(TaskId::new(id), "title", "body");
        task.status = status;
        task
    }

    #[test]
    fn counts_group_tasks_in_first_seen_order() {
        let tasks = vec![
            task("T-01", TaskStatus::Verified),
            task("T-02", TaskStatus::Failed),
            task("T-03", TaskStatus::Verified),
        ];
        assert_eq!(
            status_counts(&tasks),
            vec![(TaskStatus::Verified, 2), (TaskStatus::Failed, 1)]
        );
    }
}
