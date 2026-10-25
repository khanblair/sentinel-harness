//! Keys for the model providers: the provider table, the rows the UI shows, the live check of a
//! saved key, and the environment an agent CLI gets. A key never reaches a message or a log.

use std::time::Duration;

use reqwest::{RequestBuilder, StatusCode};
use sentinel_core::api::{KeyInfo, KeyTest};

use crate::engine::EngineError;
use crate::secrets::SecretStore;

/// The longest key the store accepts. Real keys are far shorter.
pub const MAX_KEY_BYTES: usize = 512;
/// The shortest key that gets a hint. A shorter key could be shown whole.
const HINT_MIN_CHARS: usize = 12;
const HINT_CHARS: usize = 4;
/// How long the live check of a saved key may take.
pub const LIVE_TIMEOUT: Duration = Duration::from_secs(10);
const ANTHROPIC_VERSION: &str = "2023-06-01";
const NO_KEY: &str = "No key saved yet.";
const REFUSED: &str = "The provider refused this key. Check it and save it again.";

/// How a provider wants its key in the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auth {
    /// `Authorization: Bearer KEY`.
    Bearer,
    /// `x-api-key: KEY`, with the API version header.
    AnthropicKey,
    /// `x-goog-api-key: KEY`. The key never goes in the URL.
    GoogleKey,
}

/// One model provider. `id` is also the secret-store name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Provider {
    pub id: &'static str,
    pub label: &'static str,
    /// The variable an agent CLI reads for its key.
    pub env_var: &'static str,
    /// The agent adapter that receives `env_var`. `OpenRouter` runs its own loop, so it has none.
    pub adapter: Option<&'static str>,
    pub base_url: &'static str,
    pub check_path: &'static str,
    pub auth: Auth,
}

pub const PROVIDERS: [Provider; 4] = [
    Provider {
        id: "openrouter",
        label: "OpenRouter",
        env_var: "OPENROUTER_API_KEY",
        adapter: None,
        base_url: "https://openrouter.ai/api/v1",
        check_path: "/key",
        auth: Auth::Bearer,
    },
    Provider {
        id: "anthropic",
        label: "Claude",
        env_var: "ANTHROPIC_API_KEY",
        adapter: Some("claude"),
        base_url: "https://api.anthropic.com/v1",
        check_path: "/models",
        auth: Auth::AnthropicKey,
    },
    Provider {
        id: "gemini",
        label: "Gemini",
        env_var: "GEMINI_API_KEY",
        adapter: Some("gemini"),
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        check_path: "/models",
        auth: Auth::GoogleKey,
    },
    Provider {
        id: "deepseek",
        label: "DeepSeek",
        env_var: "DEEPSEEK_API_KEY",
        adapter: Some("deepseek"),
        base_url: "https://api.deepseek.com",
        check_path: "/models",
        auth: Auth::Bearer,
    },
];

/// Where a saved key is checked. Tests point `base_url` at a mock server and shorten the timeout.
#[derive(Debug, Clone, Copy)]
pub struct KeyCheck<'a> {
    pub base_url: &'a str,
    pub timeout: Duration,
}

pub fn provider(id: &str) -> Option<&'static Provider> {
    PROVIDERS.iter().find(|provider| provider.id == id)
}

/// Trims the key. An empty key, a key over the cap, or one with a space or line break inside is
/// not a key.
pub fn clean_key(raw: &str) -> Option<String> {
    let key = raw.trim();
    let usable =
        !key.is_empty() && key.len() <= MAX_KEY_BYTES && !key.chars().any(char::is_whitespace);
    usable.then(|| key.to_owned())
}

/// The last four characters of a key, so the owner can tell keys apart. Short keys get none.
pub fn hint(key: &str) -> Option<String> {
    let count = key.chars().count();
    if count < HINT_MIN_CHARS {
        return None;
    }
    let tail: String = key.chars().skip(count - HINT_CHARS).collect();
    Some(format!("...{tail}"))
}

/// The row for one provider. A store that cannot be read counts as no key.
pub fn key_info(provider: &Provider, secrets: &dyn SecretStore) -> KeyInfo {
    let key = secrets.get(provider.id).ok().flatten();
    KeyInfo {
        provider: provider.id.to_owned(),
        label: provider.label.to_owned(),
        has_key: key.is_some(),
        hint: key.as_deref().and_then(hint),
    }
}

pub fn key_rows(secrets: &dyn SecretStore) -> Vec<KeyInfo> {
    PROVIDERS
        .iter()
        .map(|provider| key_info(provider, secrets))
        .collect()
}

/// The key variable an agent CLI needs, with the saved key as its value. Empty when the adapter
/// has no provider, no key is saved, or the variable is already set in the process.
pub fn agent_env(
    adapter: &str,
    secrets: &dyn SecretStore,
    process_env: &dyn Fn(&str) -> Option<String>,
) -> Vec<(String, String)> {
    let Some(provider) = PROVIDERS
        .iter()
        .find(|provider| provider.adapter == Some(adapter))
    else {
        return Vec::new();
    };
    if process_env(provider.env_var).is_some() {
        return Vec::new();
    }
    match secrets.get(provider.id) {
        Ok(Some(key)) if !key.is_empty() => vec![(provider.env_var.to_owned(), key)],
        _ => Vec::new(),
    }
}

