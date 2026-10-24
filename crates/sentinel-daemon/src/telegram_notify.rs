//! The notifier turns bus events into queued messages. The sender drains the queue to the Bot API
//! and reports each result back to the queue. Both run in one task, so the queue has one writer.

use std::sync::{Arc, Weak};

use sentinel_core::{Event, EventRecord};
use sentinel_telegram::TelegramError;
use sentinel_telegram::buttons::ButtonTokens;
use sentinel_telegram::format::{self, ButtonSpec, RenderContext, Rendered};
use sentinel_telegram::outbox::{Delivery, Draft, OutItem, Outbox};
use sentinel_telegram::{Button, MessageRef};
use tokio::sync::broadcast;

use crate::engine::{Engine, Inner, Result, lock};
use crate::telegram::{DRAIN_EVERY, Shared, is_stop, unix_now, upgrade, utc_offset_minutes};
use crate::telegram_store;

/// How far back the start scan looks for a `HostBackOnline` event, in event ids.
const HOST_NEWS_WINDOW: i64 = 200;

pub(crate) async fn run(
    weak: Weak<Inner>,
    shared: Arc<Shared>,
    mut events: broadcast::Receiver<EventRecord>,
) {
    if let Some(engine) = upgrade(&weak) {
        for event in host_news(&engine).unwrap_or_default() {
            queue_for(&engine, &shared, &event);
        }
        drain(&engine, &shared).await;
    }
    let mut ticker = tokio::time::interval(DRAIN_EVERY);
    loop {
        let keep_going = tokio::select! {
            received = events.recv() => take_event(&weak, &shared, received),
            _ = ticker.tick() => true,
        };
        if !keep_going {
            return;
        }
        let Some(engine) = upgrade(&weak) else {
            return;
        };
        drain(&engine, &shared).await;
    }
}

/// Queues a bus event. Returns `false` when the task should end: a stop event, a closed bus, or a
/// dropped engine.
fn take_event(
    weak: &Weak<Inner>,
    shared: &Shared,
    received: std::result::Result<EventRecord, broadcast::error::RecvError>,
) -> bool {
    match received {
        Ok(record) if is_stop(&record.event) => false,
        Ok(record) => match upgrade(weak) {
            Some(engine) => {
                queue_for(&engine, shared, &record.event);
                true
            }
            None => false,
        },
        Err(broadcast::error::RecvError::Lagged(_)) => true,
        Err(broadcast::error::RecvError::Closed) => false,
    }
}

/// Host-back events emitted before the tasks subscribed. Broadcast does not replay them.
fn host_news(engine: &Engine) -> Result<Vec<Event>> {
    let last = engine.store().last_event_id()?;
    let from = (last - HOST_NEWS_WINDOW).max(0);
    let started = engine.inner.started;
    let news = engine
        .events_after(from, 200)?
        .into_iter()
        .filter(|record| record.at >= started)
        .filter(|record| matches!(record.event, Event::HostBackOnline { .. }))
        .map(|record| record.event)
        .collect();
    Ok(news)
}

/// Queues one event for the owner's chat, if the chat is connected and paired.
fn queue_for(engine: &Engine, shared: &Shared, event: &Event) {
    let Ok(Some(row)) = telegram_store::load(engine) else {
        return;
    };
    if !telegram_store::is_paired(engine).unwrap_or(false) {
        return;
    }
    let project = project_name(engine);
    let ctx = RenderContext {
        settings: &row.settings,
        project: &project,
        hackathon: engine.config().hackathon_mode,
        cost_line: None,
    };
    let stamp = Stamp {
        chat_id: row.user_id,
        now: unix_now(engine),
    };
    let mut outbox = lock(&shared.outbox);
    let mut tokens = lock(&shared.tokens);
    let mut queues = Queues {
        outbox: &mut outbox,
        tokens: &mut tokens,
    };
    queue_event(&mut queues, stamp, event, &ctx);
}

/// The two queues the notifier writes: the outbox and the button tokens.
pub(crate) struct Queues<'a> {
    pub outbox: &'a mut Outbox,
    pub tokens: &'a mut ButtonTokens,
}

/// The chat a message goes to, and the time it was made.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Stamp {
    pub chat_id: i64,
    pub now: i64,
}

/// Renders the event and queues it. Returns the queue id, or `None` when the chat gets nothing.
/// Buttons get one-time tokens here, so a press can be checked later.
pub(crate) fn queue_event(
    queues: &mut Queues<'_>,
    stamp: Stamp,
    event: &Event,
    ctx: &RenderContext<'_>,
) -> Option<u64> {
    let rendered = format::render(event, ctx)?;
    queues
        .outbox
        .set_quiet_hours(ctx.settings.quiet_hours, utc_offset_minutes());
    let keyboard = issue_keyboard(queues.tokens, stamp, &rendered);
    queues.outbox.push(
        Draft {
            chat_id: stamp.chat_id,
            text: rendered.text,
            keyboard,
            priority: rendered.priority,
            key: None,
        },
        stamp.now,
    )
}

fn issue_keyboard(
    tokens: &mut ButtonTokens,
    stamp: Stamp,
    rendered: &Rendered,
) -> Vec<Vec<Button>> {
    rendered
        .buttons
        .iter()
        .map(|row| {
            row.iter()
                .map(|spec: &ButtonSpec| Button {
                    label: spec.label.clone(),
                    data: tokens.issue(stamp.chat_id, &spec.action, stamp.now),
                })
                .collect()
        })
        .collect()
}

