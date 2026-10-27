//! The `OpenRouter` worker offered as a tool: its entry in the tool list and the worker built for
//! a run. The key is read from the secrets each time and never leaves this module.

use std::sync::Arc;

use sentinel_core::{Tier, ToolId, ToolInfo, ToolStatus};
use sentinel_workers::{OpenRouterConfig, OpenRouterWorker, Worker};

use crate::config::Config;
use crate::engine::{EngineError, Result};
use crate::secrets::SecretStore;

const TOOL_ID: &str = "openrouter";
const ADAPTER: &str = "openrouter-loop";
const DEFAULT_MODEL: &str = "openrouter/auto";
const MISSING_KEY: &str = "Save an OpenRouter key in Settings, Connections.";

/// True when this adapter name runs the `OpenRouter` loop rather than a CLI profile.
pub(crate) fn is_openrouter_adapter(adapter: &str) -> bool {
    adapter == ADAPTER
}

/// The tool entry for the list. It is ready once a key is saved, and it is never enabled by itself.
pub(crate) fn openrouter_entry(secrets: &dyn SecretStore) -> ToolInfo {
    let status = if saved_key(secrets).is_some() {
        ToolStatus::Ready
    } else {
        ToolStatus::NeedsLogin
    };
    ToolInfo {
        id: ToolId::new(TOOL_ID),
        name: "OpenRouter".to_owned(),
        path: String::new(),
        version: "built in".to_owned(),
        adapter: ADAPTER.to_owned(),
        status,
        tier: Tier::Cheap,
        enabled: false,
    }
}

/// A worker for one run. The model is the entry of this tool id in `models`, or the router when none
/// is set. A roster may run two copies of this tool, each with its own model.
pub(crate) fn openrouter_worker(
    secrets: &dyn SecretStore,
    config: &Config,
    tool: &ToolId,
) -> Result<Arc<dyn Worker>> {
    let api_key = saved_key(secrets).ok_or_else(|| EngineError::Invalid(MISSING_KEY.to_owned()))?;
    let model = config
        .models
        .get(tool.as_str())
        .filter(|model| !model.is_empty())
        .cloned()
        .unwrap_or_else(|| DEFAULT_MODEL.to_owned());
    let settings = OpenRouterConfig {
        base_url: config.openrouter_base_url.clone(),
        api_key,
        model,
        ..OpenRouterConfig::default()
    };
    let worker =
        OpenRouterWorker::new(settings).map_err(|error| EngineError::Invalid(error.to_string()))?;
    Ok(Arc::new(worker))
}

/// The saved key. A keychain error reads as no key, the same as a missing entry.
fn saved_key(secrets: &dyn SecretStore) -> Option<String> {
    match secrets.get(TOOL_ID) {
        Ok(Some(key)) if !key.trim().is_empty() => Some(key),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemoryStore;

    #[test]
    fn the_entry_needs_login_until_a_key_is_saved() {
        let store = MemoryStore::new();
        assert_eq!(openrouter_entry(&store).status, ToolStatus::NeedsLogin);
        store.set(TOOL_ID, "sk-or-unit-test").unwrap();
        let entry = openrouter_entry(&store);
        assert_eq!(entry.status, ToolStatus::Ready);
        assert!(!entry.enabled);
        assert_eq!(entry.path, "");
        assert_eq!(entry.adapter, ADAPTER);
    }

    #[test]
    fn a_blank_key_does_not_count_as_saved() {
        let store = MemoryStore::new();
        store.set(TOOL_ID, "   ").unwrap();
        assert_eq!(openrouter_entry(&store).status, ToolStatus::NeedsLogin);
    }

    #[test]
    fn the_entry_never_carries_the_key_text() {
        let store = MemoryStore::new();
        store.set(TOOL_ID, "sk-or-unit-test").unwrap();
        let text = format!("{:?}", openrouter_entry(&store));
        assert!(!text.contains("sk-or-unit-test"), "{text}");
    }

    #[test]
    fn a_worker_needs_a_saved_key() {
        let store = MemoryStore::new();
        let tool = ToolId::new(TOOL_ID);
        let error = openrouter_worker(&store, &Config::default(), &tool).err();
        assert!(matches!(error, Some(EngineError::Invalid(text)) if text == MISSING_KEY));
        store.set(TOOL_ID, "sk-or-unit-test").unwrap();
        assert!(openrouter_worker(&store, &Config::default(), &tool).is_ok());
    }

    #[test]
    fn a_second_copy_of_the_tool_builds_its_own_worker() {
        let store = MemoryStore::new();
        store.set(TOOL_ID, "sk-or-unit-test").unwrap();
        let copy = ToolId::new("openrouter~2");
        assert!(openrouter_worker(&store, &Config::default(), &copy).is_ok());
    }

    #[test]
    fn only_the_openrouter_adapter_name_selects_the_loop() {
        assert!(is_openrouter_adapter("openrouter-loop"));
        assert!(!is_openrouter_adapter("claude"));
    }
}
