//! Clear the data of one scope, so the app can start again from the setup wizard. Every call
//! checks its scopes and any live run before it deletes a row, so a refused call changes nothing.

use std::fs;
use std::io;

use sentinel_core::api::DataScope;
use sentinel_core::{DeviceKind, HarnessState};

use crate::config::{self, Config, TelegramConfig};
use crate::controls::Caller;
use crate::engine::{Engine, EngineError, Result, lock};
use crate::telegram::{self, TOKEN_KEY};
use crate::telegram_store;

/// The keychain names Sentinel writes. `openrouter` is the key the Connections page saves.
const KEY_NAMES: [&str; 2] = ["openrouter", TOKEN_KEY];
/// The settings a run writes while it is live. Clearing history removes them.
const RUN_MARKS: [&str; 2] = ["run.live", "run.chat"];
/// Set by the first tool scan. Removing it lets the next scan turn the ready agents on again.
const TOOL_DEFAULTS_KEY: &str = "tools.defaults_applied";
/// The Brain index file and its SQLite side files.
const BRAIN_INDEX_FILES: [&str; 4] = [
    "brain.db",
    "brain.db-wal",
    "brain.db-shm",
    "brain.db-journal",
];

/// Refuses a change while a run is live in any of the engines. The hub checks every project
/// before it clears one of them.
pub fn ensure_no_live_run(engines: &[Engine]) -> Result<()> {
    if engines.iter().any(Engine::is_run_live) {
        return Err(stop_the_run());
    }
    Ok(())
}

fn stop_the_run() -> EngineError {
    EngineError::Refused("Stop the run first.".to_owned())
}

/// Each scope must belong to the project, and no scope that rewrites run data may run while a
/// run is live.
fn check_project_scopes(scopes: &[DataScope], live: bool) -> Result<()> {
    if scopes.iter().any(|scope| !scope.is_project()) {
        return Err(EngineError::Invalid(
            "That data is not in this project.".to_owned(),
        ));
    }
    let touches_run = scopes
        .iter()
        .any(|scope| matches!(scope, DataScope::Chats | DataScope::History));
    if live && touches_run {
        return Err(stop_the_run());
    }
    Ok(())
}

/// Each scope must belong to the app, and the project list is left to the hub.
fn check_app_scopes(scopes: &[DataScope]) -> Result<()> {
    if scopes.iter().any(|scope| scope.is_project()) {
        return Err(EngineError::Invalid(
            "That data belongs to one project. Clear it from the project's settings.".to_owned(),
        ));
    }
    if scopes.contains(&DataScope::Projects) {
        return Err(EngineError::Invalid(
            "The project list is cleared by the hub.".to_owned(),
        ));
    }
    Ok(())
}

/// A file or folder that is already gone is not an error.
fn ignore_missing(result: io::Result<()>) -> Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(EngineError::Io(error)),
    }
}

impl Engine {
    /// Clear the data of the project scopes. Chats and history stop when a run is live.
    pub fn clear_project_data(&self, scopes: &[DataScope]) -> Result<()> {
        check_project_scopes(scopes, self.is_run_live())?;
        for scope in scopes {
            self.clear_project_scope(*scope)?;
        }
        Ok(())
    }

    /// Clear the data of the app scopes. The primary project holds them.
    pub fn clear_app_data(&self, scopes: &[DataScope]) -> Result<()> {
        check_app_scopes(scopes)?;
        for scope in scopes {
            self.clear_app_scope(*scope)?;
        }
        Ok(())
    }

    fn clear_project_scope(&self, scope: DataScope) -> Result<()> {
        match scope {
            DataScope::Chats => self.clear_chats(),
            DataScope::Brain => self.clear_brain(),
            DataScope::History => self.clear_history(),
            DataScope::Settings => self.clear_settings(),
            DataScope::Devices
            | DataScope::Keys
            | DataScope::Telegram
            | DataScope::Routines
            | DataScope::Projects
            | DataScope::Onboarding => Err(EngineError::Invalid(
                "That data is not in this project.".to_owned(),
            )),
        }
    }

    fn clear_app_scope(&self, scope: DataScope) -> Result<()> {
        match scope {
            DataScope::Devices => self.clear_devices(),
            DataScope::Keys => self.clear_keys(),
            DataScope::Telegram => self.clear_telegram_data(),
            DataScope::Routines => self.store().clear_routines().map_err(EngineError::from),
            DataScope::Onboarding => self.onboarding_reset().map(|_| ()),
            DataScope::Chats
            | DataScope::Brain
            | DataScope::History
            | DataScope::Settings
            | DataScope::Projects => Err(EngineError::Invalid(
                "That data is not in the app.".to_owned(),
            )),
        }
    }

