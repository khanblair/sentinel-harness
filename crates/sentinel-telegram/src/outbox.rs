//! The outgoing queue. A pure state machine: it never sends. The caller asks what is due, sends
//! it, and reports the result. Times are Unix seconds, passed in.

use std::collections::{BTreeMap, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};

use time::{OffsetDateTime, UtcOffset};

use crate::api::Button;
use crate::quiet::{Priority, QuietHours, held};

/// The same message inside this window is dropped as a duplicate.
pub const DEDUPE_SECONDS: i64 = 60;
/// Wait after each failed try. Telegram's `retry_after` replaces the wait when it is given.
pub const BACKOFF_SECONDS: [i64; 5] = [2, 4, 8, 16, 32];
/// Failures past this count mark the message as `failed`. There are five retries after the first try.
pub const MAX_RETRIES: u32 = 5;
/// The bot may send this many messages or edits in any 60 second window.
pub const SENDS_PER_MINUTE: usize = 20;
const WINDOW_SECONDS: i64 = 60;

/// A message before it is queued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub chat_id: i64,
    pub text: String,
    pub keyboard: Vec<Vec<Button>>,
    pub priority: Priority,
    /// Messages with the same key share one live message, which is edited in place.
    pub key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    Send,
    /// Edit the message with this id instead of sending a new one.
    Edit(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutItem {
    pub id: u64,
    pub chat_id: i64,
    pub text: String,
    pub keyboard: Vec<Vec<Button>>,
    pub priority: Priority,
    pub key: Option<String>,
    pub delivery: Delivery,
    pub tries: u32,
    pub next_attempt_at: i64,
    pub failed: bool,
    payload_hash: u64,
}

impl OutItem {
    fn is_due(&self, now: i64) -> bool {
        !self.failed && self.next_attempt_at <= now
    }
}

#[derive(Debug, Default)]
pub struct Outbox {
    items: Vec<OutItem>,
    /// Payload hash to the time it was pushed.
    recent: BTreeMap<u64, i64>,
    /// Times of recent send attempts, for the rate limit.
    attempts: VecDeque<i64>,
    /// (chat, key) to the message id of the live message.
    live: BTreeMap<(i64, String), i64>,
    next_id: u64,
    quiet: Option<QuietHours>,
    utc_offset_minutes: i32,
}

impl Outbox {
    /// `utc_offset_minutes` is the local offset that quiet hours use, such as 180 for EAT.
    pub fn new(quiet: Option<QuietHours>, utc_offset_minutes: i32) -> Self {
        Self {
            quiet,
            utc_offset_minutes,
            ..Self::default()
        }
    }

    /// Returns the queue id, or `None` when the message is a duplicate.
    pub fn push(&mut self, draft: Draft, now: i64) -> Option<u64> {
        self.recent
            .retain(|_, pushed_at| now - *pushed_at < DEDUPE_SECONDS);
        let hash = payload_hash(draft.chat_id, &draft.text);
        if self.recent.contains_key(&hash) {
            return None;
        }
        self.recent.insert(hash, now);
        if let Some(id) = self.replace_queued_live(&draft, hash) {
            return Some(id);
        }
        Some(self.enqueue(draft, hash, now))
    }

    /// The first message that is due, urgent ones first. `None` when the rate limit is used up.
    pub fn next_due(&self, now: i64) -> Option<&OutItem> {
        if self.sends_in_window(now) >= SENDS_PER_MINUTE {
            return None;
        }
        let minute = minute_of_day(now, self.utc_offset_minutes);
        self.items
            .iter()
            .filter(|item| item.is_due(now) && !held(self.quiet, minute, item.priority))
            .min_by_key(|item| (item.priority == Priority::Normal, item.id))
    }

    pub fn item(&self, id: u64) -> Option<&OutItem> {
        self.items.iter().find(|item| item.id == id)
    }

    /// Removes the item. A new message with the same key is edited from now on.
    pub fn mark_sent(&mut self, id: u64, message_id: i64, now: i64) {
        let Some(position) = self.items.iter().position(|item| item.id == id) else {
            return;
        };
        let item = self.items.remove(position);
        if let (Delivery::Send, Some(key)) = (item.delivery, item.key) {
            self.live.insert((item.chat_id, key), message_id);
        }
        self.record_attempt(now);
    }

    /// Counts the try and sets the next one. `retry_after` is Telegram's wait in seconds.
    pub fn mark_failed(&mut self, id: u64, now: i64, retry_after: Option<u32>) {
        let Some(item) = self.items.iter_mut().find(|item| item.id == id) else {
            return;
        };
        item.tries += 1;
        if item.tries > MAX_RETRIES {
            item.failed = true;
        } else {
            let wait = retry_after.map_or_else(|| backoff(item.tries), i64::from);
            item.next_attempt_at = now + wait;
        }
        self.record_attempt(now);
    }

    fn replace_queued_live(&mut self, draft: &Draft, hash: u64) -> Option<u64> {
        let key = draft.key.as_ref()?;
        let item = self.items.iter_mut().find(|item| {
            item.chat_id == draft.chat_id && item.key.as_ref() == Some(key) && !item.failed
        })?;
        item.text.clone_from(&draft.text);
        item.keyboard.clone_from(&draft.keyboard);
        item.priority = draft.priority;
        item.payload_hash = hash;
        Some(item.id)
    }

    fn enqueue(&mut self, draft: Draft, hash: u64, now: i64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let delivery = self.delivery_for(draft.chat_id, draft.key.as_deref());
        self.items.push(OutItem {
            id,
            chat_id: draft.chat_id,
            text: draft.text,
            keyboard: draft.keyboard,
            priority: draft.priority,
            key: draft.key,
            delivery,
            tries: 0,
            next_attempt_at: now,
            failed: false,
            payload_hash: hash,
        });
        id
    }

    fn delivery_for(&self, chat_id: i64, key: Option<&str>) -> Delivery {
        key.and_then(|key| self.live.get(&(chat_id, key.to_owned())))
            .map_or(Delivery::Send, |message_id| Delivery::Edit(*message_id))
    }

    fn record_attempt(&mut self, now: i64) {
        self.attempts.push_back(now);
        while self
            .attempts
            .front()
            .is_some_and(|at| now - *at >= WINDOW_SECONDS)
        {
            self.attempts.pop_front();
        }
    }

    fn sends_in_window(&self, now: i64) -> usize {
        self.attempts
            .iter()
            .filter(|at| now - **at < WINDOW_SECONDS)
            .count()
    }
}

/// `tries` is at least 1 and at most `MAX_RETRIES` here, so the index is in range.
fn backoff(tries: u32) -> i64 {
    let index = usize::try_from(tries - 1).unwrap_or_default();
    BACKOFF_SECONDS[index]
}

fn payload_hash(chat_id: i64, text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    chat_id.hash(&mut hasher);
    text.hash(&mut hasher);
    hasher.finish()
}

fn minute_of_day(now: i64, offset_minutes: i32) -> u16 {
    let Ok(utc) = OffsetDateTime::from_unix_timestamp(now) else {
        return 0;
    };
    let offset = UtcOffset::from_whole_seconds(offset_minutes * 60).unwrap_or(UtcOffset::UTC);
    let local = utc.to_offset(offset);
    u16::from(local.hour()) * 60 + u16::from(local.minute())
}
