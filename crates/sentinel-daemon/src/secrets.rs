//! Secrets for the daemon. Values live in the OS keychain, never on disk and never in logs.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use keyring::{Entry, Error as KeyringError};

const SERVICE: &str = "sentinel";
const OPENROUTER_VAR: &str = "OPENROUTER_API_KEY";
const TELEGRAM_VAR: &str = "SENTINEL_TELEGRAM_TOKEN";

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("the secret store is unavailable")]
    Unavailable,
    #[error("the secret store failed: {0}")]
    Failed(String),
}

pub type Result<T> = std::result::Result<T, SecretError>;

pub trait SecretStore: Send + Sync {
    fn get(&self, name: &str) -> Result<Option<String>>;
    fn set(&self, name: &str, value: &str) -> Result<()>;
    fn delete(&self, name: &str) -> Result<()>;
}

/// The operating system keychain, under the service name `sentinel`.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeychainStore;

impl SecretStore for KeychainStore {
    fn get(&self, name: &str) -> Result<Option<String>> {
        let entry = match Entry::new(SERVICE, name) {
            Ok(entry) => entry,
            Err(error) => return Err(keychain_error(&error)),
        };
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            // A missing entry, a locked store, or a missing backend all read as "no value"
            // so the env fallback can answer.
            Err(_) => Ok(None),
        }
    }

    fn set(&self, name: &str, value: &str) -> Result<()> {
        let entry = Entry::new(SERVICE, name).map_err(|error| keychain_error(&error))?;
        entry
            .set_password(value)
            .map_err(|error| keychain_error(&error))
    }

    fn delete(&self, name: &str) -> Result<()> {
        let entry = Entry::new(SERVICE, name).map_err(|error| keychain_error(&error))?;
        match entry.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(keychain_error(&error)),
        }
    }
}

fn keychain_error(error: &KeyringError) -> SecretError {
    match error {
        KeyringError::PlatformFailure(_)
        | KeyringError::NoStorageAccess(_)
        | KeyringError::NoDefaultStore => SecretError::Unavailable,
        KeyringError::NoEntry
        | KeyringError::BadEncoding(_)
        | KeyringError::BadDataFormat(_, _)
        | KeyringError::BadStoreFormat(_)
        | KeyringError::TooLong(_, _)
        | KeyringError::Invalid(_, _)
        | KeyringError::Ambiguous(_)
        | KeyringError::NotSupportedByStore(_) => SecretError::Failed(error.to_string()),
        _ => SecretError::Failed("unknown keychain error".to_owned()),
    }
}

/// A store for tests.
#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    values: Arc<Mutex<BTreeMap<String, String>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemoryStore {
    fn get(&self, name: &str) -> Result<Option<String>> {
        let values = self
            .values
            .lock()
            .map_err(|_| SecretError::Failed("lock".to_owned()))?;
        Ok(values.get(name).cloned())
    }

    fn set(&self, name: &str, value: &str) -> Result<()> {
        let mut values = self
            .values
            .lock()
            .map_err(|_| SecretError::Failed("lock".to_owned()))?;
        values.insert(name.to_owned(), value.to_owned());
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<()> {
        let mut values = self
            .values
            .lock()
            .map_err(|_| SecretError::Failed("lock".to_owned()))?;
        values.remove(name);
        Ok(())
    }
}

type EnvLookup = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

enum EnvSource {
    Process,
    Named { name: String, lookup: EnvLookup },
}

/// Reads from the store first, then from the documented environment variable. Writes only
/// reach the store.
pub struct EnvFallback<S: SecretStore> {
    pub store: S,
    env: EnvSource,
}

impl<S: SecretStore> EnvFallback<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            env: EnvSource::Process,
        }
    }

    /// A fixed variable name plus an injected lookup. Tests use this to stay off the real
    /// process environment, which needs `unsafe` to write in edition 2024.
    pub fn with_env_name<F>(store: S, name: impl Into<String>, lookup: F) -> Self
    where
        F: Fn(&str) -> Option<String> + Send + Sync + 'static,
    {
        Self {
            store,
            env: EnvSource::Named {
                name: name.into(),
                lookup: Arc::new(lookup),
            },
        }
    }

    fn env_value(&self, name: &str) -> Option<String> {
        match &self.env {
            EnvSource::Process => env_name(name).and_then(|variable| std::env::var(variable).ok()),
            EnvSource::Named { name, lookup } => lookup(name),
        }
    }
}

impl<S: SecretStore> From<S> for EnvFallback<S> {
    fn from(store: S) -> Self {
        Self::new(store)
    }
}

impl<S: SecretStore> SecretStore for EnvFallback<S> {
    fn get(&self, name: &str) -> Result<Option<String>> {
        match self.store.get(name) {
            Ok(Some(value)) => Ok(Some(value)),
            Ok(None) | Err(_) => Ok(self.env_value(name)),
        }
    }

    fn set(&self, name: &str, value: &str) -> Result<()> {
        self.store.set(name, value)
    }

    fn delete(&self, name: &str) -> Result<()> {
        self.store.delete(name)
    }
}

/// The environment variable that may hold a key when the keychain is empty.
pub fn env_name(name: &str) -> Option<&'static str> {
    match name {
        "openrouter" => Some(OPENROUTER_VAR),
        "telegram" => Some(TELEGRAM_VAR),
        _ => None,
    }
}

/// Replace key-shaped tokens with `[hidden]`.
pub fn redact(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < bytes.len() {
        if let Some(end) = secret_end(bytes, index) {
            out.push_str("[hidden]");
            index = end;
            continue;
        }
        let Some(character) = text[index..].chars().next() else {
            break;
        };
        out.push(character);
        index += character.len_utf8();
    }
    out
}

