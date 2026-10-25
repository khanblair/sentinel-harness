//! The composer's send path: a plain message starts a run or goes to the live run, a slash
//! command maps to one API route, and attachments travel with the run request.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::{AddAttachment, LowerCap, PermissionsUpdate, RunRequest, SetBrainMode};
use sentinel_core::skills::SkillInfo;
use sentinel_core::{
    AttachmentId, AttachmentKind, BrainMode, ChatId, Intent, PermissionMode, ToolId,
};
use serde::Serialize;

use crate::api::{self, ApiFailure};
use crate::chats::busy_notice;
use crate::components::agent_chip::{AgentCommand, choose_chat_agent, parse_agent_command};
use crate::components::top_bar::{post_action, settle};
use crate::layout::PanelId;
use crate::skills::unknown_skill_text;
use crate::slash::{self, SlashCommand};
use crate::state::{Connection, Target, ToastKind, Ui, refresh};
use crate::views::composer::{Draft, SendRoute, reload_attachments, send_route};

const AGENT_NOTE: &str = "Applied on the agent's next step. Reserved files do not change.";
/// A type rides on a run's request. A note to the live run has no request, so it refuses a type.
const TYPE_NOTE: &str = "The type goes with a new run. Send it when the run ends.";
const RESEND_WAIT: &str = "A run is live. Send the message again when it ends.";

/// The body of `POST /api/message`.
#[derive(Serialize)]
struct MessageRequest {
    text: String,
    to: Option<ToolId>,
}

pub fn submit(ui: Ui, draft: Draft) {
    let message = draft.text.get_untracked().trim().to_owned();
    let has_chips = ui.attachments.with_untracked(|list| !list.is_empty());
    if message.is_empty() && !has_chips {
        return;
    }
    if matches!(ui.connection.get_untracked(), Connection::Offline { .. }) {
        // The page is read-only while it shows the last state. The draft stays.
        ui.show_toast(
            "You are offline. Send it when the page reconnects.",
            ToastKind::Warn,
        );
        return;
    }
    if let Some((intent, text)) = slash::parse_intent_command(&message) {
        typed_command(ui, draft, intent, text);
        return;
    }
    if slash::parse(&message).is_some() {
        clear_draft(draft);
        run_slash(ui, &message);
        return;
    }
    let intent = ui.next_intent.get_untracked();
    send_message(ui, draft, message, intent);
}

/// `/ask` alone sets the type of the next message. `/ask text` sends the text with that type.
fn typed_command(ui: Ui, draft: Draft, intent: Intent, text: String) {
    if text.is_empty() {
        ui.next_intent.set(Some(intent));
        clear_draft(draft);
    } else {
        send_message(ui, draft, text, Some(intent));
    }
}

pub fn clear_draft(draft: Draft) {
    draft.text.set(String::new());
    draft.dismissed.set(false);
}

/// Runs a slash command. A bad command shows its problem as a toast.
pub fn run_slash(ui: Ui, message: &str) {
    match slash::parse(message) {
        Some(Ok(command)) => run_command(ui, command),
        Some(Err(problem)) => ui.show_toast(problem, ToastKind::Warn),
        None => {}
    }
}

/// Sends a plain message, or one with a type. The draft and the chips stay when the send is
/// refused or fails.
pub fn send_message(ui: Ui, draft: Draft, text: String, intent: Option<Intent>) {
    let open = ui.open_chat.get_untracked();
    let (harness, live) = ui
        .snapshot
        .with_untracked(|s| (s.harness, s.run.as_ref().map(|run| run.chat.clone())));
    match send_route(harness, live.as_ref(), open.as_ref()) {
        SendRoute::StartRun => start_with_chips(ui, draft, text, intent),
        SendRoute::Busy => {
            let notice = ui
                .snapshot
                .with_untracked(|s| busy_notice(s, live.as_ref()));
            ui.show_toast(notice, ToastKind::Warn);
        }
        SendRoute::NoteToAgents if intent.is_some() => ui.show_toast(TYPE_NOTE, ToastKind::Warn),
        SendRoute::NoteToAgents => send_note(ui, draft, &text),
    }
}

pub fn pending_ids(ui: Ui) -> Vec<AttachmentId> {
    ui.attachments.with_untracked(|list| {
        list.iter()
            .map(|attachment| attachment.id.clone())
            .collect()
    })
}

