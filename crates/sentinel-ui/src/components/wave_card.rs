//! The wave card: the tasks that run together, each with the agent it was routed to.

use leptos::prelude::*;
use sentinel_core::{Snapshot, TaskId, ToolId};

use crate::components::agent_stack::{avatar_color, avatar_letter};
use crate::components::icon::Icon;
use crate::state::use_ui;
use crate::views::stream_model::goes_to_agent;

/// One task row in a wave, read from the snapshot.
pub struct WaveRow {
    pub id: TaskId,
    pub title: String,
    pub scope: usize,
    pub agent: Option<ToolId>,
}

pub fn wave_rows(snapshot: &Snapshot, ids: &[TaskId]) -> Vec<WaveRow> {
    ids.iter()
        .map(|id| {
            let task = snapshot.tasks.iter().find(|task| task.id == *id);
            WaveRow {
                id: id.clone(),
                title: task.map_or_else(String::new, |task| task.title.clone()),
                scope: task.map_or(0, |task| task.scope.entries().count()),
                agent: goes_to_agent(&snapshot.events, id),
            }
        })
        .collect()
}

#[component]
pub fn WaveCard(wave: u32, tasks: Vec<TaskId>) -> impl IntoView {
    let ui = use_ui();
    let summary = format!(
        "{} tasks run together. Their reserved files do not overlap.",
        tasks.len()
    );
    view! {
        <div class="card">
            <h4><Icon name="branch" size=16 />{format!(" Wave {wave}")}</h4>
            <div class="sub" style="margin-bottom:.5em">{summary}</div>
            {move || ui.snapshot.with(|s| wave_rows(s, &tasks).into_iter().map(wave_row).collect_view())}
        </div>
    }
}

fn wave_row(row: WaveRow) -> impl IntoView {
    let avatar = row.agent.map(|agent| {
        let style = format!("background:{};margin:0", avatar_color(&agent));
        view! { <span class="av-a" style=style>{avatar_letter(&agent).to_string()}</span> }
    });
    view! {
        <div class="row" style="padding:.25em 0">
            {avatar}
            <span class="mono">{row.id.to_string()}</span>
            <span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{row.title}</span>
            <span class="chip"><Icon name="lock" size=11 />{format!(" {}", row.scope)}</span>
        </div>
    }
}
