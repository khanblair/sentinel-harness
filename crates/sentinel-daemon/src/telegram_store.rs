//! The Telegram chat row and the paired flag, read and written by the daemon. The store keeps
//! plain columns and JSON text. This file turns them into the crate's `ChatSettings` and back.

use std::collections::BTreeSet;

use sentinel_store::telegram::{LevelSwitches, TelegramChatRecord};
use sentinel_telegram::levels::{ChatSettings, Detail, EventKind, Level};
use sentinel_telegram::quiet::QuietHours;
use serde_json::{Value, json};
use time::OffsetDateTime;

use crate::engine::{Engine, EngineError, Result};
use crate::telegram::{BOT_NAME_KEY, PAIRED_KEY};

/// One chat row, with its settings read into typed values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRow {
    pub id: String,
    pub user_id: i64,
    pub settings: ChatSettings,
    pub connected_at: OffsetDateTime,
}

/// The wire name of each event switch. The order is the order the settings page lists them.
pub const EVENT_NAMES: [(EventKind, &str); 9] = [
    (EventKind::ApprovalNeeded, "approval_needed"),
    (EventKind::DecisionNeeded, "decision_needed"),
    (EventKind::Budget70, "budget_70"),
    (EventKind::Budget95, "budget_95"),
    (EventKind::TaskBlocked, "task_blocked"),
    (EventKind::WaveFinished, "wave_finished"),
    (EventKind::RunFinished, "run_finished"),
    (EventKind::BrainProposals, "brain_proposals"),
    (EventKind::Errors, "errors"),
];

pub fn event_name(kind: EventKind) -> &'static str {
    EVENT_NAMES
        .iter()
        .find(|(known, _)| *known == kind)
        .map_or("", |(_, name)| name)
}

pub fn event_from_name(name: &str) -> Option<EventKind> {
    EVENT_NAMES
        .iter()
        .find(|(_, known)| *known == name)
        .map(|(kind, _)| *kind)
}

pub fn detail_name(detail: Detail) -> &'static str {
    match detail {
        Detail::Status => "status",
        Detail::Summary => "summary",
        Detail::Detail => "detail",
    }
}

pub fn detail_from_name(name: &str) -> Option<Detail> {
    match name {
        "status" => Some(Detail::Status),
        "summary" => Some(Detail::Summary),
        "detail" => Some(Detail::Detail),
        _ => None,
    }
}

/// The chat row, if one is active.
pub fn load(engine: &Engine) -> Result<Option<ChatRow>> {
    let Some(record) = engine.store().get_telegram_chat()? else {
        return Ok(None);
    };
    row_from_record(record).map(Some)
}

pub fn save(engine: &Engine, row: &ChatRow) -> Result<()> {
    let record = TelegramChatRecord {
        id: row.id.clone(),
        telegram_user_id: row.user_id,
        levels: flags_of(&row.settings.levels),
        detail: detail_name(row.settings.detail).to_owned(),
        events_json: events_text(&row.settings),
        quiet_hours: quiet_text(row.settings.quiet_hours),
        connected_at: row.connected_at,
        revoked_at: None,
    };
    engine.store().upsert_telegram_chat(&record)?;
    Ok(())
}

pub fn revoke(engine: &Engine, id: &str, at: OffsetDateTime) -> Result<bool> {
    Ok(engine.store().revoke_telegram_chat(id, at)?)
}

pub fn is_paired(engine: &Engine) -> Result<bool> {
    Ok(engine.store().get_setting(PAIRED_KEY)?.as_deref() == Some("true"))
}

pub fn set_paired(engine: &Engine, paired: bool) -> Result<()> {
    let text = if paired { "true" } else { "false" };
    engine.store().set_setting(PAIRED_KEY, text)?;
    Ok(())
}

pub fn bot_name(engine: &Engine) -> Result<Option<String>> {
    Ok(engine.store().get_setting(BOT_NAME_KEY)?)
}

pub fn set_bot_name(engine: &Engine, name: &str) -> Result<()> {
    engine.store().set_setting(BOT_NAME_KEY, name)?;
    Ok(())
}

fn row_from_record(record: TelegramChatRecord) -> Result<ChatRow> {
    let detail = detail_from_name(&record.detail).ok_or_else(corrupt)?;
    let events = events_from_text(&record.events_json)?;
    let quiet_hours = quiet_from_text(record.quiet_hours.as_deref())?;
    Ok(ChatRow {
        id: record.id,
        user_id: record.telegram_user_id,
        settings: ChatSettings {
            levels: levels_of(record.levels),
            detail,
            events,
            quiet_hours,
        },
        connected_at: record.connected_at,
    })
}

fn corrupt() -> EngineError {
    EngineError::Invalid("the saved Telegram settings do not read back".to_owned())
}

/// The levels in the order of the store's four switch columns.
pub const LEVEL_ORDER: [Level; 4] = [Level::Notify, Level::Control, Level::Code, Level::Reports];

fn levels_of(switches: LevelSwitches) -> BTreeSet<Level> {
    LEVEL_ORDER
        .into_iter()
        .zip(switches)
        .filter_map(|(level, on)| on.then_some(level))
        .collect()
}

pub fn flags_of(levels: &BTreeSet<Level>) -> LevelSwitches {
    LEVEL_ORDER.map(|level| levels.contains(&level))
}

fn events_text(settings: &ChatSettings) -> String {
    let names: Vec<&str> = settings
        .events
        .iter()
        .map(|kind| event_name(*kind))
        .collect();
    json!(names).to_string()
}

fn events_from_text(text: &str) -> Result<BTreeSet<EventKind>> {
    let names: Vec<String> = serde_json::from_str(text).map_err(|_| corrupt())?;
    names
        .iter()
        .map(|name| event_from_name(name).ok_or_else(corrupt))
        .collect()
}

fn quiet_text(quiet: Option<QuietHours>) -> Option<String> {
    quiet.map(|hours| {
        json!({ "start_minute": hours.start_minute, "end_minute": hours.end_minute }).to_string()
    })
}

fn quiet_from_text(text: Option<&str>) -> Result<Option<QuietHours>> {
    let Some(text) = text else {
        return Ok(None);
    };
    let value: Value = serde_json::from_str(text).map_err(|_| corrupt())?;
    let start = value.get("start_minute").and_then(Value::as_u64);
    let end = value.get("end_minute").and_then(Value::as_u64);
    match (start, end) {
        (Some(start), Some(end)) => Ok(Some(QuietHours {
            start_minute: u16::try_from(start).map_err(|_| corrupt())?,
            end_minute: u16::try_from(end).map_err(|_| corrupt())?,
        })),
        _ => Err(corrupt()),
    }
}
