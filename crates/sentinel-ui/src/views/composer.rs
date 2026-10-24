//! The composer: the text box, the slash menu, the chips row, and the footer chips. What a send
//! does is in `composer_send`.

use leptos::html;
use leptos::prelude::*;
use sentinel_core::{ChatId, HarnessState};
use web_sys::{HtmlElement, HtmlTextAreaElement, KeyboardEvent};

use crate::components::brain_chip::BrainChip;
use crate::components::icon::Icon;
use crate::components::permission_chip::PermissionChip;
use crate::components::popover::toggle_popover;
use crate::components::slash_menu::SlashMenu;
use crate::components::target_chip::TargetChip;
use crate::slash::{self, SlashEntry};
use crate::state::{Align, Connection, PopoverKind, Ui, use_ui};
use crate::views::composer_chips::ChipsRow;
use crate::views::composer_send::submit;

pub use crate::views::composer_chips::reload_attachments;

pub use crate::views::composer_send::launch;

/// The text box stops growing at this height in pixels.
const TEXTAREA_MAX_PX: i32 = 150;

/// The composer's text and menu state. Every field is a signal, so the struct is `Copy`.
#[derive(Clone, Copy)]
pub(crate) struct Draft {
    pub(crate) text: RwSignal<String>,
    pub(crate) selected: RwSignal<usize>,
    pub(crate) dismissed: RwSignal<bool>,
    pub(crate) commands: Memo<Vec<SlashEntry>>,
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

/// Where a sent message goes: a new run, a note to the live run, or nowhere because another chat
/// holds the live run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendRoute {
    StartRun,
    NoteToAgents,
    Busy,
}

/// The route for a message typed in `open`. `live_chat` is the chat of the run that is live, if
/// any.
pub fn send_route(
    harness: HarnessState,
    live_chat: Option<&ChatId>,
    open: Option<&ChatId>,
) -> SendRoute {
    if starts_run(harness) {
        SendRoute::StartRun
    } else if live_chat.is_some_and(|live| Some(live) != open) {
        SendRoute::Busy
    } else {
        SendRoute::NoteToAgents
    }
}

#[component]
pub fn Composer() -> impl IntoView {
    let ui = use_ui();
    Effect::new(move |_| {
        ui.open_chat.track();
        ui.attachments.set(Vec::new());
        reload_attachments(ui);
    });
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
        <ChipsRow />
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
    fn a_message_starts_a_run_only_when_nothing_is_live() {
        let chat = ChatId::new("c1");
        assert_eq!(
            send_route(HarnessState::Idle, None, Some(&chat)),
            SendRoute::StartRun
        );
        assert_eq!(
            send_route(HarnessState::Done, Some(&chat), Some(&chat)),
            SendRoute::StartRun
        );
    }

    #[test]
    fn a_live_run_in_the_open_chat_takes_a_note() {
        let chat = ChatId::new("c1");
        assert_eq!(
            send_route(HarnessState::Running, Some(&chat), Some(&chat)),
            SendRoute::NoteToAgents
        );
        assert_eq!(
            send_route(HarnessState::Paused, None, Some(&chat)),
            SendRoute::NoteToAgents
        );
    }

    #[test]
    fn a_live_run_in_another_chat_is_busy() {
        let live = ChatId::new("c1");
        let open = ChatId::new("c2");
        assert_eq!(
            send_route(HarnessState::Running, Some(&live), Some(&open)),
            SendRoute::Busy
        );
        assert_eq!(
            send_route(HarnessState::Running, Some(&live), None),
            SendRoute::Busy
        );
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
