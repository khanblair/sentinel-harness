//! Bot API types. The wire shapes stay private here; the rest of the crate sees the plain types.

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotInfo {
    pub id: i64,
    pub username: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub update_id: i64,
    pub message: Option<Message>,
    pub callback_query: Option<CallbackQuery>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub message_id: i64,
    pub from_id: i64,
    pub chat_id: i64,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackQuery {
    pub id: String,
    pub from_id: i64,
    pub chat_id: i64,
    pub message_id: i64,
    pub data: Option<String>,
}

/// A message the bot already sent, named by its chat and its id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageRef {
    pub chat_id: i64,
    pub message_id: i64,
}

/// One inline keyboard button. `data` is the callback data, at most 64 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Button {
    pub label: String,
    pub data: String,
}

#[derive(Deserialize)]
pub(crate) struct Envelope<T> {
    pub(crate) ok: bool,
    pub(crate) result: Option<T>,
    pub(crate) description: Option<String>,
    pub(crate) parameters: Option<ResponseParameters>,
}

#[derive(Deserialize)]
pub(crate) struct ResponseParameters {
    pub(crate) retry_after: Option<u32>,
}

#[derive(Deserialize)]
pub(crate) struct WireUser {
    pub(crate) id: i64,
}

#[derive(Deserialize)]
pub(crate) struct WireChat {
    pub(crate) id: i64,
}

#[derive(Deserialize)]
pub(crate) struct WireMessage {
    pub(crate) message_id: i64,
    pub(crate) from: Option<WireUser>,
    pub(crate) chat: WireChat,
    pub(crate) text: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct WireCallback {
    pub(crate) id: String,
    pub(crate) from: WireUser,
    pub(crate) message: Option<WireMessage>,
    pub(crate) data: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct WireUpdate {
    pub(crate) update_id: i64,
    pub(crate) message: Option<WireMessage>,
    pub(crate) callback_query: Option<WireCallback>,
}

#[derive(Deserialize)]
pub(crate) struct WireMe {
    pub(crate) id: i64,
    pub(crate) username: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct WireSent {
    pub(crate) message_id: i64,
}

impl WireUpdate {
    pub(crate) fn into_update(self) -> Update {
        Update {
            update_id: self.update_id,
            message: self.message.and_then(WireMessage::into_message),
            callback_query: self.callback_query.and_then(WireCallback::into_query),
        }
    }
}

impl WireMessage {
    /// Channel posts and service messages have no sender. They are dropped.
    fn into_message(self) -> Option<Message> {
        let from = self.from?;
        Some(Message {
            message_id: self.message_id,
            from_id: from.id,
            chat_id: self.chat.id,
            text: self.text,
        })
    }
}

impl WireCallback {
    /// A callback without its message has no chat to answer in, so it is dropped.
    fn into_query(self) -> Option<CallbackQuery> {
        let message = self.message?;
        Some(CallbackQuery {
            id: self.id,
            from_id: self.from.id,
            chat_id: message.chat.id,
            message_id: message.message_id,
            data: self.data,
        })
    }
}
