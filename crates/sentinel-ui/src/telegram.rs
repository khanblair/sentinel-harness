//! Telegram settings and activity: the pure rules the settings card and the panel share, the calls
//! to the daemon's `/api/telegram` routes, and the state both views keep. The bot token passes
//! through `save_and_check` once. It is never kept in a signal, logged, or shown in a toast.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::telegram_api::{QuietDto, SettingsUpdate, TelegramState, TokenSaved};
use sentinel_core::{Event, EventRecord};
use serde::Serialize;
use time::UtcOffset;

use crate::api::{self, ApiFailure};
use crate::layout::Layout;
use crate::state::{ToastKind, Ui};

/// How often the card asks the daemon whether `/start` has arrived.
const POLL_MS: u32 = 3_000;

/// One row of a choice list: the key the daemon stores, the words shown, and an optional hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    pub key: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
}

/// The four levels in the order of the settings page. Keys are the daemon's level names.
pub const LEVELS: [Choice; 4] = [
    Choice {
        key: "notify",
        label: "Notify",
        hint: "Sentinel sends events to this chat.",
    },
    Choice {
        key: "control",
        label: "Control",
        hint: "Approve or deny, answer decisions, pause, resume, stop, kill an agent, lower the cap.",
    },
    Choice {
        key: "code",
        label: "Code",
        hint: "Message one agent directly and ask for a short diff summary.",
    },
    Choice {
        key: "reports",
        label: "Reports",
        hint: "A summary when a run finishes, and a daily summary, with cost and Brain proposals.",
    },
];

/// The nine events the daemon can send to a chat. Budget has one event per threshold.
pub const EVENTS: [Choice; 9] = [
    Choice {
        key: "approval_needed",
        label: "Approval needed",
        hint: "",
    },
    Choice {
        key: "decision_needed",
        label: "Decision needed",
        hint: "",
    },
    Choice {
        key: "budget_70",
        label: "Budget at 70%",
        hint: "",
    },
    Choice {
        key: "budget_95",
        label: "Budget at 95%",
        hint: "",
    },
    Choice {
        key: "task_blocked",
        label: "Task blocked",
        hint: "",
    },
    Choice {
        key: "wave_finished",
        label: "Wave finished",
        hint: "",
    },
    Choice {
        key: "run_finished",
        label: "Run finished",
        hint: "",
    },
    Choice {
        key: "brain_proposals",
        label: "Brain care proposals",
        hint: "",
    },
    Choice {
        key: "errors",
        label: "Errors and crashes",
        hint: "",
    },
];

/// How much a message shows. Keys are the daemon's detail names.
pub const DETAILS: [Choice; 3] = [
    Choice {
        key: "status",
        label: "Status only",
        hint: "",
    },
    Choice {
        key: "summary",
        label: "Summary",
        hint: "",
    },
    Choice {
        key: "detail",
        label: "Detail",
        hint: "",
    },
];

/// Quiet hours a chat starts with: ten at night to seven in the morning.
pub const DEFAULT_QUIET: QuietDto = QuietDto {
    start_minute: 22 * 60,
    end_minute: 7 * 60,
};

/// The chip word for the connection: connected, waiting for `/start`, or neither.
pub fn chip_label(state: &TelegramState) -> &'static str {
    if state.connected {
        "Connected"
    } else if state.waiting_for_start {
        "Waiting for /start"
    } else {
        "Not connected"
    }
}

/// `current` with `key` switched on or off. The result keeps the order of `known`, so a toggle
/// never reorders the list and never adds a key twice.
pub fn with_choice(known: &[Choice], current: &[String], key: &str, on: bool) -> Vec<String> {
    known
        .iter()
        .map(|choice| choice.key)
        .filter(|candidate| {
            if *candidate == key {
                on
            } else {
                current.iter().any(|name| name.as_str() == *candidate)
            }
        })
        .map(str::to_owned)
        .collect()
}

pub fn levels_with(levels: &[String], level: &str, on: bool) -> Vec<String> {
    with_choice(&LEVELS, levels, level, on)
}

pub fn events_with(events: &[String], event: &str, on: bool) -> Vec<String> {
    with_choice(&EVENTS, events, event, on)
}

