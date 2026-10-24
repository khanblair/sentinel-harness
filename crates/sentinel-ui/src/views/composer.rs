//! The composer: the text box, the slash menu, the footer chips, and what each send does. A plain
//! message goes to Sentinel or one agent. A slash command maps to one API route.

use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::{LowerCap, PermissionsUpdate, RunRequest, SetBrainMode};
use sentinel_core::{BrainMode, HarnessState, PermissionMode, ToolId};
use serde::Serialize;
use web_sys::{HtmlElement, HtmlTextAreaElement, KeyboardEvent};

use crate::api::{self, ApiFailure};
use crate::components::agent_stack::ready_agents;
use crate::components::brain_chip::BrainChip;
use crate::components::icon::Icon;
use crate::components::permission_chip::PermissionChip;
use crate::components::popover::toggle_popover;
use crate::components::slash_menu::SlashMenu;
use crate::components::target_chip::TargetChip;
use crate::components::top_bar::{post_action, settle};
use crate::layout::{Layout, PanelId};
use crate::slash::{self, SlashCommand, SlashEntry};
use crate::state::{Align, Connection, PopoverKind, Target, ToastKind, Ui, use_ui};

/// The text box stops growing at this height in pixels.
const TEXTAREA_MAX_PX: i32 = 150;
const AGENT_NOTE: &str = "Applied on the agent's next step. Reserved files do not change.";

/// The body of `POST /api/message`.
#[derive(Serialize)]
struct MessageRequest {
    text: String,
    to: Option<ToolId>,
}

/// The composer's text and menu state. Every field is a signal, so the struct is `Copy`.
#[derive(Clone, Copy)]
struct Draft {
    text: RwSignal<String>,
    selected: RwSignal<usize>,
    dismissed: RwSignal<bool>,
    commands: Memo<Vec<SlashEntry>>,
}

impl Draft {
    fn menu_open(self) -> bool {
        !self.dismissed.get_untracked() && self.commands.with_untracked(|list| !list.is_empty())
    }

    fn step(self, step_down: bool) {
        let len = self.commands.with_untracked(Vec::len);
        if len > 0 {
            self.selected
                .update(|index| *index = next_index(*index, step_down, len));
        }
    }

    fn complete(self) {
        let index = self.selected.get_untracked();
        let chosen = self.commands.with_untracked(|list| pick_entry(list, index));
        if let Some((name, _)) = chosen {
            self.text.set(completed_text(name));
            self.dismissed.set(true);
        }
    }
}

/// The next menu row, wrapping at both ends. `len` is never zero here.
pub fn next_index(current: usize, step_down: bool, len: usize) -> usize {
    if step_down {
        (current + 1) % len
    } else {
        (current + len - 1) % len
    }
}

fn pick_entry(list: &[SlashEntry], index: usize) -> Option<SlashEntry> {
    list.get(index).or_else(|| list.first()).copied()
}

/// A command with its own argument, such as `/cap 1.50`, is filled in as written. A bare
/// command gets a space so the user can type the argument.
pub fn completed_text(name: &str) -> String {
    if name.contains(' ') {
        name.to_owned()
    } else {
        format!("{name} ")
    }
}

/// A run starts when no run is live. Any other state takes the message as an instruction.
pub fn starts_run(state: HarnessState) -> bool {
    matches!(state, HarnessState::Idle | HarnessState::Done)
}

#[component]
pub fn Composer() -> impl IntoView {
    let text = RwSignal::new(String::new());
    let selected = RwSignal::new(0_usize);
    let dismissed = RwSignal::new(false);
    let commands = Memo::new(move |_| slash::matches(&text.get()));
    let draft = Draft {
        text,
        selected,
        dismissed,
        commands,
    };
    view! {
        <SlashMenu commands=commands selected=selected on_pick=move |name: &'static str| pick(draft, name) />
        <ComposerRow draft=draft />
        <ComposerFoot />
    }
}