fn secret_end(bytes: &[u8], start: usize) -> Option<usize> {
    if let Some(end) = prefix_run(bytes, start, b"sk-", 20) {
        return Some(end);
    }
    if let Some(end) = prefix_run(bytes, start, b"ghp_", 20) {
        return Some(end);
    }
    if let Some(end) = prefix_run(bytes, start, b"gho_", 20) {
        return Some(end);
    }
    if let Some(end) = telegram_end(bytes, start) {
        return Some(end);
    }
    long_hex_end(bytes, start)
}

fn prefix_run(bytes: &[u8], start: usize, prefix: &[u8], tail: usize) -> Option<usize> {
    if !bytes[start..].starts_with(prefix) {
        return None;
    }
    let end = token_end(bytes, start + prefix.len());
    (end - (start + prefix.len()) >= tail).then_some(end)
}

fn telegram_end(bytes: &[u8], start: usize) -> Option<usize> {
    let digits_end = digit_end(bytes, start);
    if digits_end == start || bytes.get(digits_end) != Some(&b':') {
        return None;
    }
    let token_start = digits_end + 1;
    let end = token_end(bytes, token_start);
    (end - token_start >= 35).then_some(end)
}

fn long_hex_end(bytes: &[u8], start: usize) -> Option<usize> {
    let end = hex_end(bytes, start);
    let at_run_start = start == 0 || !bytes[start - 1].is_ascii_hexdigit();
    (end - start >= 40 && at_run_start).then_some(end)
}

fn token_end(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|byte| !is_token_byte(*byte))
        .map_or(bytes.len(), |offset| start + offset)
}

fn digit_end(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .map_or(bytes.len(), |offset| start + offset)
}

fn hex_end(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|byte| !byte.is_ascii_hexdigit())
        .map_or(bytes.len(), |offset| start + offset)
}

fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips_and_deletes() {
        let store = MemoryStore::new();
        assert_eq!(store.get("key").unwrap(), None);
        store.set("key", "value").unwrap();
        assert_eq!(store.get("key").unwrap(), Some("value".to_owned()));
        store.delete("key").unwrap();
        assert_eq!(store.get("key").unwrap(), None);
    }

    #[test]
    fn env_fallback_reads_the_variable_when_the_store_is_empty() {
        let fallback = EnvFallback::with_env_name(MemoryStore::new(), "TEST_OPENROUTER", |name| {
            (name == "TEST_OPENROUTER").then(|| "env-key".to_owned())
        });
        assert_eq!(
            fallback.get("openrouter").unwrap(),
            Some("env-key".to_owned())
        );
    }

    #[test]
    fn env_fallback_prefers_the_store_over_the_variable() {
        let store = MemoryStore::new();
        store.set("openrouter", "store-key").unwrap();
        let fallback =
            EnvFallback::with_env_name(store, "TEST_OPENROUTER", |_| Some("env-key".to_owned()));
        assert_eq!(
            fallback.get("openrouter").unwrap(),
            Some("store-key".to_owned())
        );
    }

    #[test]
    fn env_fallback_writes_only_to_the_store() {
        let fallback = EnvFallback::with_env_name(MemoryStore::new(), "TEST_TELEGRAM", |_| {
            Some("env-value".to_owned())
        });
        fallback.set("telegram", "new-value").unwrap();
        assert_eq!(
            fallback.store.get("telegram").unwrap(),
            Some("new-value".to_owned())
        );
    }

    #[test]
    fn env_fallback_uses_the_variable_when_the_store_fails() {
        struct BrokenStore;
        impl SecretStore for BrokenStore {
            fn get(&self, _name: &str) -> Result<Option<String>> {
                Err(SecretError::Unavailable)
            }
            fn set(&self, _name: &str, _value: &str) -> Result<()> {
                Err(SecretError::Unavailable)
            }
            fn delete(&self, _name: &str) -> Result<()> {
                Err(SecretError::Unavailable)
            }
        }
        let fallback = EnvFallback::with_env_name(BrokenStore, "TEST_OPENROUTER", |_| {
            Some("env-key".to_owned())
        });
        assert_eq!(
            fallback.get("openrouter").unwrap(),
            Some("env-key".to_owned())
        );
    }

    #[test]
    fn redact_hides_each_key_shape() {
        let sk = format!("sk-{}", "a".repeat(24));
        let github = format!("ghp_{}", "b".repeat(24));
        let telegram = format!("123456789:{}", "c".repeat(35));
        let hex = "d".repeat(40);
        assert_eq!(redact(&sk), "[hidden]");
        assert_eq!(redact(&github), "[hidden]");
        assert_eq!(redact(&telegram), "[hidden]");
        assert_eq!(redact(&hex), "[hidden]");
    }

    #[test]
    fn redact_keeps_normal_text_and_short_tokens() {
        assert_eq!(redact("hello world"), "hello world");
        assert_eq!(redact("sk-short"), "sk-short");
        assert_eq!(redact("abcdef0123"), "abcdef0123");
    }

    #[test]
    fn redact_keeps_the_text_around_a_secret() {
        let text = format!("token sk-{} is set", "e".repeat(20));
        assert_eq!(redact(&text), "token [hidden] is set");
    }

    #[test]
    fn redact_hides_the_documented_telegram_shape() {
        let token = format!("123456789:{}", "A".repeat(35));
        assert_eq!(redact(&format!("bot {token}")), "bot [hidden]");
    }
}
