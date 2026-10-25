//! The live socket. It keeps the page in step with the daemon: events go into the snapshot, and
//! a short quiet period after a non-output event triggers one snapshot reload.

use std::cell::{Cell, RefCell};

use futures::{SinkExt, StreamExt};
use gloo_net::websocket::Message;
use gloo_net::websocket::futures::WebSocket;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::{ClientFrame, ServerFrame};
use sentinel_core::{ChatId, Event, Snapshot};
use time::{Duration, OffsetDateTime};
use wasm_bindgen::JsCast;

use crate::api::{self, ApiFailure};
use crate::chats::pick_open_chat;
use crate::components::add_project::load_projects;
use crate::state::{Connection, Toast, ToastKind, Ui};

/// Bursts of events make one reload: the reload waits this long after the last event.
const QUIET_MS: u32 = 150;
const RECONNECT_CAP_SECONDS: u32 = 8;
const PAIR_MESSAGE: &str = "This device is not paired. Pair it from the computer.";

/// Seconds to wait before retry `attempt` (0 is the first retry): 1, 2, 4, 8, then 8 again.
pub fn reconnect_delay_seconds(attempt: u32) -> u32 {
    (1u32 << attempt.min(3)).min(RECONNECT_CAP_SECONDS)
}

/// The `HH:MM` text shown in the offline banner.
pub fn format_clock(moment: OffsetDateTime) -> String {
    format!("{:02}:{:02}", moment.hour(), moment.minute())
}

/// The current time. The browser clock is read through `js_sys` because `time` has no clock on
/// `wasm32-unknown-unknown` in this build.
fn now() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds_f64(js_sys::Date::now() / 1000.0)
}

/// Opens the socket and keeps it open. Starts once, from the app mount.
pub fn connect(ui: Ui) {
    spawn_local(async move {
        let mut attempt = 0;
        let mut was_live = false;
        loop {
            let reached_live = session(ui, was_live).await;
            was_live |= reached_live;
            let delay = reconnect_delay_seconds(attempt);
            attempt = if reached_live { 0 } else { attempt + 1 };
            let since = ui
                .snapshot
                .with_untracked(|snapshot| snapshot.events.last().map(|record| record.at))
                .unwrap_or_else(now);
            ui.connection.set(Connection::Offline {
                since: format_clock(since),
            });
            TimeoutFuture::new(delay.saturating_mul(1000)).await;
        }
    });
}

/// Fetches the snapshot of the open chat and replaces the current one. A 401 means this device is
/// not paired.
pub fn reload(ui: Ui) {
    let asked = ui.open_chat.get_untracked();
    spawn_local(async move {
        match api::snapshot_for(asked.as_ref()).await {
            Ok(snapshot) => adopt(ui, asked.as_ref(), snapshot),
            Err(failure) => report_failure(ui, &failure),
        }
    });
}

/// Applies a snapshot fetched for `asked`. When the chat the page should show differs (none was
/// open, or it is gone), it stores the new choice and fetches again for that chat. An answer for a
/// chat the user has left is dropped, because a newer fetch for the open chat is on its way.
fn adopt(ui: Ui, asked: Option<&ChatId>, snapshot: Snapshot) {
    if ui.open_chat.get_untracked().as_ref() != asked {
        return;
    }
    let chosen = pick_open_chat(&snapshot, asked);
    if chosen.as_ref() == asked {
        ui.apply_snapshot(snapshot);
    } else {
        ui.open_chat.set(chosen);
        reload(ui);
    }
}

fn report_failure(ui: Ui, failure: &ApiFailure) {
    if failure.is_unauthorized() {
        ui.modal.set(None);
        ui.toast.set(Some(Toast {
            text: PAIR_MESSAGE.to_owned(),
            kind: ToastKind::Warn,
        }));
    } else {
        ui.show_toast(failure.to_string(), ToastKind::Danger);
    }
}

thread_local! {
    static RELOAD_TICKET: Cell<u64> = const { Cell::new(0) };
}

/// Trailing debounce: only the last call inside the quiet period reloads.
fn schedule_reload(ui: Ui) {
    let ticket = RELOAD_TICKET.with(|latest| {
        let next = latest.get() + 1;
        latest.set(next);
        next
    });
    spawn_local(async move {
        TimeoutFuture::new(QUIET_MS).await;
        if RELOAD_TICKET.with(Cell::get) == ticket {
            reload(ui);
        }
    });
}

fn socket_url() -> Option<String> {
    let location = web_sys::window()?.location();
    let scheme = if location.protocol().ok()? == "https:" {
        "wss"
    } else {
        "ws"
    };
    let query = api::project_query(api::project().as_deref());
    Some(format!("{scheme}://{}/ws{query}", location.host().ok()?))
}

