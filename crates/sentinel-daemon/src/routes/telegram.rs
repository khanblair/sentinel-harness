//! The Telegram routes under `/api/telegram`. A phone may read the state. Saving the token, the
//! chat, the levels, the events, the detail, and the quiet hours need the computer. The token
//! goes to the keychain and comes back nowhere, not even to the computer.

use std::collections::BTreeSet;

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use sentinel_core::Event;
use sentinel_core::telegram_api::{
    ConnectBody, QuietDto, SettingsUpdate, TelegramState, TokenBody, TokenSaved,
};
use sentinel_telegram::levels::{ChatSettings, Detail, EventKind, Level};
use sentinel_telegram::quiet::QuietHours;
use sentinel_telegram::{BotClient, BotInfo};
use serde_json::Value;

use super::{ApiFailure, ApiResult, blocking, okay};
use crate::auth::Computer;
use crate::engine::{Engine, EngineError, Result};
use crate::telegram;
use crate::telegram_store::{self, ChatRow, EVENT_NAMES};

/// Minutes in a day. Quiet hours are minutes past midnight, below this.
const MINUTES_PER_DAY: u16 = 1440;

pub(super) fn routes() -> Router<Engine> {
    Router::new()
        .route("/api/telegram", get(read_state).put(save_settings))
        .route("/api/telegram/token", post(save_token))
        .route("/api/telegram/connect", post(connect))
        .route("/api/telegram/test", post(send_test))
        .route("/api/telegram/chats/{id}", delete(revoke_chat))
}

async fn read_state(State(engine): State<Engine>) -> ApiResult<TelegramState> {
    blocking(engine, build_state).await
}

async fn save_settings(
    State(engine): State<Engine>,
    Computer(_): Computer,
    Json(body): Json<SettingsUpdate>,
) -> ApiResult<TelegramState> {
    blocking(engine, move |e| {
        apply_settings(e, &body)?;
        build_state(e)
    })
    .await
}

async fn save_token(
    State(engine): State<Engine>,
    Computer(_): Computer,
    Json(body): Json<TokenBody>,
) -> ApiResult<TokenSaved> {
    let token = body.token.trim().to_owned();
    let bot = bot_info(&engine, &token).await?;
    blocking(engine, move |e| {
        telegram::save_token(e, &token)?;
        let bot_username = bot.username.unwrap_or_default();
        telegram_store::set_bot_name(e, &bot_username)?;
        Ok::<_, EngineError>(TokenSaved { bot_username })
    })
    .await
}

async fn connect(
    State(engine): State<Engine>,
    Computer(_): Computer,
    Json(body): Json<ConnectBody>,
) -> ApiResult<TelegramState> {
    let typed = body
        .token
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let (token, save_token) = match typed {
        Some(token) => (token.to_owned(), true),
        None => (saved_token(&engine).await?, false),
    };
    let bot = bot_info(&engine, &token).await?;
    let plan = ConnectPlan {
        token,
        save_token,
        user_id: body.user_id,
        bot_name: bot.username.unwrap_or_default(),
    };
    blocking(engine, move |e| {
        finish_connect(e, &plan)?;
        build_state(e)
    })
    .await
}

async fn send_test(State(engine): State<Engine>, Computer(_): Computer) -> ApiResult<Value> {
    let (chat_id, token) = off_thread(engine.clone(), paired_chat).await?;
    let base_url = engine.config().telegram.base_url;
    let client = BotClient::new(&base_url, &token);
    client
        .send_message(chat_id, TEST_TEXT, None)
        .await
        .map_err(|_| {
            ApiFailure::new(
                StatusCode::BAD_GATEWAY,
                "Telegram did not take the message.",
            )
        })?;
    engine.emit_quiet(&Event::TelegramSent {
        summary: "test message sent".to_owned(),
    });
    Ok(axum::Json(okay()))
}

