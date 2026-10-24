//! Telegram settings and activity: the pure rules the card and the panel share, and the calls to
//! the daemon's `/api/telegram` routes. The bot token passes through `save_and_check` once. It is
//! never kept in a signal, logged, or shown in a toast.

use sentinel_core::telegram_api::{QuietDto, SettingsUpdate, TelegramState, TokenSaved};
use sentinel_core::{Event, EventRecord};
use serde::Serialize;
use time::UtcOffset;

use crate::api::{self, ApiFailure};

/// One row of a choice list: the key the daemon stores, the words shown, and an optional hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    pub key: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
}

const fn choice(key: &'static str, label: &'static str, hint: &'static str) -> Choice {
    Choice { key, label, hint }
}

/// The four levels in the order of the settings page. Keys are the daemon's level names.
pub const LEVELS: [Choice; 4] = [
    choice("notify", "Notify", "Sentinel sends events to this chat."),
    choice(
        "control",
        "Control",
        "Approve or deny, answer decisions, pause, resume, stop, kill an agent, lower the cap.",
    ),
    choice(
        "code",
        "Code",
        "Message one agent directly and ask for a short diff summary.",
    ),
    choice(
        "reports",
        "Reports",
        "A summary when a run finishes, and a daily summary, with cost and Brain proposals.",
    ),
];

/// The nine events the daemon sends to a chat. Budget has one event per threshold.
pub const EVENTS: [Choice; 9] = [
    choice("approval_needed", "Approval needed", ""),
    choice("decision_needed", "Decision needed", ""),
    choice("budget_70", "Budget at 70%", ""),
    choice("budget_95", "Budget at 95%", ""),
    choice("task_blocked", "Task blocked", ""),
    choice("wave_finished", "Wave finished", ""),
    choice("run_finished", "Run finished", ""),
    choice("brain_proposals", "Brain care proposals", ""),
    choice("errors", "Errors and crashes", ""),
];

/// How much a message shows. Keys are the daemon's detail names.
pub const DETAILS: [Choice; 3] = [
    choice("status", "Status only", ""),
    choice("summary", "Summary", ""),
    choice("detail", "Detail", ""),
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

/// Copies a changed update into the state, so the change shows before the daemon answers.
pub fn apply_settings(state: &mut TelegramState, update: &SettingsUpdate) {
    state.levels.clone_from(&update.levels);
    state.events.clone_from(&update.events);
    state.detail.clone_from(&update.detail);
    state.quiet_hours = update.quiet_hours;
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

/// The words of a Telegram event. Other events have none, so they stay out of the log.
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
    connect(user_id).await
}

/// Waits for `/start` from `user_id` with the token already saved. The daemon checks the saved
/// token again, so this works after a revoke too.
pub async fn connect(user_id: i64) -> Result<TelegramState, ApiFailure> {
    let body = ConnectOut {
        token: None,
        user_id,
    };
    api::post("/api/telegram/connect", &body).await
}

pub async fn send_test() -> Result<serde_json::Value, ApiFailure> {
    api::post_empty("/api/telegram/test").await
}

/// Revokes the chat. The daemon also stops the bot's tasks.
pub async fn revoke(user_id: i64) -> Result<TelegramState, ApiFailure> {
    let path = format!("/api/telegram/chats/{user_id}");
    api::send("DELETE", &path, &serde_json::json!({})).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn state(connected: bool, waiting: bool) -> TelegramState {
        TelegramState {
            connected,
            waiting_for_start: waiting,
            bot_username: None,
            token_saved: connected || waiting,
            user_id: None,
            levels: Vec::new(),
            events: Vec::new(),
            detail: "detail".to_owned(),
            quiet_hours: None,
        }
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
    fn the_chip_follows_the_connection_state() {
        assert_eq!(chip_label(&state(true, false)), "Connected");
        assert_eq!(chip_label(&state(false, true)), "Waiting for /start");
        assert_eq!(chip_label(&state(false, false)), "Not connected");
    }

    #[test]
    fn a_level_switches_on_and_off_in_settings_order() {
        let on = levels_with(&keys(&["code", "notify"]), "control", true);
        assert_eq!(on, keys(&["notify", "control", "code"]));
        assert_eq!(levels_with(&on, "notify", true), on);
        assert_eq!(
            levels_with(&on, "notify", false),
            keys(&["control", "code"])
        );
    }

    #[test]
    fn an_event_switches_off_without_touching_the_others() {
        let now = events_with(&keys(&["budget_95", "errors"]), "budget_95", false);
        assert_eq!(now, keys(&["errors"]));
    }

    #[test]
    fn minutes_show_as_hours_and_minutes() {
        assert_eq!(minutes_to_time(0), "00:00");
        assert_eq!(minutes_to_time(7 * 60 + 5), "07:05");
        assert_eq!(minutes_to_time(23 * 60 + 59), "23:59");
    }

    #[test]
    fn a_time_input_value_becomes_minutes_and_bad_values_give_nothing() {
        assert_eq!(time_to_minutes("08:30"), Some(510));
        assert_eq!(time_to_minutes("23:59"), Some(1439));
        for bad in ["24:00", "12:60", "1230", "8:05", "12:30:00", ""] {
            assert_eq!(time_to_minutes(bad), None, "{bad} should not parse");
        }
    }

    #[test]
    fn a_user_id_is_digits_above_zero() {
        assert_eq!(valid_user_id("1048293"), Some(1_048_293));
        for bad in ["+5", "-5", "0", " 12", "", "99999999999999999999"] {
            assert_eq!(valid_user_id(bad), None, "{bad} should be refused");
        }
    }

    #[test]
    fn quiet_hours_start_from_the_default_window() {
        assert_eq!(quiet_switched(None, true), Some(DEFAULT_QUIET));
        assert_eq!(quiet_switched(None, false), None);
        let moved = quiet_edited(Some(DEFAULT_QUIET), QuietEdge::End, 6 * 60);
        assert_eq!(moved.start_minute, DEFAULT_QUIET.start_minute);
        assert_eq!(moved.end_minute, 6 * 60);
    }

    #[test]
    fn only_telegram_events_give_an_activity_line() {
        let events = [
            record(Event::TelegramPaired { user_id: 42 }),
            record(Event::HostHeartbeat),
            record(Event::TelegramReceived {
                accepted: false,
                command: "/bypass".to_owned(),
            }),
        ];
        assert_eq!(
            activity_lines(&events),
            keys(&["00:00 Chat paired with user ID 42", "00:00 Refused /bypass"])
        );
    }
}