#[component]
fn ComposerRow(draft: Draft) -> impl IntoView {
    let ui = use_ui();
    let input = NodeRef::<html::Textarea>::new();
    Effect::new(move |_| {
        draft.text.track();
        if let Some(element) = input.get() {
            autosize(&element);
        }
    });
    view! {
        <div class="comp">
            <button id="attachBtn" class="ibtn" aria-label="Add backlog or context" data-tip="Add"
                on:click=move |_| toggle_popover(ui, "attachBtn", Align::Left, PopoverKind::Attach)>
                <Icon name="plus" size=18 />
            </button>
            <textarea id="cin" rows="1" placeholder="Message Sentinel, or type / for commands"
                aria-label="Message" node_ref=input
                prop:value=move || draft.text.get()
                on:input=move |event| {
                    draft.text.set(event_target_value(&event));
                    draft.dismissed.set(false);
                    draft.selected.set(0);
                }
                on:keydown=move |event: KeyboardEvent| handle_key(ui, draft, &event)></textarea>
            <button class="ibtn" disabled=true aria-label="Voice input"
                data-tip="Voice input is not available yet" title="Voice input is not available yet">
                <Icon name="mic" size=18 />
            </button>
            <button class="ibtn send" aria-label="Send"
                prop:disabled=move || matches!(ui.connection.get(), Connection::Offline { .. })
                on:click=move |_| submit(ui, draft)>
                <Icon name="send" size=16 />
            </button>
        </div>
    }
}

#[component]
fn ComposerFoot() -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="comp-foot">
            <PermissionChip />
            <BrainChip />
            <span style="flex:1"></span>
            <TargetChip />
            {move || (ui.snapshot.with(|s| s.harness) == HarnessState::Running)
                .then(|| view! { <span class="spin"></span> })}
        </div>
    }
}

fn autosize(element: &HtmlTextAreaElement) {
    let html: &HtmlElement = element;
    let style = html.style();
    let _ = style.set_property("height", "auto");
    let height = element.scroll_height().min(TEXTAREA_MAX_PX);
    let _ = style.set_property("height", &format!("{height}px"));
}

fn pick(draft: Draft, name: &str) {
    draft.text.set(completed_text(name));
    draft.dismissed.set(true);
}

fn handle_key(ui: Ui, draft: Draft, event: &KeyboardEvent) {
    let open = draft.menu_open();
    match event.key().as_str() {
        "Enter" if !event.shift_key() => {
            event.prevent_default();
            submit(ui, draft);
        }
        "ArrowDown" if open => {
            event.prevent_default();
            draft.step(true);
        }
        "ArrowUp" if open => {
            event.prevent_default();
            draft.step(false);
        }
        "Tab" if open => {
            event.prevent_default();
            draft.complete();
        }
        "Escape" if open => draft.dismissed.set(true),
        _ => {}
    }
}

fn submit(ui: Ui, draft: Draft) {
    let message = draft.text.get_untracked().trim().to_owned();
    if message.is_empty() {
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
    draft.text.set(String::new());
    draft.dismissed.set(false);
    dispatch(ui, &message);
}

/// Sends what the user typed: a command, or a plain message.
pub fn dispatch(ui: Ui, message: &str) {
    match slash::parse(message) {
        Some(Ok(command)) => run_command(ui, command),
        Some(Err(problem)) => ui.show_toast(problem, ToastKind::Warn),
        None => send_message(ui, message),
    }
}

fn send_message(ui: Ui, text: &str) {
    let harness = ui.snapshot.with_untracked(|s| s.harness);
    if starts_run(harness) {
        launch(ui, None, Some(text.to_owned()));
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
        settle(
            ui,
            api::post::<_, serde_json::Value>("/api/message", &request).await,
        );
    });
}

/// Starts a run, optionally in plan-only mode, and optionally sends one instruction to Sentinel
/// once the run has started.
pub fn launch(ui: Ui, mode: Option<PermissionMode>, instruction: Option<String>) {
    spawn_local(async move {
        settle(ui, start_flow(mode, instruction).await);
    });
}

async fn start_flow(
    mode: Option<PermissionMode>,
    instruction: Option<String>,
) -> Result<(), ApiFailure> {
    if let Some(mode) = mode {
        set_mode(mode).await?;
    }
    api::start_run(&RunRequest::default()).await?;
    if let Some(text) = instruction {
        let request = MessageRequest { text, to: None };
        api::post::<_, serde_json::Value>("/api/message", &request).await?;
    }
    Ok(())
}

