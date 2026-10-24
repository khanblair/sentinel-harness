//! A decision card: two tasks contradict each other, and only the user can choose which goes on.
//! Pending shows one button per task and "Skip both". Answered shows one line.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::AnswerDecision;
use sentinel_core::{Decision, DecisionId, DecisionState, Snapshot, TaskId};

use crate::api;
use crate::components::icon::Icon;
use crate::components::top_bar::settle;
use crate::state::{Ui, use_ui};

/// The answer the daemon takes for "skip both tasks".
pub const SKIP_BOTH: &str = "skip_both";

/// The one-line result of a decision. Pending gives no line.
pub fn decision_line(state: DecisionState, answer: Option<&str>) -> String {
    match (state, answer) {
        (DecisionState::Pending, _) => "Waiting for your choice.".to_owned(),
        (DecisionState::Defaulted, _) => "No answer in time. Both tasks were skipped.".to_owned(),
        (DecisionState::Answered, Some(SKIP_BOTH)) => "You skipped both tasks.".to_owned(),
        (DecisionState::Answered, Some(task)) => format!("You chose {task}."),
        (DecisionState::Answered, None) => "Answered.".to_owned(),
    }
}

#[derive(Clone)]
struct DecisionView {
    prompt: String,
    tasks: Vec<TaskId>,
    pending: bool,
    line: String,
}

fn view_of(decision: &Decision) -> DecisionView {
    DecisionView {
        prompt: decision.prompt.clone(),
        tasks: decision.tasks.clone(),
        pending: decision.state == DecisionState::Pending,
        line: decision_line(decision.state, decision.answer.as_deref()),
    }
}

fn decision_view(snapshot: &Snapshot, id: &DecisionId) -> Option<DecisionView> {
    snapshot
        .decisions
        .iter()
        .find(|item| item.id == *id)
        .map(view_of)
}

fn answer(ui: Ui, id: &DecisionId, choice: String) {
    let path = format!("/api/decisions/{id}");
    spawn_local(async move {
        let body = AnswerDecision { answer: choice };
        settle(ui, api::post::<_, serde_json::Value>(&path, &body).await);
    });
}

fn choices(ui: Ui, id: &DecisionId, view: &DecisionView) -> AnyView {
    let mut buttons: Vec<AnyView> = view
        .tasks
        .iter()
        .map(|task| choice_button(ui, id, task.as_str().to_owned(), task.to_string()))
        .collect();
    buttons.push(choice_button(
        ui,
        id,
        SKIP_BOTH.to_owned(),
        "Skip both".to_owned(),
    ));
    buttons.collect_view().into_any()
}

fn choice_button(ui: Ui, id: &DecisionId, choice: String, label: String) -> AnyView {
    let id = id.clone();
    view! {
        <button class="btn" on:click=move |_| answer(ui, &id, choice.clone())>{label}</button>
    }
    .into_any()
}

#[component]
pub fn DecisionCard(id: DecisionId) -> impl IntoView {
    let ui = use_ui();
    move || {
        let Some(view) = ui.snapshot.with(|s| decision_view(s, &id)) else {
            return ().into_any();
        };
        if !view.pending {
            return view! {
                <div class="card">
                    <div class="row"><Icon name="check" size=15 /><span>{view.line}</span></div>
                </div>
            }
            .into_any();
        }
        let buttons = choices(ui, &id, &view);
        view! {
            <div class="card apv">
                <h4><Icon name="branch" size=16 />" Pick one task"</h4>
                <div>{view.prompt}</div>
                <div class="acts">{buttons}</div>
            </div>
        }
        .into_any()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_lines_say_what_was_chosen() {
        assert_eq!(
            decision_line(DecisionState::Answered, Some("T-01")),
            "You chose T-01."
        );
        assert_eq!(
            decision_line(DecisionState::Answered, Some(SKIP_BOTH)),
            "You skipped both tasks."
        );
        assert!(decision_line(DecisionState::Defaulted, None).contains("Both tasks were skipped"));
        assert!(decision_line(DecisionState::Pending, None).starts_with("Waiting"));
    }
}
