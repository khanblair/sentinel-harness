//! Engine methods for getting at the daemon: Tailscale, pairing, devices, the install check,
//! connections, folders, and onboarding.

use std::path::{Path, PathBuf};

use sentinel_core::api::{
    DoctorReport, FolderEntry, PairResult, PairStart, PairSubmit, RepoCheckResponse,
    TailscaleStatus,
};
use sentinel_core::{DeviceId, DeviceInfo, Event, OnboardingState};

use crate::connections::{self, ConnectionStatus};
use crate::doctor::{current_os, install_check};
use crate::engine::{Engine, EngineError, Result, lock};
use crate::folders;
use crate::onboarding;
use crate::pairing::{PairError, PairOutcome, PendingId, PendingPair};
use crate::tailscale;

impl Engine {
    /// Ask Tailscale and remember the answer for the snapshot.
    pub async fn tailscale_status(&self) -> TailscaleStatus {
        let status = tailscale::status(self.inner.runner.as_ref()).await;
        let changed = {
            let mut known = lock(&self.inner.tailscale);
            let changed = *known != status.state;
            *known = status.state;
            changed
        };
        if changed {
            self.emit_quiet(&Event::TailscaleChanged {
                state: status.state,
            });
        }
        status
    }

    pub async fn tailscale_sign_in(&self) -> tailscale::SignInResult {
        let result = tailscale::sign_in(self.inner.runner.as_ref()).await;
        self.tailscale_status().await;
        result
    }

    /// A new pairing code for the link `base_url`. Shown on the computer.
    pub async fn pair_start(&self, base_url: &str) -> PairStart {
        let status = self.tailscale_status().await;
        lock(&self.inner.pairing).start(base_url, status, self.now())
    }

    /// The phone sends the code. The computer then has to approve the new device.
    pub fn pair_submit(&self, submit: &PairSubmit) -> std::result::Result<PendingId, PairError> {
        let id = lock(&self.inner.pairing).submit(submit, self.now())?;
        self.emit_quiet(&Event::DeviceAction {
            device: DeviceId::new(id.as_str()),
            action: format!("asked to pair as {}", submit.device_name),
        });
        Ok(id)
    }

    pub fn pair_pending(&self) -> Vec<PendingPair> {
        lock(&self.inner.pairing).pending()
    }

    pub fn pair_outcome(&self, id: &PendingId) -> PairOutcome {
        lock(&self.inner.pairing).outcome(id)
    }

    pub fn pair_approve(&self, id: &PendingId) -> std::result::Result<PairResult, PairError> {
        let result = lock(&self.inner.pairing).approve(id, &self.inner.store, self.now())?;
        self.emit_quiet(&Event::DeviceAction {
            device: result.device.clone(),
            action: "paired".to_owned(),
        });
        Ok(result)
    }

    pub fn pair_deny(&self, id: &PendingId) -> std::result::Result<(), PairError> {
        lock(&self.inner.pairing).deny(id)
    }

    pub fn devices(&self) -> Result<Vec<DeviceInfo>> {
        Ok(self.snapshot()?.devices)
    }

    pub fn revoke_device(&self, id: &DeviceId, by: &str) -> Result<()> {
        self.inner.store.revoke_device(id, self.now())?;
        self.emit(&Event::DeviceAction {
            device: id.clone(),
            action: format!("revoked by {by}"),
        })?;
        Ok(())
    }

    /// The install check shown before onboarding and by `sentinel doctor`.
    pub async fn doctor(&self) -> DoctorReport {
        install_check(self.inner.runner.as_ref(), current_os()).await
    }

    /// State and a test for every connection on the Connections page.
    pub async fn connections(&self) -> Vec<ConnectionStatus> {
        let tailscale = self.tailscale_status().await;
        vec![
            tailscale_connection(&tailscale),
            connections::openrouter(self.inner.secrets.as_ref(), "https://openrouter.ai").await,
            connections::mcp_file_server(None),
            connections::git(self.inner.runner.as_ref()).await,
            connections::keychain(self.inner.secrets.as_ref()),
        ]
    }

    pub fn repo_check(&self, path: &str) -> RepoCheckResponse {
        folders::check_repo(Path::new(path))
    }

    /// Browse folders on a phone. Always inside the home folder.
    pub fn list_folders(&self, path: Option<&str>) -> Result<Vec<FolderEntry>> {
        let target = path.map_or_else(|| self.inner.home.clone(), PathBuf::from);
        folders::list_folders(&self.inner.home, &target)
            .map_err(|e| EngineError::Refused(e.to_string()))
    }

    /// The computer's own folder dialog. Blocks, so the caller runs it off the async threads.
    pub fn pick_folder_blocking(&self) -> Option<PathBuf> {
        folders::pick_native(Some(&self.inner.home))
    }

