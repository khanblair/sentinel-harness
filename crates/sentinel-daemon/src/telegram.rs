//! Telegram inside the daemon. The `sentinel-telegram` crate holds the Bot API client, the parser,
//! the formatter, and the outbox. This module starts the background tasks and stops them. Each
//! task ends when the chat is revoked or reconnected, when the engine drops, or when a loop fails.

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use sentinel_core::{ConnectionState, Event, EventRecord};
use sentinel_telegram::BotClient;
use sentinel_telegram::buttons::ButtonTokens;
use sentinel_telegram::outbox::Outbox;
use tokio::sync::broadcast;

use crate::engine::{Engine, EngineError, Inner, Result};
use crate::telegram_store;

/// The keychain key of the bot token. It never reaches the database, a log, or a response.
pub const TOKEN_KEY: &str = "telegram.bot_token";
/// The settings key that says the owner sent `/start`.
pub const PAIRED_KEY: &str = "telegram.paired";
/// The settings key with the bot's username from `getMe`.
pub const BOT_NAME_KEY: &str = "telegram.bot_username";
/// The name the router uses for every call a chat makes.
pub const CALLER_NAME: &str = "telegram";
/// The `ConnectionChanged` name that stops the tasks.
const CONNECTION_NAME: &str = "telegram";
/// Seconds Telegram holds a long poll open.
pub(crate) const POLL_SECONDS: u32 = 25;
/// How often the sender checks the queue when no event arrives.
pub(crate) const DRAIN_EVERY: Duration = Duration::from_secs(1);
/// Quiet hours are read at this offset. The daemon has no local-time setting yet, so this is UTC.
pub(crate) const UTC_OFFSET_MINUTES: i32 = 0;

/// What the three tasks share: the bot client, the one-time buttons, and the queue.
pub(crate) struct Shared {
    pub client: BotClient,
    pub tokens: Mutex<ButtonTokens>,
    pub outbox: Mutex<Outbox>,
}

impl Shared {
    fn new(client: BotClient) -> Self {
        Self {
            client,
            tokens: Mutex::new(ButtonTokens::new()),
            outbox: Mutex::new(Outbox::new()),
        }
    }
}

/// Starts the tasks when a chat is connected and its token is saved. Returns `false` when there is
/// nothing to run. The daemon calls this at start.
pub fn start_if_active(engine: &Engine) -> Result<bool> {
    if telegram_store::load(engine)?.is_none() || saved_token(engine)?.is_none() {
        return Ok(false);
    }
    start(engine)?;
    Ok(true)
}

/// Starts the poller and the notifier. The caller stops an older pair first with `stop`.
pub fn start(engine: &Engine) -> Result<()> {
    let token = saved_token(engine)?
        .ok_or_else(|| EngineError::Invalid("Save the bot token first.".to_owned()))?;
    let base_url = engine.config().telegram.base_url;
    let shared = Arc::new(Shared::new(BotClient::new(&base_url, &token)));
    let weak = Arc::downgrade(&engine.inner);
    let poll_events = engine.subscribe();
    let notify_events = engine.subscribe();
    tokio::spawn(crate::telegram_poll::run(
        weak.clone(),
        Arc::clone(&shared),
        poll_events,
    ));
    tokio::spawn(crate::telegram_notify::run(weak, shared, notify_events));
    Ok(())
}

/// Tells the running tasks to end. Each one sees this event on the bus and returns.
pub fn stop(engine: &Engine) -> Result<()> {
    engine.emit(&Event::ConnectionChanged {
        name: CONNECTION_NAME.to_owned(),
        state: ConnectionState::Missing,
    })?;
    Ok(())
}

pub(crate) fn saved_token(engine: &Engine) -> Result<Option<String>> {
    let value = engine.inner.secrets.get(TOKEN_KEY);
    value.map_err(|error| EngineError::Invalid(error.to_string()))
}

pub(crate) fn save_token(engine: &Engine, token: &str) -> Result<()> {
    let saved = engine.inner.secrets.set(TOKEN_KEY, token);
    saved.map_err(|error| EngineError::Invalid(error.to_string()))
}

pub(crate) fn upgrade(weak: &Weak<Inner>) -> Option<Engine> {
    weak.upgrade().map(|inner| Engine { inner })
}

pub(crate) fn is_stop(event: &Event) -> bool {
    matches!(
        event,
        Event::ConnectionChanged { name, state: ConnectionState::Missing } if name == CONNECTION_NAME
    )
}

/// Waits until a stop event arrives or the bus closes. A lagged receiver keeps waiting, and the
/// poller also checks the chat row after each poll, so a missed stop still ends it.
pub(crate) async fn wait_for_stop(events: &mut broadcast::Receiver<EventRecord>) {
    loop {
        match events.recv().await {
            Ok(record) if is_stop(&record.event) => return,
            Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => return,
        }
    }
}

/// The unix time in seconds. Button tokens and the outbox both use it.
pub(crate) fn unix_now(engine: &Engine) -> i64 {
    engine.now().unix_timestamp()
}