/// The words for a key, or nothing when the key is not in the list.
pub fn label_of(choices: &[Choice], key: &str) -> &'static str {
    choices
        .iter()
        .find(|choice| choice.key == key)
        .map_or("", |choice| choice.label)
}

fn all_digits(text: &str) -> bool {
    text.bytes().all(|byte| byte.is_ascii_digit())
}

/// `HH:MM` for minutes past midnight, as a time input shows it.
pub fn minutes_to_time(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// Parses the `HH:MM` text of a time input. A half-typed or longer value gives `None`.
pub fn time_to_minutes(text: &str) -> Option<u16> {
    let (hours, minutes) = text.split_once(':')?;
    if hours.len() != 2 || minutes.len() != 2 || !all_digits(hours) || !all_digits(minutes) {
        return None;
    }
    let hours: u16 = hours.parse().ok()?;
    let minutes: u16 = minutes.parse().ok()?;
    (hours < 24 && minutes < 60).then_some(hours * 60 + minutes)
}

/// A Telegram user ID: digits only, above zero, and small enough for `i64`.
pub fn valid_user_id(text: &str) -> Option<i64> {
    if text.is_empty() || !all_digits(text) {
        return None;
    }
    text.parse::<i64>().ok().filter(|id| *id > 0)
}

/// Quiet hours after the toggle moves. Off drops the window. On keeps the last window or starts
/// from the default.
pub fn quiet_switched(current: Option<QuietDto>, on: bool) -> Option<QuietDto> {
    on.then(|| current.unwrap_or(DEFAULT_QUIET))
}

/// Which end of the quiet window a time input changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuietEdge {
    Start,
    End,
}

pub fn quiet_edited(current: Option<QuietDto>, edge: QuietEdge, minute: u16) -> QuietDto {
    let quiet = current.unwrap_or(DEFAULT_QUIET);
    match edge {
        QuietEdge::Start => QuietDto {
            start_minute: minute,
            ..quiet
        },
        QuietEdge::End => QuietDto {
            end_minute: minute,
            ..quiet
        },
    }
}

/// The settings the card can change, copied from the last known state.
pub fn settings_of(state: &TelegramState) -> SettingsUpdate {
    SettingsUpdate {
        levels: state.levels.clone(),
        events: state.events.clone(),
        detail: state.detail.clone(),
        quiet_hours: state.quiet_hours,
    }
}

/// One line per Telegram event, oldest first, with the time in UTC. Other events give no line.
pub fn activity_lines(events: &[EventRecord]) -> Vec<String> {
    events.iter().filter_map(activity_line).collect()
}

fn activity_line(record: &EventRecord) -> Option<String> {
    let text = activity_text(&record.event)?;
    let utc = record.at.to_offset(UtcOffset::UTC);
    Some(format!("{:02}:{:02} {text}", utc.hour(), utc.minute()))
}

/// The words of a Telegram event. Other events have none, so they are left out of the log.
fn activity_text(event: &Event) -> Option<String> {
    if let Event::TelegramPaired { user_id } = event {
        return Some(format!("Chat paired with user ID {user_id}"));
    }
    if let Event::TelegramSent { summary } = event {
        return Some(format!("Sent: {summary}"));
    }
    if let Event::TelegramReceived { accepted, command } = event {
        let verb = if *accepted { "Got" } else { "Refused" };
        return Some(format!("{verb} {command}"));
    }
    None
}

#[derive(Serialize)]
struct TokenOut<'a> {
    token: &'a str,
}

/// No `Debug` derive, so the body cannot reach a log by accident.
#[derive(Serialize)]
struct ConnectOut {
    token: Option<String>,
    user_id: i64,
}

pub async fn load() -> Result<TelegramState, ApiFailure> {
    api::get("/api/telegram").await
}

pub async fn save_settings(update: &SettingsUpdate) -> Result<TelegramState, ApiFailure> {
    api::send("PUT", "/api/telegram", update).await
}

/// Saves the token, checks it with Telegram, and waits for `/start` from `user_id`. The daemon
/// keeps the token in the keychain and never sends it back.
pub async fn save_and_check(token: String, user_id: i64) -> Result<TelegramState, ApiFailure> {
    let _: TokenSaved = api::post("/api/telegram/token", &TokenOut { token: &token }).await?;
    api::post(
        "/api/telegram/connect",
        &ConnectOut {
            token: None,
            user_id,
        },
    )
    .await
}

