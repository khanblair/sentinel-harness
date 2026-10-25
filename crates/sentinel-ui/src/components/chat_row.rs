//! `ChatRow`: one chat in the sidebar (`docs/ui-rules.md`, Layout). Its more button and a long
//! press open the chat menu. Rename happens in place.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gloo_timers::callback::Timeout;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::ChatId;
use sentinel_core::api::UpdateChat;
use web_sys::KeyboardEvent;

use crate::api::{self, Then};
use crate::chats::{ChatRow as RowData, RowIcon, commit_rename};
use crate::components::add_project::switch_project;
use crate::components::chat_menu::{more_anchor, row_anchor};
use crate::components::icon::Icon;
use crate::components::popover::toggle_popover;
use crate::components::top_bar::settle;
use crate::state::{self, Align, Popover, PopoverKind, Ui, use_ui};

/// How long a press lasts before it counts as a long press.
const LONG_PRESS_MS: u32 = 500;

/// The timer of a press on a row. The flag tells the click after a long press to do nothing.
#[derive(Clone, Default)]
struct PressState {
    timer: Rc<RefCell<Option<Timeout>>>,
    fired: Rc<Cell<bool>>,
}

impl PressState {
    /// Starts the timer. Dropping a `Timeout` cancels it, so `end` only clears the slot.
    fn start(&self, ui: Ui, chat: &ChatId) {
        self.fired.set(false);
        let fired = self.fired.clone();
        let anchor = row_anchor(chat);
        let timeout = Timeout::new(LONG_PRESS_MS, move || {
            fired.set(true);
            ui.popover.set(Some(Popover {
                anchor_id: anchor,
                align: Align::Left,
                kind: PopoverKind::Chat,
            }));
        });
        *self.timer.borrow_mut() = Some(timeout);
    }

    fn end(&self) {
        self.timer.borrow_mut().take();
    }

    /// True when the click follows a long press. It also resets the flag.
    fn swallow_click(&self) -> bool {
        self.fired.replace(false)
    }
}

/// Whether the row's chat is the open one.
fn row_is_open(ui: Ui, chat: ChatId) -> impl Fn() -> bool {
    move || ui.open_chat.with(|open| open.as_ref() == Some(&chat))
}

/// Whether the row's chat is in rename mode.
fn row_is_editing(ui: Ui, chat: ChatId) -> impl Fn() -> bool {
    move || {
        ui.renaming
            .with(|state| state.as_ref().is_some_and(|state| state.chat == chat))
    }
}

#[component]
pub fn ChatRow(row: RowData) -> impl IntoView {
    let ui = use_ui();
    let id = row.id.clone();
    let input_chat = id.clone();
    let more_chat = id.clone();
    let press = PressState::default();
    let active = row_is_open(ui, id.clone());
    let editing = row_is_editing(ui, id.clone());
    let title = row.title.clone();
    let icon = status_icon(row.icon);
    view! {
        <div class="run" id=row_anchor(&id) class:active=active role="button" tabindex="0"
            on:click=row_click(ui, id.clone(), press.clone())
            on:pointerdown=row_press(ui, id.clone(), press.clone())
            on:pointerup=row_release(press.clone())
            on:pointerleave=row_release(press.clone())
            on:pointercancel=row_release(press.clone())>
            {row.pinned.then(|| view! {
                <span class="pin" title="Pinned"><Icon name="pin" size=13 /></span>
            })}
            {move || if editing() {
                view! { <RenameInput ui=ui chat=input_chat.clone() /> }.into_any()
            } else {
                view! { <span class="t">{title.clone()}</span> }.into_any()
            }}
            <span class="meta">
                {row.needs_you.then(|| view! { <span class="dot" title="Needs you"></span> })}
                <span>{row.count}</span>
                {icon}
            </span>
            <MoreButton ui=ui chat=more_chat />
        </div>
    }
}

/// The more button of a row. It opens the chat menu under itself and leaves the chat closed.
#[component]
fn MoreButton(ui: Ui, chat: ChatId) -> impl IntoView {
    let anchor = more_anchor(&chat);
    view! {
        <button class="ibtn more" id=anchor.clone() aria-label="Chat actions"
            on:pointerdown=move |event| event.stop_propagation()
            on:click=move |event| {
                event.stop_propagation();
                toggle_popover(ui, &anchor, Align::Right, PopoverKind::Chat);
            }>
            <Icon name="more" size=15 />
        </button>
    }
}

