//! `BoardColumn`: one status group on the Board, with its heading and cards. `PlannedColumn` is the
//! same shape for the backlog tasks that no run has planned yet.

use leptos::prelude::*;
use sentinel_core::api::BacklogTask;

use crate::components::task_card::{CardModel, PlannedCard, TaskCard};

/// One status group. `label` is the `TaskStatus::label()` text it shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub label: &'static str,
    pub cards: Vec<CardModel>,
}

/// The line under the Planned heading: where the tasks come from, and what a run does with them.
pub fn planned_line(backlog: &str) -> String {
    format!("From {backlog}. A run plans them and reserves their files.")
}

#[component]
pub fn BoardColumn(column: Column) -> impl IntoView {
    let Column { label, cards } = column;
    let heading = format!("{label} · {}", cards.len());
    view! {
        <div class="psec">
            <h5>{heading}</h5>
            {cards.into_iter().map(|card| view! { <TaskCard card=card /> }).collect_view()}
        </div>
    }
}

#[component]
pub fn PlannedColumn(line: String, tasks: Vec<BacklogTask>) -> impl IntoView {
    let heading = format!("Planned · {}", tasks.len());
    view! {
        <div class="psec">
            <h5>{heading}</h5>
            <div class="sub" style="margin-bottom:.5em">{line}</div>
            {tasks.into_iter().map(|task| view! { <PlannedCard task=task /> }).collect_view()}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_planned_line_names_the_backlog_file() {
        assert_eq!(
            planned_line("backlog.md"),
            "From backlog.md. A run plans them and reserves their files."
        );
    }
}