    pub fn create_starter_backlog(&self, path: &str) -> Result<()> {
        folders::create_starter_backlog(Path::new(path))
            .map_err(|e| EngineError::Refused(e.to_string()))?;
        self.refresh_repo();
        Ok(())
    }

    /// Choose the repository the engine works on.
    pub fn set_repo(&self, path: &str) -> Result<RepoCheckResponse> {
        if self.is_run_live() {
            return Err(EngineError::Refused(
                "A run is live. Stop it before choosing another folder.".to_owned(),
            ));
        }
        let check = folders::check_repo(Path::new(path));
        if !check.usable {
            return Err(EngineError::Invalid(format!(
                "{path} is not ready: fix the rows shown"
            )));
        }
        self.update_config(|c| c.repo_path = Some(path.to_owned()))?;
        self.refresh_repo();
        Ok(check)
    }

    pub fn onboarding(&self) -> Result<OnboardingState> {
        Ok(onboarding::load(&self.inner.store)?)
    }

    pub fn onboarding_step(&self, step: u8) -> Result<OnboardingState> {
        Ok(onboarding::set_step(&self.inner.store, step)?)
    }

    pub fn onboarding_skip(&self, step: u8) -> Result<OnboardingState> {
        Ok(onboarding::skip_step(&self.inner.store, step)?)
    }

    pub fn onboarding_complete(&self) -> Result<OnboardingState> {
        Ok(onboarding::complete(&self.inner.store)?)
    }

    pub fn onboarding_reset(&self) -> Result<OnboardingState> {
        Ok(onboarding::reset(&self.inner.store)?)
    }
}

fn tailscale_connection(status: &TailscaleStatus) -> ConnectionStatus {
    use sentinel_core::{ConnectionState, TailscaleState};
    let (state, detail) = match status.state {
        TailscaleState::Running => (
            ConnectionState::Connected,
            status
                .address
                .clone()
                .unwrap_or_else(|| "running".to_owned()),
        ),
        TailscaleState::SignedOut => (ConnectionState::NeedsLogin, "Signed out".to_owned()),
        TailscaleState::NotInstalled => (ConnectionState::Missing, "Not installed".to_owned()),
    };
    ConnectionStatus {
        name: "Tailscale".to_owned(),
        state,
        detail,
    }
}

impl Engine {
    /// The token of "This computer", created on first use. It is saved in `.sentinel/desktop.token`
    /// (readable by this user only) so the CLI and `sentinel open` can reach the daemon. The
    /// daemon stores only its hash.
    pub fn ensure_desktop_token(&self) -> Result<String> {
        let path = self.repo_root().join(".sentinel/desktop.token");
        let known = self
            .inner
            .store
            .list_devices()?
            .into_iter()
            .any(|d| d.kind == sentinel_core::DeviceKind::Desktop && d.revoked_at.is_none());
        if known && let Ok(token) = std::fs::read_to_string(&path) {
            return Ok(token.trim().to_owned());
        }
        let token = crate::hashing::new_token();
        self.inner.store.insert_device(&sentinel_store::Device {
            id: DeviceId::new(uuid::Uuid::new_v4().to_string()),
            name: "This computer".to_owned(),
            kind: sentinel_core::DeviceKind::Desktop,
            token_hash: crate::hashing::hash_token(&token),
            created_at: self.now(),
            revoked_at: None,
        })?;
        write_private(&path, &token)?;
        Ok(token)
    }

    /// Who owns this token, if it is a valid, unrevoked device token.
    pub fn authenticate(&self, token: &str) -> Option<sentinel_store::Device> {
        let hash = crate::hashing::hash_token(token);
        self.inner.store.device_by_token_hash(&hash).ok().flatten()
    }

    /// Output lines of one task, for the Terminal panel.
    pub fn task_output(
        &self,
        task: &sentinel_core::TaskId,
        after: i64,
        limit: usize,
    ) -> Result<Vec<sentinel_core::EventRecord>> {
        let mut found = Vec::new();
        let mut cursor = after;
        while found.len() < limit {
            let page = self.inner.store.events_after(cursor, 2000)?;
            let Some(last) = page.last() else { break };
            cursor = last.id;
            found.extend(
                page.into_iter().filter(
                    |r| matches!(&r.event, Event::AgentOutput { task: t, .. } if t == task),
                ),
            );
        }
        found.truncate(limit);
        Ok(found)
    }
}

#[cfg(unix)]
fn write_private(path: &Path, text: &str) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(text.as_bytes())?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private(path: &Path, text: &str) -> Result<()> {
    std::fs::write(path, text)?;
    Ok(())
}
