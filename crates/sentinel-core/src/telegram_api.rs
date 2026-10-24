//! Telegram settings as the API and the UI see them. The bot token is never in these types.

use serde::{Deserialize, Serialize};

/// Quiet hours as minutes past midnight. The window may wrap past midnight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuietDto {
    pub start_minute: u16,
    pub end_minute: u16,
}

/// `GET /api/telegram`. Levels are named `notify`, `control`, `code`, and `reports`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramState {
    pub connected: bool,
    pub waiting_for_start: bool,
    pub bot_username: Option<String>,
    pub token_saved: bool,
    pub user_id: Option<i64>,
    pub levels: Vec<String>,
    pub events: Vec<String>,
    pub detail: String,
    pub quiet_hours: Option<QuietDto>,
}

/// `PUT /api/telegram`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsUpdate {
    pub levels: Vec<String>,
    pub events: Vec<String>,
    pub detail: String,
    pub quiet_hours: Option<QuietDto>,
}

/// `POST /api/telegram/token`. No `Debug`, so the token cannot reach a log by accident.
#[derive(Deserialize)]
pub struct TokenBody {
    pub token: String,
}

/// `POST /api/telegram/connect`. No `Debug`, so the token cannot reach a log by accident.
#[derive(Deserialize)]
pub struct ConnectBody {
    /// Optional. With no token, the saved one is checked again.
    pub token: Option<String>,
    pub user_id: i64,
}

/// The answer to `POST /api/telegram/token`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenSaved {
    pub bot_username: String,
}