pub async fn send_test() -> Result<serde_json::Value, ApiFailure> {
    api::post_empty("/api/telegram/test").await
}

/// Revokes the chat. The daemon also stops the bot's tasks.
pub async fn revoke(user_id: i64) -> Result<TelegramState, ApiFailure> {
    let path = format!("/api/telegram/chats/{user_id}");
    api::send("DELETE", &path, &serde_json::json!({})).await
}

/// The Telegram state one view keeps. Each view loads its own copy on mount. All fields are
/// `Copy`, so handlers and tasks take it by value.
#[derive(Clone, Copy)]
pub struct TelegramCtx {
    pub ui: Ui,
    pub state: RwSignal<Option<TelegramState>>,
}

impl TelegramCtx {
    /// A context with no state yet. Asks the daemon for the state now.
    pub fn load(ui: Ui) -> Self {
        let ctx = Self {
            ui,
            state: RwSignal::new(None),
        };
        spawn_local(async move {
            if let Ok(state) = load().await {
                ctx.state.set(Some(state));
            }
        });
        ctx
    }

    /// Phones read the state but cannot change it. Every control locks there.
    pub fn locked(self) -> bool {
        self.ui.layout.get() == Layout::Phone
    }

    pub fn is_waiting(self) -> bool {
        self.state
            .with_untracked(|state| state.as_ref().is_some_and(|s| s.waiting_for_start))
    }

    /// Shows a failed call as a toast. The state is fetched again, so the page shows what the
    /// daemon really has.
    pub fn apply(self, result: Result<TelegramState, ApiFailure>) {
        match result {
            Ok(state) => self.state.set(Some(state)),
            Err(failure) => {
                self.ui.show_toast(failure.to_string(), ToastKind::Danger);
                self.reload();
            }
        }
    }

    pub fn reload(self) {
        spawn_local(async move { self.apply(load().await) });
    }

    /// Changes the settings. The change shows at once and the whole update goes to the daemon.
    pub fn edit(self, change: impl FnOnce(&mut SettingsUpdate)) {
        let Some(mut next) = self
            .state
            .with_untracked(|state| state.as_ref().map(settings_of))
        else {
            return;
        };
        change(&mut next);
        self.state.update(|state| {
            if let Some(state) = state {
                state.levels.clone_from(&next.levels);
                state.events.clone_from(&next.events);
                state.detail.clone_from(&next.detail);
                state.quiet_hours = next.quiet_hours;
            }
        });
        spawn_local(async move { self.apply(save_settings(&next).await) });
    }
}