/// Starts a run in the open chat with the pending chips. A backlog chip with no text is a run
/// with no instruction. After the daemon accepts, the draft, the chips, and the one-shot type
/// clear.
pub fn start_with_chips(ui: Ui, draft: Draft, text: String, intent: Option<Intent>) {
    let request = RunRequest {
        chat: ui.open_chat.get_untracked(),
        text: (!text.is_empty()).then_some(text),
        attachments: pending_ids(ui),
        intent,
        ..RunRequest::default()
    };
    spawn_local(async move {
        match api::start_run(&request).await {
            Ok(_) => {
                clear_draft(draft);
                ui.attachments.set(Vec::new());
                ui.next_intent.set(None);
                refresh(ui);
            }
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

/// Sends `text` again as a new run with `intent`. Only a free daemon takes it, so a live run
/// refuses. The pending chips stay with the composer.
pub fn resend_as(ui: Ui, intent: Intent, text: String) {
    let open = ui.open_chat.get_untracked();
    let (harness, live) = ui
        .snapshot
        .with_untracked(|s| (s.harness, s.run.as_ref().map(|run| run.chat.clone())));
    if send_route(harness, live.as_ref(), open.as_ref()) != SendRoute::StartRun {
        ui.show_toast(RESEND_WAIT, ToastKind::Warn);
        return;
    }
    let request = RunRequest {
        chat: open,
        text: Some(text),
        intent: Some(intent),
        ..RunRequest::default()
    };
    spawn_local(async move {
        match api::start_run(&request).await {
            Ok(_) => refresh(ui),
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

/// Sends a message to the live run: to Sentinel, or to the agent the target chip chose.
pub fn send_note(ui: Ui, draft: Draft, text: &str) {
    if text.is_empty() {
        return;
    }
    let target = ui.target.get_untracked();
    let to = match &target {
        Target::Sentinel => None,
        Target::Agent(id) => Some(id.clone()),
    };
    if matches!(target, Target::Agent(_)) {
        ui.show_toast(AGENT_NOTE, ToastKind::Info);
    }
    let request = MessageRequest {
        text: text.to_owned(),
        to,
    };
    spawn_local(async move {
        match api::post::<_, serde_json::Value>("/api/message", &request).await {
            Ok(_) => {
                clear_draft(draft);
                refresh(ui);
            }
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

/// Starts a run, optionally in plan-only mode, and optionally sends one instruction to Sentinel
/// once the run has started.
pub fn launch(ui: Ui, mode: Option<PermissionMode>, instruction: Option<String>) {
    let chat = ui.open_chat.get_untracked();
    spawn_local(async move {
        settle(ui, start_flow(chat, mode, instruction).await);
    });
}

/// Starts a run in `chat` (a new chat when `None`). The instruction goes into the run request, so
/// the planner reads it with the run.
pub async fn start_flow(
    chat: Option<ChatId>,
    mode: Option<PermissionMode>,
    instruction: Option<String>,
) -> Result<(), ApiFailure> {
    // The plan-only start is the Plan type, so the run carries it too.
    let intent = (mode == Some(PermissionMode::PlanOnly)).then_some(Intent::Plan);
    if let Some(mode) = mode {
        set_mode(mode).await?;
    }
    let request = RunRequest {
        chat,
        text: instruction,
        intent,
        ..RunRequest::default()
    };
    api::start_run(&request).await.map(|_| ())
}

pub async fn set_mode(mode: PermissionMode) -> Result<serde_json::Value, ApiFailure> {
    let update = PermissionsUpdate {
        mode: Some(mode),
        ..PermissionsUpdate::default()
    };
    api::send("PUT", "/api/permissions", &update).await
}

pub fn run_command(ui: Ui, command: SlashCommand) {
    match command {
        SlashCommand::Pause => post_action(ui, "/api/pause".to_owned()),
        SlashCommand::Resume => post_action(ui, "/api/resume".to_owned()),
        SlashCommand::Stop => post_action(ui, "/api/stop".to_owned()),
        SlashCommand::Kill(name) => kill_agent(ui, &name),
        SlashCommand::Cap(amount) => lower_cap(ui, amount),
        SlashCommand::Brain(mode) => set_brain(ui, mode),
        SlashCommand::Agent(name) => run_agent_command(ui, &name),
        SlashCommand::Focus(name) => focus_agent(ui, &name),
        SlashCommand::Report => ui.open_panel(PanelId::Report),
        SlashCommand::Skill(name) => add_skill(ui, &name),
        SlashCommand::Help => ui.show_toast(help_text(), ToastKind::Info),
    }
}

/// Adds a skill to the open chat's next message. The name must be one of the skills Sentinel has.
pub fn add_skill(ui: Ui, name: &str) {
    let Some(chat) = ui.open_chat.get_untracked() else {
        ui.show_toast("Open a chat first.", ToastKind::Warn);
        return;
    };
    if skill_attached(ui, name) {
        ui.show_toast(skill_goes_text(name), ToastKind::Info);
        return;
    }
    let name = name.to_owned();
    spawn_local(async move {
        match api::get::<Vec<SkillInfo>>("/api/skills").await {
            Ok(list) if list.iter().any(|skill| skill.name == name) => {
                attach_skill(ui, &chat, &name).await;
            }
            Ok(list) => ui.show_toast(unknown_skill_text(&name, &list), ToastKind::Warn),
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

/// True when the open chat already carries this skill for its next message.
pub fn skill_attached(ui: Ui, name: &str) -> bool {
    ui.attachments.with_untracked(|list| {
        list.iter()
            .any(|attachment| attachment.kind == AttachmentKind::Skill && attachment.name == name)
    })
}

fn skill_goes_text(name: &str) -> String {
    format!("Skill {name} goes with your next message.")
}

/// Adds the skill as a chip of the chat. The daemon reads its text when the message is sent.
pub async fn attach_skill(ui: Ui, chat: &ChatId, name: &str) {
    let body = AddAttachment {
        kind: AttachmentKind::Skill,
        path: None,
        text: None,
        name: Some(name.to_owned()),
        content_base64: None,
    };
    match api::add_attachment(chat, &body).await {
        Ok(_) => {
            reload_attachments(ui);
            ui.show_toast(skill_goes_text(name), ToastKind::Ok);
        }
        Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
    }
}

pub fn help_text() -> String {
    let names: Vec<&str> = slash::help_lines().iter().map(|(name, _)| *name).collect();
    format!("Commands: {}", names.join(", "))
}

pub fn kill_agent(ui: Ui, name: &str) {
    ui.show_toast(format!("Killing {name}."), ToastKind::Warn);
    post_action(ui, format!("/api/agents/{name}/kill"));
}

pub fn lower_cap(ui: Ui, amount: String) {
    spawn_local(async move {
        let body = LowerCap { cap: amount };
        settle(
            ui,
            api::post::<_, serde_json::Value>("/api/cap/lower", &body).await,
        );
    });
}

pub fn set_brain(ui: Ui, mode: BrainMode) {
    let current = ui.snapshot.with_untracked(|s| s.brain_mode);
    if ui.from_phone.get_untracked() && !current.phone_may_set(mode) {
        ui.show_toast(
            "A phone can only set Brain to a stricter mode.",
            ToastKind::Warn,
        );
        return;
    }
    spawn_local(async move {
        let body = SetBrainMode { mode };
        settle(
            ui,
            api::post::<_, serde_json::Value>("/api/brain/mode", &body).await,
        );
    });
}

/// `/agent claude` and `/agent auto` set the open chat's agent.
pub fn run_agent_command(ui: Ui, name: &str) {
    match parse_agent_command(name) {
        AgentCommand::Auto => choose_chat_agent(ui, None, None),
        AgentCommand::Named(agent) => choose_chat_agent(ui, Some(&agent), None),
        AgentCommand::Missing => ui.show_toast(
            "Name an agent, for example /agent claude or /agent auto.",
            ToastKind::Warn,
        ),
    }
}

/// Sends `text` as a new run with no type, as the stopped-agent card's Retry does. A live run
/// refuses, as in `resend_as`.
pub fn resend_text(ui: Ui, text: String) {
    let open = ui.open_chat.get_untracked();
    let (harness, live) = ui
        .snapshot
        .with_untracked(|s| (s.harness, s.run.as_ref().map(|run| run.chat.clone())));
    if send_route(harness, live.as_ref(), open.as_ref()) != SendRoute::StartRun {
        ui.show_toast(RESEND_WAIT, ToastKind::Warn);
        return;
    }
    spawn_local(async move {
        match start_retry(open, text).await {
            Ok(()) => refresh(ui),
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

/// Starts a run in `chat` with one text and no type.
pub async fn start_retry(chat: Option<ChatId>, text: String) -> Result<(), ApiFailure> {
    let request = RunRequest {
        chat,
        text: Some(text),
        ..RunRequest::default()
    };
    api::start_run(&request).await.map(|_| ())
}

pub fn focus_agent(ui: Ui, name: &str) {
    if name == "all" {
        ui.focus.set(None);
        return;
    }
    let known = ui
        .snapshot
        .with_untracked(|s| s.tools.iter().any(|tool| tool.id.as_str() == name));
    if known {
        ui.focus.set(Some(ToolId::new(name)));
    } else {
        ui.show_toast(format!("No agent called {name}."), ToastKind::Warn);
    }
}