async fn set_mode(mode: PermissionMode) -> Result<serde_json::Value, ApiFailure> {
    let update = PermissionsUpdate {
        mode: Some(mode),
        ..PermissionsUpdate::default()
    };
    api::send("PUT", "/api/permissions", &update).await
}

fn run_command(ui: Ui, command: SlashCommand) {
    match command {
        SlashCommand::Pause => post_action(ui, "/api/pause".to_owned()),
        SlashCommand::Resume => post_action(ui, "/api/resume".to_owned()),
        SlashCommand::Stop => post_action(ui, "/api/stop".to_owned()),
        SlashCommand::Kill(name) => kill_agent(ui, &name),
        SlashCommand::Cap(amount) => lower_cap(ui, amount),
        SlashCommand::Plan => spawn_local(async move {
            settle(ui, set_mode(PermissionMode::PlanOnly).await);
        }),
        SlashCommand::Brain(mode) => set_brain(ui, mode),
        SlashCommand::Agent(name) => choose_agent(ui, &name),
        SlashCommand::Focus(name) => focus_agent(ui, &name),
        SlashCommand::Report => ui.open_panel(PanelId::Report),
        SlashCommand::Help => ui.show_toast(help_text(), ToastKind::Info),
    }
}

fn help_text() -> String {
    let names: Vec<&str> = slash::help_lines().iter().map(|(name, _)| *name).collect();
    format!("Commands: {}", names.join(", "))
}

fn kill_agent(ui: Ui, name: &str) {
    ui.show_toast(format!("Killing {name}."), ToastKind::Warn);
    post_action(ui, format!("/api/agents/{name}/kill"));
}

fn lower_cap(ui: Ui, amount: String) {
    spawn_local(async move {
        let body = LowerCap { cap: amount };
        settle(
            ui,
            api::post::<_, serde_json::Value>("/api/cap/lower", &body).await,
        );
    });
}

fn set_brain(ui: Ui, mode: BrainMode) {
    let current = ui.snapshot.with_untracked(|s| s.brain_mode);
    if ui.layout.get_untracked() == Layout::Phone && !current.phone_may_set(mode) {
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

fn choose_agent(ui: Ui, name: &str) {
    if name == "sentinel" {
        ui.target.set(Target::Sentinel);
        return;
    }
    let ready = ui
        .snapshot
        .with_untracked(|s| ready_agents(s).iter().any(|tool| tool.id.as_str() == name));
    if ready {
        ui.target.set(Target::Agent(ToolId::new(name)));
        ui.show_toast(format!("Messages now go to {name}."), ToastKind::Ok);
    } else {
        ui.show_toast(format!("No ready agent called {name}."), ToastKind::Warn);
    }
}

fn focus_agent(ui: Ui, name: &str) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_keys_wrap_at_both_ends() {
        assert_eq!(next_index(0, false, 3), 2);
        assert_eq!(next_index(2, true, 3), 0);
        assert_eq!(next_index(0, true, 3), 1);
    }

    #[test]
    fn a_bare_command_gets_a_space_and_a_full_one_does_not() {
        assert_eq!(completed_text("/report"), "/report ");
        assert_eq!(completed_text("/cap 1.50"), "/cap 1.50");
    }

    #[test]
    fn only_idle_and_done_runs_start_a_run_from_a_message() {
        assert!(starts_run(HarnessState::Idle));
        assert!(starts_run(HarnessState::Done));
        assert!(!starts_run(HarnessState::Running));
        assert!(!starts_run(HarnessState::Paused));
    }

    #[test]
    fn a_stale_menu_index_falls_back_to_the_first_row() {
        let list = [("/pause", "Pause the run"), ("/stop", "Stop the run")];
        assert_eq!(pick_entry(&list, 1), Some(("/stop", "Stop the run")));
        assert_eq!(pick_entry(&list, 9), Some(("/pause", "Pause the run")));
        assert_eq!(pick_entry(&[], 0), None);
    }
}