    /// Every chat goes, with its events and uploads. `delete_chat` keeps one empty chat in place.
    fn clear_chats(&self) -> Result<()> {
        let caller = Caller::pc("Sentinel");
        for chat in self.store().list_chats()? {
            self.delete_chat(&chat.id, &caller)?;
        }
        self.ensure_a_chat()
    }

    /// Drop the open Brain first, so its watcher stops and its index file closes. The next use
    /// opens an empty vault and seeds it again.
    fn clear_brain(&self) -> Result<()> {
        drop(lock(&self.inner.brain).take());
        let sentinel = self.repo_path().join(".sentinel");
        ignore_missing(fs::remove_dir_all(sentinel.join("brain")))?;
        for name in BRAIN_INDEX_FILES {
            ignore_missing(fs::remove_file(sentinel.join(name)))?;
        }
        Ok(())
    }

    /// The runs, their tasks, and their cost rows. Chats stay. The harness goes back to idle.
    fn clear_history(&self) -> Result<()> {
        self.store().clear_history()?;
        self.store().delete_settings(&RUN_MARKS)?;
        lock(&self.inner.live).state = HarnessState::Idle;
        ignore_missing(fs::remove_file(
            self.repo_root().join(".sentinel/report.md"),
        ))
    }

    /// The defaults come back. The repository folder the user chose stays, because it is what
    /// the project is.
    fn clear_settings(&self) -> Result<()> {
        let kept = Config {
            repo_path: self.config().repo_path,
            ..Config::default()
        };
        config::save(&config::config_path(self.repo_root()), &kept)?;
        *lock(&self.inner.config) = kept;
        self.store().delete_settings(&[TOOL_DEFAULTS_KEY])?;
        self.refresh_repo();
        Ok(())
    }

    /// Every paired phone goes. This computer's own desktop device stays signed in.
    fn clear_devices(&self) -> Result<()> {
        for device in self.store().list_devices()? {
            if device.kind != DeviceKind::Desktop && device.revoked_at.is_none() {
                self.revoke_device(&device.id, "a data reset")?;
            }
        }
        Ok(())
    }

    fn clear_keys(&self) -> Result<()> {
        for name in KEY_NAMES {
            self.delete_secret(name)?;
        }
        Ok(())
    }

    /// Stop the poller if a chat is connected, then reset the settings, the log, and the token.
    fn clear_telegram_data(&self) -> Result<()> {
        if telegram_store::load(self)?.is_some() {
            telegram::stop(self)?;
        }
        self.store().clear_telegram()?;
        self.update_config(|config| config.telegram = TelegramConfig::default())?;
        self.delete_secret(TOKEN_KEY)
    }

    fn delete_secret(&self, name: &str) -> Result<()> {
        self.inner
            .secrets
            .delete(name)
            .map_err(|_| EngineError::Refused("The keychain did not clear that key.".to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_scope_list_with_an_app_scope_is_refused() {
        let error = check_project_scopes(&[DataScope::Chats, DataScope::Keys], false);
        assert!(matches!(error, Err(EngineError::Invalid(_))));
    }

    #[test]
    fn chats_and_history_are_refused_while_a_run_is_live() {
        assert!(check_project_scopes(&[DataScope::Chats], true).is_err());
        assert!(check_project_scopes(&[DataScope::History], true).is_err());
        assert!(check_project_scopes(&[DataScope::Brain, DataScope::Settings], true).is_ok());
    }

    #[test]
    fn the_project_list_is_refused_by_the_engine() {
        assert!(check_app_scopes(&[DataScope::Projects]).is_err());
        assert!(check_app_scopes(&[DataScope::Brain]).is_err());
        assert!(check_app_scopes(&[DataScope::Devices, DataScope::Onboarding]).is_ok());
    }

    #[test]
    fn a_missing_file_is_not_an_error_but_another_failure_is() {
        let missing = io::Error::from(io::ErrorKind::NotFound);
        assert!(ignore_missing(Err(missing)).is_ok());
        let denied = io::Error::from(io::ErrorKind::PermissionDenied);
        assert!(ignore_missing(Err(denied)).is_err());
    }
}