/// Asks the daemon every few seconds while the chat waits for `/start`. Call from a component
/// body. The loop stops when the component is dropped. A reply that comes after that is dropped
/// too, so no disposed signal is read.
pub fn poll_while_waiting(ctx: TelegramCtx) {
    let stopped = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stopped);
    on_cleanup(move || flag.store(true, Ordering::Relaxed));
    spawn_local(async move {
        loop {
            TimeoutFuture::new(POLL_MS).await;
            if stopped.load(Ordering::Relaxed) {
                return;
            }
            if !ctx.is_waiting() {
                continue;
            }
            let result = load().await;
            if stopped.load(Ordering::Relaxed) {
                return;
            }
            if let Ok(state) = result {
                ctx.state.set(Some(state));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn record(event: Event) -> EventRecord {
        EventRecord {
            id: 1,
            chat: None,
            at: time::OffsetDateTime::UNIX_EPOCH,
            event,
        }
    }

    #[test]
    fn no_token_and_no_chat_says_not_connected() {
        let state = TelegramState {
            connected: false,
            waiting_for_start: false,
            bot_username: None,
            token_saved: false,
            user_id: None,
            levels: Vec::new(),
            events: Vec::new(),
            detail: "detail".to_owned(),
            quiet_hours: None,
        };
        assert_eq!(chip_label(&state), "Not connected");
    }

    #[test]
    fn a_waiting_chat_says_waiting_for_start() {
        let state = TelegramState {
            connected: false,
            waiting_for_start: true,
            bot_username: Some("sentinel_bot".to_owned()),
            token_saved: true,
            user_id: Some(42),
            levels: Vec::new(),
            events: Vec::new(),
            detail: "detail".to_owned(),
            quiet_hours: None,
        };
        assert_eq!(chip_label(&state), "Waiting for /start");
    }

    #[test]
    fn switching_a_level_on_keeps_the_settings_order() {
        let now = keys(&["code", "notify"]);
        assert_eq!(
            levels_with(&now, "control", true),
            keys(&["notify", "control", "code"])
        );
    }

    #[test]
    fn switching_a_level_on_twice_does_not_repeat_it() {
        let once = levels_with(&[], "notify", true);
        assert_eq!(levels_with(&once, "notify", true), keys(&["notify"]));
    }

    #[test]
    fn switching_a_level_off_removes_only_that_level() {
        let now = keys(&["notify", "code"]);
        assert_eq!(levels_with(&now, "notify", false), keys(&["code"]));
    }

    #[test]
    fn switching_an_event_on_and_off_keeps_the_others() {
        let now = events_with(&[], "budget_95", true);
        let now = events_with(&now, "errors", true);
        assert_eq!(events_with(&now, "budget_95", false), keys(&["errors"]));
    }

    #[test]
    fn minutes_show_as_hours_and_minutes() {
        assert_eq!(minutes_to_time(0), "00:00");
        assert_eq!(minutes_to_time(7 * 60 + 5), "07:05");
        assert_eq!(minutes_to_time(23 * 60 + 59), "23:59");
    }

    #[test]
    fn a_time_input_value_becomes_minutes() {
        assert_eq!(time_to_minutes("08:30"), Some(510));
        assert_eq!(time_to_minutes("00:00"), Some(0));
        assert_eq!(time_to_minutes("23:59"), Some(1439));
    }

    #[test]
    fn a_bad_time_value_gives_nothing() {
        assert_eq!(time_to_minutes("24:00"), None);
        assert_eq!(time_to_minutes("12:60"), None);
        assert_eq!(time_to_minutes("1230"), None);
        assert_eq!(time_to_minutes("8:05"), None);
        assert_eq!(time_to_minutes("12:30:00"), None);
        assert_eq!(time_to_minutes(""), None);
    }

    #[test]
    fn a_positive_number_is_a_user_id() {
        assert_eq!(valid_user_id("1048293"), Some(1_048_293));
    }

    #[test]
    fn a_user_id_must_be_digits_above_zero() {
        assert_eq!(valid_user_id("+5"), None);
        assert_eq!(valid_user_id("-5"), None);
        assert_eq!(valid_user_id("0"), None);
        assert_eq!(valid_user_id(" 12"), None);
        assert_eq!(valid_user_id(""), None);
        assert_eq!(valid_user_id("99999999999999999999"), None);
    }

    #[test]
    fn turning_quiet_hours_on_starts_from_the_default_window() {
        assert_eq!(quiet_switched(None, true), Some(DEFAULT_QUIET));
        assert_eq!(quiet_switched(None, false), None);
    }

    #[test]
    fn an_edit_moves_only_its_own_end_of_the_window() {
        let moved = quiet_edited(Some(DEFAULT_QUIET), QuietEdge::End, 6 * 60);
        assert_eq!(moved.start_minute, DEFAULT_QUIET.start_minute);
        assert_eq!(moved.end_minute, 6 * 60);
    }

    #[test]
    fn only_telegram_events_give_an_activity_line() {
        let events = [
            record(Event::TelegramPaired { user_id: 42 }),
            record(Event::HostHeartbeat),
            record(Event::TelegramSent {
                summary: "test message sent".to_owned(),
            }),
        ];
        assert_eq!(
            activity_lines(&events),
            vec![
                "00:00 Chat paired with user ID 42".to_owned(),
                "00:00 Sent: test message sent".to_owned(),
            ]
        );
    }

    #[test]
    fn a_refused_command_is_marked_refused() {
        let events = [record(Event::TelegramReceived {
            accepted: false,
            command: "/bypass".to_owned(),
        })];
        assert_eq!(
            activity_lines(&events),
            vec!["00:00 Refused /bypass".to_owned()]
        );
    }
}