/// Checks the saved key of one provider with a cheap read. A missing key is an error. Every
/// answer from the provider, good or bad, is a `KeyTest`.
pub async fn test_saved_key(
    provider: &Provider,
    secrets: &dyn SecretStore,
    check: KeyCheck<'_>,
) -> Result<KeyTest, EngineError> {
    let key = match secrets.get(provider.id) {
        Ok(Some(key)) => key,
        Ok(None) => return Err(EngineError::Invalid(NO_KEY.to_owned())),
        Err(_) => {
            return Ok(answer(
                provider,
                false,
                "The keychain could not read the key.",
            ));
        }
    };
    let (ok, message) = match send_check(provider, &key, check).await {
        Ok(status) => status_verdict(status),
        Err(error) => (false, transport_message(&error)),
    };
    Ok(answer(provider, ok, message))
}

fn answer(provider: &Provider, ok: bool, message: &str) -> KeyTest {
    KeyTest {
        provider: provider.id.to_owned(),
        ok,
        message: message.to_owned(),
    }
}

async fn send_check(
    provider: &Provider,
    key: &str,
    check: KeyCheck<'_>,
) -> Result<StatusCode, reqwest::Error> {
    let client = reqwest::Client::builder().timeout(check.timeout).build()?;
    let url = format!(
        "{}{}",
        check.base_url.trim_end_matches('/'),
        provider.check_path
    );
    let response = authorize(client.get(url), provider.auth, key)
        .send()
        .await?;
    Ok(response.status())
}

fn authorize(request: RequestBuilder, auth: Auth, key: &str) -> RequestBuilder {
    match auth {
        Auth::Bearer => request.bearer_auth(key),
        Auth::AnthropicKey => request
            .header("x-api-key", key)
            .header("anthropic-version", ANTHROPIC_VERSION),
        Auth::GoogleKey => request.header("x-goog-api-key", key),
    }
}

/// A 429 means the key is valid, so it counts as working.
fn status_verdict(status: StatusCode) -> (bool, &'static str) {
    if status.is_success() {
        return (true, "The key works.");
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        return (true, "The key is valid but the provider says slow down.");
    }
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return (false, REFUSED);
    }
    (
        false,
        "The provider answered with an error. Try again later.",
    )
}

