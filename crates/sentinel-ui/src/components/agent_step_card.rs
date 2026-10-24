//! One agent's card for one attempt at one task. The steps are not in the events, so the card
//! shows the attempt and its state.

use leptos::prelude::*;
use sentinel_core::{Snapshot, TaskId, ToolId};

use crate::app::agent_label;
use crate::components::agent_stack::{avatar_color, avatar_letter};
use crate::components::icon::Icon;
use crate::state::use_ui;
use crate::views::stream_model::AgentState;

struct AgentHeader {
    name: String,
    title: String,
}

fn header(snapshot: &Snapshot, task: &TaskId, agent: &ToolId) -> AgentHeader {
    let found = snapshot.tasks.iter().find(|item| item.id == *task);
    let title = found.map_or_else(String::new, |item| item.title.clone());
    let attempt = found
        .filter(|item| item.attempts > 1)
        .map_or_else(String::new, |item| format!(" · attempt {}", item.attempts));
    AgentHeader {
        name: agent_label(&snapshot.tools, agent),
        title: format!("{task} {title}{attempt}"),
    }
}

fn state_badge(state: AgentState) -> AnyView {
    match state {
        AgentState::Running => view! { <span class="spin"></span> }.into_any(),
        AgentState::Done => view! {
            <span class="stb" style="color:var(--ok)"><Icon name="check" size=13 />" Done"</span>
        }
        .into_any(),
        AgentState::Failed => view! {
            <span class="stb" style="color:var(--danger)"><Icon name="x" size=13 />" Failed"</span>
        }
        .into_any(),
        AgentState::Killed => view! {
            <span class="stb" style="color:var(--danger)"><Icon name="stop" size=12 />" Killed"</span>
        }
        .into_any(),
    }
}

#[component]
pub fn AgentStepCard(task: TaskId, agent: ToolId, state: AgentState) -> impl IntoView {
    let ui = use_ui();
    let style = format!("background:{};margin:0", avatar_color(&agent));
    let letter = avatar_letter(&agent).to_string();
    let working = (state == AgentState::Running).then(|| {
        view! {
            <div class="step">
                <span class="kd"><span class="spin"></span></span>
                <span class="tx">"working"</span>
            </div>
        }
    });
    view! {
        <div class="card agent">
            <div class="hd">
                <span class="av-a" style=style>{letter}</span>
                {move || {
                    let head = ui.snapshot.with(|s| header(s, &task, &agent));
                    view! {
                        <span class="nm">{head.name}</span>
                        <span class="tt">{head.title}</span>
                    }
                }}
                {state_badge(state)}
            </div>
            {working}
        </div>
    }
}
