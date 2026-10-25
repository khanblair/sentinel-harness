//! Settings the computer owner changes: the budget before a run, the `OpenRouter` key, a test of one
//! connection, and the file list of the repository.

use std::process::Command;

use sentinel_core::Cost;

use sentinel_core::api::{KeyInfo, KeyTest};

use crate::connections::ConnectionStatus;
use crate::controls::{Caller, usd_text};
use crate::engine::{Engine, EngineError, Result};
use crate::keys::{self, KeyCheck, LIVE_TIMEOUT};

/// The longest file list sent to the UI.
const MAX_FILES: usize = 5000;

impl Engine {
    /// Set the budget cap before a run starts, for example in setup. Once a run is live the cap can
    /// only be lowered (`lower_cap`).
    pub fn set_cap(&self, text: &str, caller: &Caller) -> Result<Cost> {
        if self.is_run_live() {
            return Err(EngineError::Refused(
                "A run is live. During a run the cap can only be lowered.".to_owned(),
            ));
        }
        let cap = Cost::parse_usd(text).map_err(|e| EngineError::Invalid(e.to_string()))?;
        if cap == Cost::ZERO {
            return Err(EngineError::Invalid(
                "The cap must be more than zero.".to_owned(),
            ));
        }
        self.update_config(|c| c.cap_usd = usd_text(cap))?;
        self.emit(&sentinel_core::Event::DeviceAction {
            device: sentinel_core::DeviceId::new(&caller.name),
            action: format!("set the cap to {cap}"),
        })?;
        Ok(cap)
    }

    /// Keep the `OpenRouter` key in the system keychain.
    pub fn set_openrouter_key(&self, key: &str) -> Result<()> {
        self.save_key("openrouter", key)
    }

    /// The row of every model provider. A key is never read back to a client.
    pub fn key_rows(&self) -> Vec<KeyInfo> {
        keys::key_rows(self.inner.secrets.as_ref())
    }

    /// Keep one provider's key in the system keychain.
    pub fn save_key(&self, provider: &str, text: &str) -> Result<()> {
        let provider = find_provider(provider)?;
        let key = keys::clean_key(text)
            .ok_or_else(|| EngineError::Invalid("That does not look like a key.".to_owned()))?;
        self.inner
            .secrets
            .set(provider.id, &key)
            .map_err(|_| EngineError::Refused("The keychain refused the key.".to_owned()))
    }

    /// Remove one provider's key from the system keychain.
    pub fn delete_key(&self, provider: &str) -> Result<()> {
        let provider = find_provider(provider)?;
        self.inner
            .secrets
            .delete(provider.id)
            .map_err(|_| EngineError::Refused("The keychain refused the change.".to_owned()))
    }

    /// Check the saved key of one provider against the provider itself.
    pub async fn test_key(&self, provider: &str) -> Result<KeyTest> {
        let provider = find_provider(provider)?;
        let check = KeyCheck {
            base_url: provider.base_url,
            timeout: LIVE_TIMEOUT,
        };
        keys::test_saved_key(provider, self.inner.secrets.as_ref(), check).await
    }

    /// Check one connection again and return its fresh state.
    pub async fn test_connection(&self, name: &str) -> Result<ConnectionStatus> {
        self.connections()
            .await
            .into_iter()
            .find(|row| row.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| EngineError::NotFound(format!("connection {name}")))
    }

    /// The files git knows in the repository: tracked, plus new files that are not ignored.
    pub fn repo_files(&self) -> Result<Vec<String>> {
        let output = Command::new("git")
            .current_dir(self.repo_root())
            .args([
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
            ])
            .output()?;
        if !output.status.success() {
            return Err(EngineError::Invalid(
                "git could not list the files of this folder.".to_owned(),
            ));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut files: Vec<String> = text
            .split('\0')
            .filter(|path| !path.is_empty() && !path.starts_with(".sentinel/"))
            .map(str::to_owned)
            .collect();
        files.sort();
        files.dedup();
        files.truncate(MAX_FILES);
        Ok(files)
    }
}

fn find_provider(id: &str) -> Result<&'static keys::Provider> {
    keys::provider(id).ok_or_else(|| EngineError::NotFound(format!("provider {id}")))
}
