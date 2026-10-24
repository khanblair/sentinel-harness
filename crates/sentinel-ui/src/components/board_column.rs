//! `BoardColumn`: one status group on the Board, with its heading and cards.

use leptos::prelude::*;

use crate::components::task_card::{CardModel, TaskCard};

/// One status group. `label` is the `TaskStatus::label()` text it shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub label: &'static str,
    pub cards: Vec<CardModel>,
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
