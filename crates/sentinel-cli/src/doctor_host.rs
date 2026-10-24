//! The host checks of `sentinel doctor` (P14-06, `docs/product-scope.md` F21). Each row says
//! whether the always-on host is ready, and what to do when it is not.

use sentinel_core::TailscaleState;
use sentinel_core::api::{CheckRow, CheckStatus, HostStatus, ServiceState, TailscaleStatus};
use sentinel_daemon::cmd::CommandRunner;
use sentinel_daemon::doctor::{OsKind, current_os};
use sentinel_daemon::engine::{Engine, Result};

/// A heartbeat this old or older means the daemon is not running.
const STALE_HEARTBEAT_SECONDS: i64 = 60;

/// Read the host state and the sleep settings, then build the rows. `pmset` is `None` on
/// systems without `pmset`, which leaves the sleep row out.
pub(crate) async fn host_check_rows(
    engine: &Engine,
    runner: &dyn CommandRunner,
) -> Result<Vec<CheckRow>> {
    let status = engine.host_status().await?;
    let heartbeat_age = engine
        .last_heartbeat()?
        .map(|at| (engine.now() - at).whole_seconds());
    let pmset = if current_os() == OsKind::Mac {
        Some(runner.run("pmset", &["-g"]).await.stdout)
    } else {
        None
    };
    Ok(host_rows(&status, heartbeat_age, pmset.as_deref()))
}

/// One row per host check, in a fixed order.
pub(crate) fn host_rows(
    status: &HostStatus,
    heartbeat_age: Option<i64>,
    pmset: Option<&str>,
) -> Vec<CheckRow> {
    let mut rows = vec![
        service_row(status.service),
        tailscale_row(&status.tailscale),
        agents_row(status.agents_ready, status.agents_total),
        repository_row(status.repo_path.as_deref()),
        heartbeat_row(heartbeat_age),
    ];
    if let Some(text) = pmset {
        rows.push(sleep_row(text));
    }
    rows
}

fn row(name: &str, status: CheckStatus, detail: impl Into<String>) -> CheckRow {
    CheckRow {
        name: name.to_owned(),
        status,
        detail: detail.into(),
        fix: None,
    }
}

fn service_row(state: ServiceState) -> CheckRow {
    match state {
        ServiceState::Installed => row("Service", CheckStatus::Ok, "Installed."),
        ServiceState::NotInstalled => row(
            "Service",
            CheckStatus::NeedsAction,
            "Run `sentinel service install`.",
        ),
    }
}

fn tailscale_row(status: &TailscaleStatus) -> CheckRow {
    match (status.state, status.address.as_deref()) {
        (TailscaleState::Running, Some(address)) => row(
            "Tailscale",
            CheckStatus::Ok,
            format!("Running at {address}."),
        ),
        (TailscaleState::Running, None) => row(
            "Tailscale",
            CheckStatus::NeedsAction,
            "Tailscale is running but has no address yet. Check it in the Tailscale app.",
        ),
        (TailscaleState::SignedOut, _) => row(
            "Tailscale",
            CheckStatus::NeedsAction,
            "Tailscale is installed but signed out. Sign in to Tailscale on this computer.",
        ),
        (TailscaleState::NotInstalled, _) => row(
            "Tailscale",
            CheckStatus::NeedsAction,
            "Install Tailscale from tailscale.com/download, then sign in.",
        ),
    }
}

fn agents_row(ready: u32, total: u32) -> CheckRow {
    if ready == 0 {
        row(
            "Agent CLIs",
            CheckStatus::NeedsAction,
            "Log in to at least one agent CLI on this machine.",
        )
    } else {
        row(
            "Agent CLIs",
            CheckStatus::Ok,
            format!("{ready} of {total} ready."),
        )
    }
}

fn repository_row(path: Option<&str>) -> CheckRow {
    match path {
        Some(path) => row("Repository", CheckStatus::Ok, path),
        None => row(
            "Repository",
            CheckStatus::NeedsAction,
            "Choose a folder in Settings.",
        ),
    }
}

fn heartbeat_row(age: Option<i64>) -> CheckRow {
    match age {
        Some(seconds) if (0..STALE_HEARTBEAT_SECONDS).contains(&seconds) => row(
            "Heartbeat",
            CheckStatus::Ok,
            format!("Last heartbeat {seconds} seconds ago."),
        ),
        _ => row(
            "Heartbeat",
            CheckStatus::NeedsAction,
            "The daemon is not running. Start it with `sentinel daemon`.",
        ),
    }
}