async fn revoke_chat(
    State(engine): State<Engine>,
    Computer(_): Computer,
    Path(id): Path<String>,
) -> ApiResult<TelegramState> {
    blocking(engine, move |e| {
        let now = e.now();
        if !telegram_store::revoke(e, &id, now)? {
            return Err(EngineError::NotFound("that Telegram chat".to_owned()));
        }
        telegram_store::set_paired(e, false)?;
        telegram::stop(e)?;
        build_state(e)
    })
    .await
}

const TEST_TEXT: &str = "Sentinel test message. This chat is connected.";

/// What `connect` needs once the bot answered: the token, whether to keep it, and the owner.
struct ConnectPlan {
    token: String,
    save_token: bool,
    user_id: i64,
    bot_name: String,
}

fn build_state(engine: &Engine) -> Result<TelegramState> {
    let row = telegram_store::load(engine)?;
    let paired = telegram_store::is_paired(engine)?;
    let token_saved = telegram::saved_token(engine)?.is_some();
    let bot_username = telegram_store::bot_name(engine)?;
    let mut state = TelegramState {
        connected: row.is_some() && paired,
        waiting_for_start: row.is_some() && !paired,
        bot_username,
        token_saved,
        user_id: None,
        levels: Vec::new(),
        events: Vec::new(),
        detail: telegram_store::detail_name(Detail::Detail).to_owned(),
        quiet_hours: None,
    };
    if let Some(row) = row {
        fill_from_row(&mut state, &row);
    }
    Ok(state)
}

fn fill_from_row(state: &mut TelegramState, row: &ChatRow) {
    state.user_id = Some(row.user_id);
    state.levels = level_names(&row.settings.levels);
    state.events = EVENT_NAMES
        .iter()
        .filter(|(kind, _)| row.settings.events.contains(kind))
        .map(|(_, name)| (*name).to_owned())
        .collect();
    telegram_store::detail_name(row.settings.detail).clone_into(&mut state.detail);
    state.quiet_hours = row.settings.quiet_hours.map(|hours| QuietDto {
        start_minute: hours.start_minute,
        end_minute: hours.end_minute,
    });
}

/// The level names a chat has on, in the order of the settings page.
fn level_names(levels: &BTreeSet<Level>) -> Vec<String> {
    telegram_store::LEVEL_ORDER
        .into_iter()
        .filter(|level| levels.contains(level))
        .map(|level| level_name(level).to_owned())
        .collect()
}

fn level_name(level: Level) -> &'static str {
    match level {
        Level::Notify => "notify",
        Level::Control => "control",
        Level::Code => "code",
        Level::Reports => "reports",
    }
}

fn level_from_name(name: &str) -> Result<Level> {
    telegram_store::LEVEL_ORDER
        .into_iter()
        .find(|level| level_name(*level) == name)
        .ok_or_else(|| EngineError::Invalid(format!("Unknown level {name}.")))
}

fn apply_settings(engine: &Engine, body: &SettingsUpdate) -> Result<()> {
    let Some(mut row) = telegram_store::load(engine)? else {
        return Err(EngineError::NotFound(
            "a connected Telegram chat".to_owned(),
        ));
    };
    row.settings = settings_from(body)?;
    telegram_store::save(engine, &row)
}

fn settings_from(body: &SettingsUpdate) -> Result<ChatSettings> {
    let detail = telegram_store::detail_from_name(&body.detail)
        .ok_or_else(|| EngineError::Invalid("Unknown detail level.".to_owned()))?;
    let events = body
        .events
        .iter()
        .map(|name| {
            telegram_store::event_from_name(name)
                .ok_or_else(|| EngineError::Invalid(format!("Unknown event {name}.")))
        })
        .collect::<Result<BTreeSet<EventKind>>>()?;
    let quiet_hours = body.quiet_hours.map(quiet_from_dto).transpose()?;
    Ok(ChatSettings {
        levels: levels_from_names(&body.levels)?,
        detail,
        events,
        quiet_hours,
    })
}

