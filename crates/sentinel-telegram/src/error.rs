//! Errors of the Telegram client. No variant carries the bot token.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, TelegramError>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TelegramError {
    #[error("Telegram request failed: {0}")]
    Transport(String),
    #[error("Telegram refused the request: {0}")]
    Api(String),
    #[error("Telegram asks to wait {retry_after_seconds} seconds")]
    RateLimited { retry_after_seconds: u32 },
    #[error("Telegram sent a reply that does not fit the API: {0}")]
    Decode(String),
}
