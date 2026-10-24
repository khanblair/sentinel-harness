//! The always-on host (`docs/product-scope.md`, F21). The daemon writes a heartbeat every few
//! seconds. After a stop of more than a minute, the next start says it is back online. A run that
//! was in flight is picked up by crash resume (`resume.rs`), not by this module.

use std::path::PathBuf;
use std::time::Duration;

use sentinel_core::Event;
use sentinel_core::api::{HostStatus, ServiceState};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::engine::{Engine, Result};

/// How often the heartbeat is written.
const BEAT_EVERY: Duration = Duration::from_secs(10);
/// A gap longer than this makes the next start say it is back online.
const GAP_SECONDS: u64 = 60;
const BEAT_KEY: &str = "host.heartbeat";
/// The launchd job and the systemd unit that `sentinel service install` writes.
const LAUNCHD_FILE: &str = "Library/LaunchAgents/dev.sentinel.daemon.plist";
const SYSTEMD_FILE: &str = ".config/systemd/user/sentinel.service";

impl Engine {
    /// Say the daemon runs without native dialogs.
    pub fn set_headless(&self, headless: bool) {
        self.inner
            .headless
            .store(headless, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn is_headless(&self) -> bool {
        self.inner
            .headless
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Write the heartbeat now.
    pub fn beat(&self) -> Result<()> {
        let text = self
            .now()
            .format(&Rfc3339)
            .map_err(|e| crate::engine::EngineError::Invalid(e.to_string()))?;
        self.store().set_setting(BEAT_KEY, &text)?;
        Ok(())
    }

    /// The last heartbeat, when one was written.
    pub fn last_heartbeat(&self) -> Result<Option<OffsetDateTime>> {
        let text = self.store().get_setting(BEAT_KEY)?;
        Ok(text.and_then(|t| OffsetDateTime::parse(&t, &Rfc3339).ok()))
    }

    /// How long the daemon was down, in seconds, when the gap is long enough to report.
    pub fn downtime_seconds(&self) -> Result<Option<u64>> {
        let Some(last) = self.last_heartbeat()? else {
            return Ok(None);
        };
        let gap = (self.now() - last).whole_seconds();
        Ok(u64::try_from(gap).ok().filter(|gap| *gap > GAP_SECONDS))
    }

    /// Report a gap once, then beat for as long as the daemon runs.
    pub fn start_host_loop(&self) {
        if let Ok(Some(gap_seconds)) = self.downtime_seconds() {
            self.emit_quiet(&Event::HostBackOnline { gap_seconds });
        }
        let engine = self.clone();
        tokio::spawn(async move {
            loop {
                if let Err(error) = engine.beat() {
                    eprintln!("sentinel: could not write the heartbeat: {error}");
                }
                tokio::time::sleep(BEAT_EVERY).await;
            }
        });
    }

    /// What the host card shows.
    pub async fn host_status(&self) -> Result<HostStatus> {
        let tools = self.store().list_tools()?;
        let ready = tools
            .iter()
            .filter(|t| t.enabled && t.status.is_ready())
            .count();
        Ok(HostStatus {
            service: service_state(self.home()),
            headless: self.is_headless(),
            uptime_seconds: u64::try_from((self.now() - self.inner.started).whole_seconds())
                .unwrap_or(0),
            last_heartbeat: self
                .last_heartbeat()?
                .and_then(|at| at.format(&Rfc3339).ok()),
            tailscale: self.tailscale_status().await,
            agents_ready: u32::try_from(ready).unwrap_or(0),
            agents_total: u32::try_from(tools.len()).unwrap_or(0),
            repo_path: Some(self.repo_path().to_string_lossy().into_owned()),
        })
    }
}

/// The service files that `sentinel service install` writes, from the home folder.
fn service_files(home: &std::path::Path) -> [PathBuf; 2] {
    [home.join(LAUNCHD_FILE), home.join(SYSTEMD_FILE)]
}

pub(crate) fn service_state(home: &std::path::Path) -> ServiceState {
    if service_files(home).iter().any(|path| path.is_file()) {
        ServiceState::Installed
    } else {
        ServiceState::NotInstalled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_service_file_means_not_installed() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(service_state(home.path()), ServiceState::NotInstalled);
    }

    #[test]
    fn a_launchd_file_means_installed() {
        let home = tempfile::tempdir().unwrap();
        let file = home.path().join(LAUNCHD_FILE);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, "x").unwrap();
        assert_eq!(service_state(home.path()), ServiceState::Installed);
    }
}
