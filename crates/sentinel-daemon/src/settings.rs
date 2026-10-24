//! Settings the computer owner changes: the budget before a run, the `OpenRouter` key, a test of one
//! connection, and the file list of the repository.

use std::process::Command;

use sentinel_core::Cost;

use crate::connections::ConnectionStatus;
use crate::controls::{Caller, usd_text};
use crate::engine::{Engine, EngineError, Result};

/// The longest key the store accepts. Real keys are far shorter.
const MAX_KEY_BYTES: usize = 512;
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

    /// Keep the `OpenRouter` key in the system keychain. The key is never read back to a client.
    pub fn set_openrouter_key(&self, key: &str) -> Result<()> {
        let key = key.trim();
        if key.is_empty() || key.len() > MAX_KEY_BYTES {
            return Err(EngineError::Invalid(
                "The key is empty or too long.".to_owned(),
            ));
        }
        self.inner
            .secrets
            .set("openrouter", key)
            .map_err(|_| EngineError::Refused("The keychain refused the key.".to_owned()))
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