fn levels_from_names(names: &[String]) -> Result<BTreeSet<Level>> {
    names.iter().map(|name| level_from_name(name)).collect()
}

fn quiet_from_dto(dto: QuietDto) -> Result<QuietHours> {
    if dto.start_minute >= MINUTES_PER_DAY || dto.end_minute >= MINUTES_PER_DAY {
        return Err(EngineError::Invalid(
            "Quiet hours are minutes past midnight, below 1440.".to_owned(),
        ));
    }
    Ok(QuietHours {
        start_minute: dto.start_minute,
        end_minute: dto.end_minute,
    })
}

/// Saves the token, drops the older chat, and starts the tasks. A new owner must send `/start`.
fn finish_connect(engine: &Engine, plan: &ConnectPlan) -> Result<()> {
    if plan.save_token {
        telegram::save_token(engine, &plan.token)?;
    }
    telegram::stop(engine)?;
    let now = engine.now();
    let previous = telegram_store::load(engine)?;
    let settings = keep_or_default(previous.as_ref(), plan.user_id);
    if let Some(old) = previous.filter(|old| old.user_id != plan.user_id) {
        telegram_store::revoke(engine, &old.id, now)?;
    }
    let row = ChatRow {
        id: plan.user_id.to_string(),
        user_id: plan.user_id,
        settings,
        connected_at: now,
    };
    telegram_store::save(engine, &row)?;
    telegram_store::set_paired(engine, false)?;
    telegram_store::set_bot_name(engine, &plan.bot_name)?;
    telegram::start(engine)
}

/// The same owner keeps the settings they chose. A new owner starts with Notify only, every event
/// on, and the Detail level, which is the default.
fn keep_or_default(previous: Option<&ChatRow>, user_id: i64) -> ChatSettings {
    match previous {
        Some(old) if old.user_id == user_id => old.settings.clone(),
        _ => ChatSettings {
            levels: BTreeSet::from([Level::Notify]),
            detail: Detail::Detail,
            events: EVENT_NAMES.iter().map(|(kind, _)| *kind).collect(),
            quiet_hours: None,
        },
    }
}

fn paired_chat(engine: &Engine) -> Result<(i64, String)> {
    let row = telegram_store::load(engine)?
        .ok_or_else(|| EngineError::NotFound("a connected Telegram chat".to_owned()))?;
    if !telegram_store::is_paired(engine)? {
        return Err(EngineError::Invalid(
            "Send /start to the bot first.".to_owned(),
        ));
    }
    let token = telegram::saved_token(engine)?
        .ok_or_else(|| EngineError::Invalid("Save the bot token first.".to_owned()))?;
    Ok((row.user_id, token))
}

/// Runs one store or keychain job off the async runtime. Its value stays in this process: it is
/// never serialized into a response.
async fn off_thread<T, F>(engine: Engine, job: F) -> std::result::Result<T, ApiFailure>
where
    T: Send + 'static,
    F: FnOnce(&Engine) -> Result<T> + Send + 'static,
{
    let outcome = tokio::task::spawn_blocking(move || job(&engine))
        .await
        .map_err(|_| {
            ApiFailure::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "The computer could not finish that.",
            )
        })?;
    outcome.map_err(ApiFailure::from)
}

async fn saved_token(engine: &Engine) -> std::result::Result<String, ApiFailure> {
    let token = off_thread(engine.clone(), telegram::saved_token).await?;
    token.ok_or_else(|| ApiFailure::new(StatusCode::BAD_REQUEST, "Save the bot token first."))
}

async fn bot_info(engine: &Engine, token: &str) -> std::result::Result<BotInfo, ApiFailure> {
    let base_url = engine.config().telegram.base_url;
    BotClient::new(&base_url, token)
        .get_me()
        .await
        .map_err(|_| {
            ApiFailure::new(
                StatusCode::BAD_REQUEST,
                "Telegram did not accept the bot token.",
            )
        })
}