/// The sleep values of `pmset -g`. A Mac on battery and on power can list two `sleep` lines.
#[derive(Debug, Default, PartialEq, Eq)]
struct SleepSettings {
    sleep_minutes: Vec<u32>,
    sleep_disabled: bool,
}

fn parse_sleep(pmset: &str) -> SleepSettings {
    let mut settings = SleepSettings::default();
    for line in pmset.lines() {
        // Only the first word names the setting. `displaysleep` and `disksleep` must not match.
        let mut words = line.split_whitespace();
        let (Some(key), Some(value)) = (words.next(), words.next()) else {
            continue;
        };
        match key {
            "sleep" => settings.sleep_minutes.extend(value.parse::<u32>().ok()),
            "disablesleep" => settings.sleep_disabled = value == "1",
            _ => {}
        }
    }
    settings
}

fn sleep_row(pmset: &str) -> CheckRow {
    let settings = parse_sleep(pmset);
    if settings.sleep_disabled {
        return row("Sleep settings", CheckStatus::Ok, "Sleep is turned off.");
    }
    if settings.sleep_minutes.is_empty() {
        return row(
            "Sleep settings",
            CheckStatus::NeedsAction,
            "Could not read the sleep settings. Run `pmset -g` to check.",
        );
    }
    if settings.sleep_minutes.iter().all(|minutes| *minutes == 0) {
        row(
            "Sleep settings",
            CheckStatus::Ok,
            "This Mac does not sleep.",
        )
    } else {
        row(
            "Sleep settings",
            CheckStatus::NeedsAction,
            "This Mac sleeps. Run `caffeinate` or change Energy settings.",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready_status() -> HostStatus {
        HostStatus {
            service: ServiceState::Installed,
            headless: true,
            uptime_seconds: 3600,
            last_heartbeat: Some("2026-10-25T10:00:00Z".to_owned()),
            tailscale: TailscaleStatus {
                state: TailscaleState::Running,
                address: Some("100.101.102.103".to_owned()),
                machine_name: None,
            },
            agents_ready: 2,
            agents_total: 3,
            repo_path: Some("/srv/project".to_owned()),
        }
    }

    fn bare_status() -> HostStatus {
        HostStatus {
            service: ServiceState::NotInstalled,
            headless: false,
            uptime_seconds: 0,
            last_heartbeat: None,
            tailscale: TailscaleStatus {
                state: TailscaleState::NotInstalled,
                address: None,
                machine_name: None,
            },
            agents_ready: 0,
            agents_total: 0,
            repo_path: None,
        }
    }

    fn statuses(rows: &[CheckRow]) -> Vec<CheckStatus> {
        rows.iter().map(|row| row.status).collect()
    }

    #[test]
    fn a_ready_host_has_every_row_ok() {
        let rows = host_rows(&ready_status(), Some(5), Some("sleep 0\n"));
        assert_eq!(rows.len(), 6);
        assert!(statuses(&rows).iter().all(|s| *s == CheckStatus::Ok));
        assert_eq!(rows[1].detail, "Running at 100.101.102.103.");
    }

    #[test]
    fn a_bare_host_says_what_to_fix() {
        let rows = host_rows(&bare_status(), None, None);
        assert_eq!(rows.len(), 5, "no sleep row off macOS");
        assert!(
            statuses(&rows)
                .iter()
                .all(|s| *s == CheckStatus::NeedsAction)
        );
        assert_eq!(rows[0].detail, "Run `sentinel service install`.");
        assert_eq!(
            rows[2].detail,
            "Log in to at least one agent CLI on this machine."
        );
        assert_eq!(
            rows[4].detail,
            "The daemon is not running. Start it with `sentinel daemon`."
        );
    }

    #[test]
    fn pmset_reads_only_the_sleep_and_disablesleep_settings() {
        let sleeps_never = "sleep 0\ndisplaysleep 10\ndisksleep 10\n";
        assert_eq!(sleep_row(sleeps_never).status, CheckStatus::Ok);
        assert_eq!(
            sleep_row("sleep 10\n").status,
            CheckStatus::NeedsAction,
            "a Mac that sleeps after 10 minutes needs action"
        );
        assert_eq!(
            sleep_row("sleep 10\ndisablesleep 1\n").status,
            CheckStatus::Ok
        );
        assert_eq!(
            sleep_row("sleep 1 (sleep prevented by sshd)\n").status,
            CheckStatus::NeedsAction
        );
    }
}