/// The row click: open the chat, unless it ends a long press.
fn row_click(ui: Ui, chat: ChatId, press: PressState) -> impl Fn(web_sys::MouseEvent) + 'static {
    move |_| {
        if press.swallow_click() {
            return;
        }
        ui.open(chat.clone());
        state::refresh(ui);
    }
}

fn row_press(ui: Ui, chat: ChatId, press: PressState) -> impl Fn(web_sys::PointerEvent) + 'static {
    move |_| press.start(ui, &chat)
}

fn row_release(press: PressState) -> impl Fn(web_sys::PointerEvent) + 'static {
    move |_| press.end()
}

/// The title input. It starts with the text selected. Enter and blur save, Escape cancels.
#[component]
fn RenameInput(ui: Ui, chat: ChatId) -> impl IntoView {
    let field = NodeRef::<html::Input>::new();
    let start = ui
        .renaming
        .with_untracked(|state| state.as_ref().map(|state| state.title.clone()))
        .unwrap_or_default();
    let shown = start.clone();
    // The value is set here, not only in the view, so the selection covers the whole title.
    Effect::new(move |_| {
        if let Some(element) = field.get() {
            element.set_value(&shown);
            let _ = element.focus();
            element.select();
        }
    });
    let key_chat = chat.clone();
    view! {
        <input class="ren" aria-label="Chat name" prop:value=start node_ref=field
            on:click=move |event| event.stop_propagation()
            on:keydown=move |event: KeyboardEvent| rename_key(ui, &key_chat, &event)
            on:blur=move |event| save_on_blur(ui, &chat, &event_target_value(&event)) />
    }
}

fn rename_key(ui: Ui, chat: &ChatId, event: &KeyboardEvent) {
    match event.key().as_str() {
        "Enter" => {
            event.prevent_default();
            save_rename(ui, chat, &event_target_value(event));
        }
        "Escape" => ui.renaming.set(None),
        _ => {}
    }
}

/// A blur saves only while the row is still in rename mode. Enter and Escape clear it first.
fn save_on_blur(ui: Ui, chat: &ChatId, typed: &str) {
    if ui.renaming.with_untracked(Option::is_some) {
        save_rename(ui, chat, typed);
    }
}

/// Leaves rename mode, then saves the name when it is not empty and differs.
fn save_rename(ui: Ui, chat: &ChatId, typed: &str) {
    let current = ui
        .renaming
        .with_untracked(|state| state.as_ref().map(|state| state.title.clone()))
        .unwrap_or_default();
    ui.renaming.set(None);
    let Some(name) = commit_rename(&current, typed) else {
        return;
    };
    let chat = chat.clone();
    spawn_local(async move {
        let update = UpdateChat {
            title: Some(name),
            ..UpdateChat::default()
        };
        settle(ui, api::update_chat(&chat, &update).await);
    });
}

/// A chat of a project that is not the active one. It has no menu: a click switches to the
/// project and opens the chat. `project` is the id to save, `None` for the primary project.
#[component]
pub fn PlainChatRow(row: RowData, project: Option<String>) -> impl IntoView {
    let chat = row.id.clone();
    let icon = status_icon(row.icon);
    let open = move || switch_project(project.as_deref(), &Then::OpenChat(chat.clone()));
    let key_open = open.clone();
    view! {
        <div class="run" role="button" tabindex="0"
            on:click=move |_| open()
            on:keydown=move |event| {
                if event.key() == "Enter" {
                    key_open();
                }
            }>
            <span class="t">{row.title}</span>
            <span class="meta">
                {row.needs_you.then(|| view! { <span class="dot" title="Needs you"></span> })}
                <span>{row.count}</span>
                {icon}
            </span>
        </div>
    }
}

pub fn status_icon(icon: RowIcon) -> AnyView {
    match icon {
        RowIcon::Spinner => view! { <span class="spin" title="Running"></span> }.into_any(),
        RowIcon::Paused => view! {
            <span style="color:var(--warn)" title="Paused"><Icon name="pause" size=14 /></span>
        }
        .into_any(),
        RowIcon::Done => view! {
            <span style="color:var(--ok)" title="Finished"><Icon name="check" size=14 /></span>
        }
        .into_any(),
        RowIcon::Blocked => view! {
            <span style="color:var(--danger)" title="Has blocked tasks"><Icon name="alert" size=14 /></span>
        }
        .into_any(),
        RowIcon::Planned => view! {
            <span title="Plan waiting for approval"><Icon name="clock" size=14 /></span>
        }
        .into_any(),
        RowIcon::Stopped => view! {
            <span title="Stopped"><Icon name="stop" size=13 /></span>
        }
        .into_any(),
        RowIcon::None => ().into_any(),
    }
}
