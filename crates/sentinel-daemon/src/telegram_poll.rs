//! The long-poll loop. It fetches updates, drops the ones from anyone but the owner, and hands the
//! rest to the router. It backs off on errors and ends when the chat is revoked or the engine drops.

use std::sync::{Arc, Weak};
use std::time::Duration;

use sentinel_core::{Event, EventRecord};
use sentinel_telegram::Update;
use tokio::sync::broadcast;

use crate::engine::{Engine, Inner};
use crate::telegram::{POLL_SECONDS, Shared, upgrade, wait_for_stop};
use crate::telegram_router::{self, Press, receipt_name};
use crate::telegram_store::{self, ChatRow};

/// Waits after each failed poll, in seconds. The last wait repeats.
const BACKOFF_SECONDS: [u64; 5] = [1, 2, 4, 8, 30];

pub(crate) async fn run(
    weak: Weak<Inner>,
    shared: Arc<Shared>,
    mut stop: broadcast::Receiver<EventRecord>,
) {
    let mut offset = starting_offset(&weak, &shared).await;
    let mut failures = 0_usize;
    loop {
        let polled = tokio::select! {
            result = shared.client.get_updates(offset, POLL_SECONDS) => result,
            () = wait_for_stop(&mut stop) => return,
        };
        let Some(engine) = upgrade(&weak) else {
            return;
        };
        let Ok(Some(row)) = telegram_store::load(&engine) else {
            return;
        };
        let alive = match polled {
            Ok(updates) => {
                failures = 0;
                offset = next_offset(&updates, offset);
                handle_batch(&engine, &shared, &row, updates).await;
                true
            }
            Err(error) => {
                eprintln!("sentinel: Telegram poll failed: {error}");
                let index = failures;
                failures = failures.saturating_add(1);
                back_off(index, &mut stop).await
            }
        };
        if !alive {
            return;
        }
    }
}

/// Where a new poller starts. A paired chat skips what arrived while the daemon was down, so an
/// old `/pause` or `/stop` cannot replay. A chat that is not paired yet reads from the start, so
/// the `/start` that connects it is never missed.
async fn starting_offset(weak: &Weak<Inner>, shared: &Shared) -> i64 {
    let paired =
        upgrade(weak).is_some_and(|engine| telegram_store::is_paired(&engine).unwrap_or(false));
    if !paired {
        return 0;
    }
    // Telegram answers an offset of -1 with only the newest update.
    match shared.client.get_updates(-1, 0).await {
        Ok(updates) => next_offset(&updates, 0),
        Err(_) => 0,
    }
}

/// The offset that asks Telegram for the updates after this batch.
fn next_offset(updates: &[Update], current: i64) -> i64 {
    updates
        .last()
        .map_or(current, |update| update.update_id + 1)
}

async fn handle_batch(engine: &Engine, shared: &Shared, row: &ChatRow, updates: Vec<Update>) {
    for update in updates {
        dispatch(engine, shared, row, update).await;
    }
}

/// Waits out the failure backoff. Returns `false` when the chat was revoked during the wait.
async fn back_off(failures: usize, stop: &mut broadcast::Receiver<EventRecord>) -> bool {
    let wait = Duration::from_secs(backoff(failures));
    tokio::select! {
        () = tokio::time::sleep(wait) => true,
        () = wait_for_stop(stop) => false,
    }
}

/// Sends a message or a button press to the router when it comes from the owner. Anyone else is
/// logged by command word and gets no reply.
async fn dispatch(engine: &Engine, shared: &Shared, row: &ChatRow, update: Update) {
    if let Some(message) = update.message {
        let text = message.text.unwrap_or_default();
        if !is_owner(row, message.chat_id, message.from_id) {
            log_received(engine, false, receipt_name(&text));
            return;
        }
        telegram_router::on_text(engine, shared, row, &text).await;
    }
    if let Some(query) = update.callback_query {
        if !is_owner(row, query.chat_id, query.from_id) {
            log_received(engine, false, "button");
            return;
        }
        let token = query.data.unwrap_or_default();
        let press = Press {
            callback_id: &query.id,
            token: &token,
        };
        telegram_router::on_press(engine, shared, row, press).await;
    }
}

/// Only the one private chat of the owner counts. Groups and channels are not supported.
fn is_owner(row: &ChatRow, chat_id: i64, from_id: i64) -> bool {
    chat_id == row.user_id && from_id == row.user_id
}

pub(crate) fn log_received(engine: &Engine, accepted: bool, command: &str) {
    let event = Event::TelegramReceived {
        accepted,
        command: command.to_owned(),
    };
    engine.emit_quiet(&event);
}

fn backoff(failures: usize) -> u64 {
    let last = BACKOFF_SECONDS.len() - 1;
    BACKOFF_SECONDS[failures.min(last)]
}

#[cfg(test)]
mod tests {
    use super::backoff;

    #[test]
    fn the_wait_after_each_failed_poll_grows_then_holds_at_thirty_seconds() {
        let waits: Vec<u64> = (0..7).map(backoff).collect();
        assert_eq!(waits, vec![1, 2, 4, 8, 30, 30, 30]);
    }
}