/// One connection, from open to close. Returns true when the socket reached `Live`.
async fn session(ui: Ui, reconnecting: bool) -> bool {
    ui.connection.set(Connection::Connecting);
    let Some(url) = socket_url() else {
        return false;
    };
    let Ok(mut socket) = WebSocket::open(&url) else {
        return false;
    };
    let auth = ClientFrame::Auth {
        token: api::token().unwrap_or_default(),
    };
    let after = ui
        .snapshot
        .with_untracked(|snapshot| snapshot.last_event_id);
    if send_frame(&mut socket, &auth).await.is_err()
        || send_frame(&mut socket, &ClientFrame::Subscribe { after })
            .await
            .is_err()
    {
        return false;
    }
    ui.connection.set(Connection::Live);
    check_for_new_build();
    if reconnecting {
        reload(ui);
    }
    while let Some(message) = socket.next().await {
        match message {
            Ok(Message::Text(text)) => handle_text(ui, &text),
            Ok(Message::Bytes(_)) => {}
            Err(_) => break,
        }
    }
    true
}

async fn send_frame(
    socket: &mut WebSocket,
    frame: &ClientFrame,
) -> Result<(), gloo_net::websocket::WebSocketError> {
    let text = serde_json::to_string(frame).unwrap_or_default();
    socket.send(Message::Text(text)).await
}

thread_local! {
    /// The daemon start time this page saw first. A later, different one means a new build runs.
    static FIRST_STARTED: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Whether the page should reload. `first` is the start time the page saw first, `now` the one the
/// daemon reports. Only a change means a new build; with no `first` yet, nothing reloads.
pub fn should_reload(first: Option<&str>, now: &str) -> bool {
    first.is_some_and(|first| first != now)
}

/// Compares the daemon start time with the first one seen. The first answer is only remembered.
/// A failed call, or typed text in an input, waits for the next live connection.
fn check_for_new_build() {
    spawn_local(async {
        let Ok(now) = api::daemon_started().await else {
            return;
        };
        let first = FIRST_STARTED.with(|cell| cell.borrow().clone());
        if first.is_none() {
            FIRST_STARTED.with(|cell| *cell.borrow_mut() = Some(now));
        } else if should_reload(first.as_deref(), &now) && !owner_is_typing() {
            reload_page();
        }
    });
}

/// True while the focused input or textarea holds text. A reload would lose that text.
fn owner_is_typing() -> bool {
    let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.active_element())
    else {
        return false;
    };
    if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>() {
        return !input.value().is_empty();
    }
    element
        .dyn_ref::<web_sys::HtmlTextAreaElement>()
        .is_some_and(|area| !area.value().is_empty())
}

fn reload_page() {
    if let Some(window) = web_sys::window() {
        let _ = window.location().reload();
    }
}

fn handle_text(ui: Ui, text: &str) {
    let Ok(ServerFrame::Event { record }) = serde_json::from_str::<ServerFrame>(text) else {
        return;
    };
    let is_output = matches!(record.event, Event::AgentOutput { .. });
    let is_chat = matches!(record.event, Event::ChatChanged { .. });
    ui.ingest(record);
    if is_chat {
        load_projects(ui);
    }
    if !is_output {
        schedule_reload(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_waits_one_two_four_then_eight_seconds() {
        let delays: Vec<u32> = (0..5).map(reconnect_delay_seconds).collect();
        assert_eq!(delays, vec![1, 2, 4, 8, 8]);
    }

    #[test]
    fn reconnect_delay_never_passes_the_cap() {
        assert_eq!(reconnect_delay_seconds(u32::MAX), 8);
    }

    #[test]
    fn clock_text_is_hours_and_minutes() {
        let moment = OffsetDateTime::UNIX_EPOCH + Duration::seconds(3723);
        assert_eq!(format_clock(moment), "01:02");
    }

    #[test]
    fn clock_text_pads_single_digits() {
        let moment = OffsetDateTime::UNIX_EPOCH + Duration::seconds(5 * 60 + 9 * 3600);
        assert_eq!(format_clock(moment), "09:05");
    }

    #[test]
    fn the_first_start_time_is_remembered_without_a_reload() {
        assert!(!should_reload(None, "2026-10-25T10:00:00Z"));
    }

    #[test]
    fn the_same_start_time_does_not_reload() {
        let started = "2026-10-25T10:00:00Z";
        assert!(!should_reload(Some(started), started));
    }

    #[test]
    fn a_new_start_time_reloads() {
        assert!(should_reload(
            Some("2026-10-25T10:00:00Z"),
            "2026-10-25T11:30:00Z"
        ));
    }
}
