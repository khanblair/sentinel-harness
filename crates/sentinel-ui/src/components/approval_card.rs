//! An approval card: an agent asks for one more file, or the user approves a wave or the plan.
//! The card reads its live state from the snapshot. Pending shows the buttons, answered shows one
//! line.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::AnswerApproval;
use sentinel_core::{Approval, ApprovalId, ApprovalKind, ApprovalState, Snapshot};

use crate::api;
use crate::app::agent_label;
use crate::components::icon::Icon;
use crate::components::top_bar::{post_action, settle};
use crate::state::{Ui, use_ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalAction {
    Answer(bool),
    StopRun,
}

/// The state line: `pending`, `allowed by NAME`, `denied by NAME`, or `defaulted`.
pub fn approval_status(state: ApprovalState, decided_by: Option<&str>) -> String {
    match state {
        ApprovalState::Pending => "pending".to_owned(),
        ApprovalState::Allowed => with_name("allowed", decided_by),
        ApprovalState::Denied => with_name("denied", decided_by),
        ApprovalState::Defaulted => "defaulted".to_owned(),
    }
}

fn with_name(verb: &str, decided_by: Option<&str>) -> String {
    match decided_by {
        Some(name) => format!("{verb} by {name}"),
        None => verb.to_owned(),
    }
}

/// Button label, action, and class for each kind. Verbs name the effect.
pub fn approval_buttons(kind: ApprovalKind) -> Vec<(&'static str, ApprovalAction, &'static str)> {
    match kind {
        ApprovalKind::Wave => vec![
            ("Approve wave", ApprovalAction::Answer(true), "btn pri"),
            ("Stop run", ApprovalAction::StopRun, "btn dng"),
        ],
        ApprovalKind::Plan => vec![
            ("Approve plan", ApprovalAction::Answer(true), "btn pri"),
            ("Stop run", ApprovalAction::StopRun, "btn dng"),
        ],
        ApprovalKind::Extension | ApprovalKind::Decision => vec![
            ("Allow", ApprovalAction::Answer(true), "btn pri"),
            ("Deny", ApprovalAction::Answer(false), "btn"),
        ],
    }
}

/// The title and the reason for each kind. The reason says what is asked and why.
pub fn approval_copy(approval: &Approval, agent: &str) -> (String, String) {
    let path = approval.path.as_deref().unwrap_or("A file");
    let task = approval
        .task
        .as_ref()
        .map_or("this task", |task| task.as_str());
    match approval.kind {
        ApprovalKind::Extension => (
            format!("{agent} asks for one more file"),
            format!("{path} is not reserved for {task}. Allow it only if the task needs it."),
        ),
        ApprovalKind::Wave => (
            "Approve the next wave".to_owned(),
            "The tasks in the next wave start writing their reserved files.".to_owned(),
        ),
        ApprovalKind::Plan => (
            "Approve the plan".to_owned(),
            "Agents write nothing until you approve this plan.".to_owned(),
        ),
        ApprovalKind::Decision => (
            "Answer this decision".to_owned(),
            "Sentinel needs your choice before it goes on.".to_owned(),
        ),
    }
}

#[derive(Clone)]
struct ApprovalView {
    title: String,
    body: String,
    status: String,
    pending: bool,
    allowed: bool,
    kind: ApprovalKind,
}

fn approval_view(snapshot: &Snapshot, id: &ApprovalId) -> Option<ApprovalView> {
    let approval = snapshot.approvals.iter().find(|item| item.id == *id)?;
    let agent = approval.agent.as_ref().map_or_else(
        || "An agent".to_owned(),
        |agent| agent_label(&snapshot.tools, agent),
    );
    let (title, body) = approval_copy(approval, &agent);
    Some(ApprovalView {
        title,
        body,
        status: approval_status(approval.state, approval.decided_by.as_deref()),
        pending: approval.state == ApprovalState::Pending,
        allowed: approval.state == ApprovalState::Allowed,
        kind: approval.kind,
    })
}

fn act(ui: Ui, id: &ApprovalId, action: ApprovalAction) {
    match action {
        ApprovalAction::StopRun => post_action(ui, "/api/stop".to_owned()),
        ApprovalAction::Answer(allow) => answer(ui, id, allow),
    }
}

fn answer(ui: Ui, id: &ApprovalId, allow: bool) {
    let path = format!("/api/approvals/{id}");
    spawn_local(async move {
        let result = api::post::<_, serde_json::Value>(&path, &AnswerApproval { allow }).await;
        settle(ui, result);
    });
}

fn answered(view: &ApprovalView) -> AnyView {
    let icon = if view.allowed { "check" } else { "x" };
    let line = format!("{}: {}", view.title, view.status);
    view! {
        <div class="card">
            <div class="row"><Icon name=icon size=15 /><span>{line}</span></div>
        </div>
    }
    .into_any()
}

fn pending(ui: Ui, id: &ApprovalId, view: &ApprovalView) -> AnyView {
    let buttons = approval_buttons(view.kind)
        .into_iter()
        .map(|(label, action, class)| {
            let id = id.clone();
            view! { <button class=class on:click=move |_| act(ui, &id, action)>{label}</button> }
        })
        .collect_view();
    view! {
        <div class="card apv">
            <h4><Icon name="lock" size=16 />{format!(" {}", view.title)}</h4>
            <div>{view.body.clone()}</div>
            <div class="acts">{buttons}</div>
        </div>
    }
    .into_any()
}

#[component]
pub fn ApprovalCard(id: ApprovalId) -> impl IntoView {
    let ui = use_ui();
    move || {
        let Some(view) = ui.snapshot.with(|s| approval_view(s, &id)) else {
            return ().into_any();
        };
        if view.pending {
            pending(ui, &id, &view)
        } else {
            answered(&view)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reads_pending_allowed_denied_or_defaulted() {
        assert_eq!(approval_status(ApprovalState::Pending, None), "pending");
        assert_eq!(
            approval_status(ApprovalState::Allowed, Some("Blair's phone")),
            "allowed by Blair's phone"
        );
        assert_eq!(
            approval_status(ApprovalState::Denied, Some("desktop")),
            "denied by desktop"
        );
        assert_eq!(approval_status(ApprovalState::Defaulted, None), "defaulted");
    }

    #[test]
    fn allowed_without_a_device_name_says_only_allowed() {
        assert_eq!(approval_status(ApprovalState::Allowed, None), "allowed");
    }

    #[test]
    fn a_wave_card_offers_approve_and_stop() {
        let labels: Vec<&str> = approval_buttons(ApprovalKind::Wave)
            .into_iter()
            .map(|(label, _, _)| label)
            .collect();
        assert_eq!(labels, vec!["Approve wave", "Stop run"]);
    }

    #[test]
    fn a_file_request_offers_allow_and_deny() {
        let buttons = approval_buttons(ApprovalKind::Extension);
        assert_eq!(buttons[0].1, ApprovalAction::Answer(true));
        assert_eq!(buttons[1].1, ApprovalAction::Answer(false));
    }
}