/// Fixed sentences only. The error text of reqwest can carry the URL, so it is never shown.
fn transport_message(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "The provider did not answer in time."
    } else {
        "Could not reach the provider."
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemoryStore;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const KEY: &str = "sk-test-key-0123456789";

    fn by_id(id: &str) -> &'static Provider {
        provider(id).unwrap()
    }

    fn store_with(provider: &Provider, key: &str) -> MemoryStore {
        let store = MemoryStore::new();
        store.set(provider.id, key).unwrap();
        store
    }

    async fn answer_with(provider: &Provider, status: u16) -> KeyTest {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(provider.check_path))
            .respond_with(ResponseTemplate::new(status))
            .mount(&server)
            .await;
        let base = server.uri();
        let check = KeyCheck {
            base_url: &base,
            timeout: Duration::from_secs(5),
        };
        test_saved_key(provider, &store_with(provider, KEY), check)
            .await
            .unwrap()
    }

    #[test]
    fn clean_key_trims_and_refuses_empty_or_spaced_text() {
        assert_eq!(clean_key("  abc123  "), Some("abc123".to_owned()));
        assert_eq!(clean_key("   "), None);
        assert_eq!(clean_key("abc 123"), None);
        assert_eq!(clean_key("abc\n123"), None);
        assert_eq!(clean_key(&"k".repeat(MAX_KEY_BYTES + 1)), None);
    }

    #[test]
    fn hint_shows_the_last_four_characters_of_a_long_key() {
        assert_eq!(hint("sk-abcdefgh1234"), Some("...1234".to_owned()));
        assert_eq!(hint("short-key"), None);
    }

    #[test]
    fn key_rows_show_the_hint_and_never_more_of_the_key() {
        let store = store_with(by_id("gemini"), "gem-0123456789abcdef");
        let row = key_info(by_id("gemini"), &store);
        assert!(row.has_key);
        assert_eq!(row.hint.as_deref(), Some("...cdef"));
        assert!(!row.hint.unwrap().contains("0123456789ab"));
        let empty = key_info(by_id("deepseek"), &MemoryStore::new());
        assert!(!empty.has_key);
        assert_eq!(empty.hint, None);
    }

    #[test]
    fn agent_env_gives_each_adapter_its_own_variable() {
        let store = MemoryStore::new();
        store.set("anthropic", KEY).unwrap();
        store.set("gemini", "gem-key").unwrap();
        store.set("deepseek", "ds-key").unwrap();
        let none = |_: &str| None;
        assert_eq!(
            agent_env("claude", &store, &none),
            vec![("ANTHROPIC_API_KEY".to_owned(), KEY.to_owned())]
        );
        assert_eq!(
            agent_env("gemini", &store, &none),
            vec![("GEMINI_API_KEY".to_owned(), "gem-key".to_owned())]
        );
        assert_eq!(
            agent_env("deepseek", &store, &none),
            vec![("DEEPSEEK_API_KEY".to_owned(), "ds-key".to_owned())]
        );
        assert_eq!(agent_env("opencode", &store, &none), Vec::new());
    }

    #[test]
    fn agent_env_gives_nothing_when_the_variable_is_already_set() {
        let store = MemoryStore::new();
        store.set("anthropic", KEY).unwrap();
        let set = |name: &str| (name == "ANTHROPIC_API_KEY").then(|| "from-shell".to_owned());
        assert_eq!(agent_env("claude", &store, &set), Vec::new());
    }

    #[test]
    fn status_verdict_reads_success_rate_limit_and_refusal() {
        assert_eq!(status_verdict(StatusCode::OK), (true, "The key works."));
        assert!(status_verdict(StatusCode::TOO_MANY_REQUESTS).0);
        assert_eq!(status_verdict(StatusCode::UNAUTHORIZED), (false, REFUSED));
        assert_eq!(status_verdict(StatusCode::FORBIDDEN), (false, REFUSED));
    }

    #[tokio::test]
    async fn every_provider_says_the_key_works_on_200() {
        for provider in &PROVIDERS {
            let answer = answer_with(provider, 200).await;
            assert!(answer.ok, "{}", provider.id);
            assert_eq!(answer.message, "The key works.");
        }
    }

    #[tokio::test]
    async fn every_provider_refuses_a_401_without_showing_the_key() {
        for provider in &PROVIDERS {
            let answer = answer_with(provider, 401).await;
            assert!(!answer.ok, "{}", provider.id);
            assert_eq!(answer.message, REFUSED);
            assert!(!answer.message.contains(KEY));
        }
    }

    #[tokio::test]
    async fn a_429_counts_as_a_working_key() {
        let answer = answer_with(by_id("deepseek"), 429).await;
        assert!(answer.ok);
        assert_eq!(
            answer.message,
            "The key is valid but the provider says slow down."
        );
    }

    #[tokio::test]
    async fn a_silent_provider_times_out_with_a_plain_sentence() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/key"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(300)))
            .mount(&server)
            .await;
        let base = server.uri();
        let check = KeyCheck {
            base_url: &base,
            timeout: Duration::from_millis(50),
        };
        let openrouter = by_id("openrouter");
        let answer = test_saved_key(openrouter, &store_with(openrouter, KEY), check)
            .await
            .unwrap();
        assert!(!answer.ok);
        assert_eq!(answer.message, "The provider did not answer in time.");
    }

    #[tokio::test]
    async fn an_unreachable_provider_gets_a_plain_sentence() {
        let check = KeyCheck {
            base_url: "http://127.0.0.1:1",
            timeout: Duration::from_secs(5),
        };
        let openrouter = by_id("openrouter");
        let answer = test_saved_key(openrouter, &store_with(openrouter, KEY), check)
            .await
            .unwrap();
        assert!(!answer.ok);
        assert_eq!(answer.message, "Could not reach the provider.");
    }

    #[tokio::test]
    async fn a_missing_key_is_an_error_not_an_answer() {
        let check = KeyCheck {
            base_url: "http://127.0.0.1:1",
            timeout: Duration::from_secs(5),
        };
        let result = test_saved_key(by_id("anthropic"), &MemoryStore::new(), check).await;
        assert!(matches!(result, Err(EngineError::Invalid(text)) if text == NO_KEY));
    }

    #[tokio::test]
    async fn claude_sends_the_key_and_the_api_version_as_headers() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .and(header("x-api-key", KEY))
            .and(header("anthropic-version", ANTHROPIC_VERSION))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let base = server.uri();
        let check = KeyCheck {
            base_url: &base,
            timeout: Duration::from_secs(5),
        };
        let claude = by_id("anthropic");
        let answer = test_saved_key(claude, &store_with(claude, KEY), check)
            .await
            .unwrap();
        assert!(answer.ok);
    }

    #[tokio::test]
    async fn gemini_sends_the_key_in_a_header_and_never_in_the_url() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .and(header("x-goog-api-key", KEY))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let base = server.uri();
        let check = KeyCheck {
            base_url: &base,
            timeout: Duration::from_secs(5),
        };
        let gemini = by_id("gemini");
        let answer = test_saved_key(gemini, &store_with(gemini, KEY), check)
            .await
            .unwrap();
        assert!(answer.ok);
        let requests = server.received_requests().await.unwrap();
        assert!(!requests[0].url.as_str().contains(KEY));
    }
}
