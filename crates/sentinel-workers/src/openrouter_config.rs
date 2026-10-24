//! Settings and errors of the `OpenRouter` worker.

use std::fmt;
use std::time::Duration;

use thiserror::Error;

const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";
const DEFAULT_WINDOW_TOKENS: u32 = 32_000;
const DEFAULT_MAX_STEPS: u32 = 30;
const DEFAULT_RETRY_BASE: Duration = Duration::from_secs(1);
/// What an API key shows as in any printed text.
pub(crate) const HIDDEN: &str = "[hidden]";

/// Settings for one `OpenRouter` worker. The API key is never printed.
#[derive(Clone)]
pub struct OpenRouterConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub window_tokens: u32,
    pub max_steps: u32,
    /// The first retry wait. Each later wait doubles it. Tests set it to zero.
    pub retry_base: Duration,
}

impl Default for OpenRouterConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            api_key: String::new(),
            model: String::new(),
            window_tokens: DEFAULT_WINDOW_TOKENS,
            max_steps: DEFAULT_MAX_STEPS,
            retry_base: DEFAULT_RETRY_BASE,
        }
    }
}

impl fmt::Debug for OpenRouterConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenRouterConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &HIDDEN)
            .field("model", &self.model)
            .field("window_tokens", &self.window_tokens)
            .field("max_steps", &self.max_steps)
            .field("retry_base", &self.retry_base)
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum OpenRouterError {
    #[error("cannot build the HTTP client: {0}")]
    Client(#[from] reqwest::Error),
}

/// Replace the API key in text. An empty key replaces nothing.
pub(crate) fn redact(text: &str, key: &str) -> String {
    if key.is_empty() {
        text.to_string()
    } else {
        text.replace(key, HIDDEN)
    }
}