/// Sends every message that is due now. A failure goes back to the queue with its wait.
async fn drain(engine: &Engine, shared: &Shared) {
    loop {
        let now = unix_now(engine);
        let next: Option<OutItem> = lock(&shared.outbox).next_due(now).cloned();
        let Some(item) = next else {
            return;
        };
        let outcome = deliver(shared, &item).await;
        record_outcome(engine, shared, &item, outcome);
    }
}

/// Moves the item out of the queue when it was sent, or schedules its retry when it was not.
fn record_outcome(
    engine: &Engine,
    shared: &Shared,
    item: &OutItem,
    outcome: std::result::Result<i64, TelegramError>,
) {
    let now = unix_now(engine);
    let mut outbox = lock(&shared.outbox);
    match outcome {
        Ok(message_id) => {
            outbox.mark_sent(item.id, message_id, now);
            engine.emit_quiet(&Event::TelegramSent {
                summary: "message sent".to_owned(),
            });
        }
        Err(error) => outbox.mark_failed(item.id, now, retry_after(&error)),
    }
}

async fn deliver(shared: &Shared, item: &OutItem) -> std::result::Result<i64, TelegramError> {
    let keyboard = (!item.keyboard.is_empty()).then_some(item.keyboard.as_slice());
    match item.delivery {
        Delivery::Send => {
            shared
                .client
                .send_message(item.chat_id, &item.text, keyboard)
                .await
        }
        Delivery::Edit(message_id) => {
            let target = MessageRef {
                chat_id: item.chat_id,
                message_id,
            };
            shared
                .client
                .edit_message(target, &item.text, keyboard)
                .await
                .map(|()| message_id)
        }
    }
}

fn retry_after(error: &TelegramError) -> Option<u32> {
    match error {
        TelegramError::RateLimited {
            retry_after_seconds,
        } => Some(*retry_after_seconds),
        TelegramError::Transport(_) | TelegramError::Api(_) | TelegramError::Decode(_) => None,
    }
}

fn project_name(engine: &Engine) -> String {
    engine
        .repo_path()
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use sentinel_core::{Approval, ApprovalId, ApprovalKind, ApprovalState, Event, HarnessState};
    use sentinel_telegram::buttons::ButtonTokens;
    use sentinel_telegram::format::RenderContext;
    use sentinel_telegram::levels::{ChatSettings, EventKind, Level};
    use sentinel_telegram::outbox::Outbox;
    use sentinel_telegram::quiet::QuietHours;

    use super::{Queues, Stamp, queue_event};

    const CHAT: i64 = 42;
    /// 23:00 UTC on 1970-01-01, inside the night window below.
    const LATE_NIGHT: i64 = 23 * 3600;

    /// Notify on, with the approval and crash switches on, and quiet hours from 22:00 to 07:00.
    fn night_settings() -> ChatSettings {
        ChatSettings {
            levels: BTreeSet::from([Level::Notify]),
            events: BTreeSet::from([EventKind::ApprovalNeeded, EventKind::Errors]),
            quiet_hours: Some(QuietHours {
                start_minute: 22 * 60,
                end_minute: 7 * 60,
            }),
            ..ChatSettings::default()
        }
    }

    fn context(settings: &ChatSettings) -> RenderContext<'_> {
        RenderContext {
            settings,
            project: "demo",
            hackathon: false,
            cost_line: None,
        }
    }

    fn pending_approval() -> Event {
        Event::ApprovalRequested {
            approval: Approval {
                id: ApprovalId::new("approval-1"),
                kind: ApprovalKind::Extension,
                task: None,
                agent: None,
                path: None,
                state: ApprovalState::Pending,
                decided_by: None,
            },
        }
    }

    #[test]
    fn a_normal_event_is_queued_but_held_during_quiet_hours() {
        let settings = night_settings();
        let mut outbox = Outbox::new();
        let mut tokens = ButtonTokens::new();
        let crash = Event::HarnessState {
            state: HarnessState::Crashed,
        };
        let mut queues = Queues {
            outbox: &mut outbox,
            tokens: &mut tokens,
        };
        let stamp = Stamp {
            chat_id: CHAT,
            now: LATE_NIGHT,
        };
        let queue_id = queue_event(&mut queues, stamp, &crash, &context(&settings));
        assert!(queue_id.is_some(), "the crash message should be queued");
        assert!(
            outbox.next_due(LATE_NIGHT).is_none(),
            "a normal message must wait for the window to end"
        );
    }

    #[test]
    fn an_approval_request_goes_out_during_quiet_hours() {
        let settings = night_settings();
        let mut outbox = Outbox::new();
        let mut tokens = ButtonTokens::new();
        let mut queues = Queues {
            outbox: &mut outbox,
            tokens: &mut tokens,
        };
        let stamp = Stamp {
            chat_id: CHAT,
            now: LATE_NIGHT,
        };
        queue_event(&mut queues, stamp, &pending_approval(), &context(&settings));
        assert!(
            outbox.next_due(LATE_NIGHT).is_some(),
            "an approval is urgent and must not be held"
        );
    }
}
