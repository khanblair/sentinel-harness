//! Bot API calls over HTTPS. Each method is one request and returns one typed result.

use std::fmt;
use std::time::Duration;

use serde::Serialize;
use serde::de::{DeserializeOwned, IgnoredAny};
use serde_json::{Value, json};

use crate::api::{BotInfo, Button, Envelope, MessageRef, Update, WireMe, WireSent, WireUpdate};
use crate::error::{Result, TelegramError};

/// Time a plain call may take before the request is abandoned.
const PLAIN_CALL_WAIT: Duration = Duration::from_secs(30);
/// Extra time on top of the long-poll wait, so the HTTP request outlives Telegram's reply.
const REQUEST_SLACK: Duration = Duration::from_secs(10);
const HIDDEN: &str = "[hidden]";

pub struct BotClient {
    base_url: String,
    token: String,
    http: reqwest::Client,
}

impl fmt::Debug for BotClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BotClient").field("token", &HIDDEN).finish()
    }
}

impl BotClient {
    /// `base_url` is `https://api.telegram.org` in production. Tests pass a local mock server.
    pub fn new(base_url: &str, token: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            token: token.to_owned(),
            http: reqwest::Client::new(),
        }
    }

    pub async fn get_me(&self) -> Result<BotInfo> {
        let me: WireMe = self.call("getMe", json!({}), PLAIN_CALL_WAIT).await?;
        Ok(BotInfo {
            id: me.id,
            username: me.username,
        })
    }

    /// Long polling. The call waits up to `timeout_seconds` for new updates.
    pub async fn get_updates(&self, offset: i64, timeout_seconds: u32) -> Result<Vec<Update>> {
        let body = json!({
            "offset": offset,
            "timeout": timeout_seconds,
            "allowed_updates": ["message", "callback_query"],
        });
        let wait = Duration::from_secs(u64::from(timeout_seconds));
        let updates: Vec<WireUpdate> = self.call("getUpdates", body, wait).await?;
        Ok(updates.into_iter().map(WireUpdate::into_update).collect())
    }

    /// Returns the id of the message that was sent.
    pub async fn send_message(
        &self,
        chat_id: i64,
        text: &str,
        keyboard: Option<&[Vec<Button>]>,
    ) -> Result<i64> {
        let mut body = json!({ "chat_id": chat_id, "text": text });
        add_keyboard(&mut body, keyboard);
        let sent: WireSent = self.call("sendMessage", body, PLAIN_CALL_WAIT).await?;
        Ok(sent.message_id)
    }

    /// Replaces the text and keyboard of a message the bot sent before.
    pub async fn edit_message(
        &self,
        target: MessageRef,
        text: &str,
        keyboard: Option<&[Vec<Button>]>,
    ) -> Result<()> {
        let mut body = json!({
            "chat_id": target.chat_id,
            "message_id": target.message_id,
            "text": text,
        });
        add_keyboard(&mut body, keyboard);
        let _: IgnoredAny = self.call("editMessageText", body, PLAIN_CALL_WAIT).await?;
        Ok(())
    }

    /// Stops the spinner on a button press. `text` may be empty.
    pub async fn answer_callback(&self, callback_id: &str, text: &str) -> Result<()> {
        let body = json!({ "callback_query_id": callback_id, "text": text });
        let _: IgnoredAny = self
            .call("answerCallbackQuery", body, PLAIN_CALL_WAIT)
            .await?;
        Ok(())
    }

    async fn call<T, B>(&self, method: &str, body: B, wait: Duration) -> Result<T>
    where
        T: DeserializeOwned,
        B: Serialize,
    {
        let url = format!("{}/bot{}/{method}", self.base_url, self.token);
        let response = self
            .http
            .post(url)
            .json(&body)
            .timeout(wait + REQUEST_SLACK)
            .send()
            .await
            .map_err(transport_error)?;
        // Telegram sends its error reply as JSON with the status code, so the body is read first.
        let envelope: Envelope<T> = response.json().await.map_err(transport_error)?;
        envelope.into_result(&self.token)
    }
}

impl<T> Envelope<T> {
    fn into_result(self, token: &str) -> Result<T> {
        if self.ok {
            return self
                .result
                .ok_or_else(|| TelegramError::Decode("no result in an ok reply".to_owned()));
        }
        if let Some(seconds) = self.parameters.and_then(|p| p.retry_after) {
            return Err(TelegramError::RateLimited {
                retry_after_seconds: seconds,
            });
        }
        let description = self
            .description
            .unwrap_or_else(|| "no reason given".to_owned());
        Err(TelegramError::Api(hide_token(&description, token)))
    }
}

/// `reqwest` puts the request URL in its messages, and the URL holds the token. Drop it.
fn transport_error(error: reqwest::Error) -> TelegramError {
    TelegramError::Transport(error.without_url().to_string())
}

fn hide_token(text: &str, token: &str) -> String {
    if token.is_empty() {
        return text.to_owned();
    }
    text.replace(token, HIDDEN)
}

fn add_keyboard(body: &mut Value, keyboard: Option<&[Vec<Button>]>) {
    let Some(rows) = keyboard else {
        return;
    };
    let inline: Vec<Vec<Value>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|button| json!({ "text": button.label, "callback_data": button.data }))
                .collect()
        })
        .collect();
    body["reply_markup"] = json!({ "inline_keyboard": inline });
}
