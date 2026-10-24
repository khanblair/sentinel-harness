//! Tailscale status and the bind address for the daemon (task P7-01).
//!
//! The daemon binds to this computer's tailnet address only. Any other address such as
//! `0.0.0.0` would expose the API, so it is refused (`docs/architecture.md`, Security Model).

use std::net::{IpAddr, Ipv4Addr};

use sentinel_core::api::TailscaleStatus;
use sentinel_core::state::TailscaleState;
use serde::Deserialize;

use crate::cmd::{CmdOutput, CommandRunner};

/// Why the daemon cannot bind to the tailnet address.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BindError {
    #[error("tailscale is not running (state {0:?})")]
    NoTailscale(TailscaleState),
    #[error("that address is never a valid bind address")]
    Refused,
}

/// Where the daemon serves the UI. Loopback is an explicit dev flag (D50), off by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindMode {
    Tailscale,
    DevLoopback,
}

/// What `tailscale up` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignInResult {
    pub ok: bool,
    pub message: String,
}

/// Check this computer's Tailscale state with `tailscale status --json`.
pub async fn status(runner: &dyn CommandRunner) -> TailscaleStatus {
    let output = runner.run("tailscale", &["status", "--json"]).await;
    parse_status(&output)
}

/// Read a `TailscaleState`, an address, and a machine name out of one status run.
pub fn parse_status(output: &CmdOutput) -> TailscaleStatus {
    match serde_json::from_str::<StatusJson>(&output.stdout) {
        Ok(json) => status_from_json(json),
        // The status command answers `0` with plain text when it still works.
        Err(_) if output.status == Some(0) => signed_out(),
        Err(_) if never_started_or_missing(output) => not_installed(),
        Err(_) => signed_out(),
    }
}

/// Start the sign-in flow with `tailscale up`.
pub async fn sign_in(runner: &dyn CommandRunner) -> SignInResult {
    let output = runner.run("tailscale", &["up"]).await;
    sign_in_from_output(&output)
}

/// The address the daemon may bind to for one mode.
pub fn bind_address(status: &TailscaleStatus, mode: BindMode) -> Result<IpAddr, BindError> {
    match mode {
        BindMode::DevLoopback => Ok(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        BindMode::Tailscale => tailnet_address(status),
    }
}

fn tailnet_address(status: &TailscaleStatus) -> Result<IpAddr, BindError> {
    if status.state != TailscaleState::Running {
        return Err(BindError::NoTailscale(status.state));
    }
    let text = status.address.as_ref().ok_or(BindError::Refused)?;
    let address = text.parse::<IpAddr>().map_err(|_| BindError::Refused)?;
    match address {
        IpAddr::V4(ip) if ip.is_unspecified() => Err(BindError::Refused),
        IpAddr::V6(ip) if ip.is_unspecified() => Err(BindError::Refused),
        IpAddr::V4(_) | IpAddr::V6(_) => Ok(address),
    }
}

fn sign_in_from_output(output: &CmdOutput) -> SignInResult {
    if output.status == Some(0) {
        let message = output.stdout.trim();
        let message = if message.is_empty() {
            "Signed in to Tailscale.".to_owned()
        } else {
            message.to_owned()
        };
        return SignInResult { ok: true, message };
    }
    SignInResult {
        ok: false,
        message: failure_text(output),
    }
}

fn failure_text(output: &CmdOutput) -> String {
    let stderr = output.stderr.trim();
    if !stderr.is_empty() {
        return stderr.to_owned();
    }
    match output.status {
        Some(code) => format!("tailscale up exited with status {code}."),
        None => "tailscale is not installed.".to_owned(),
    }
}

fn status_from_json(json: StatusJson) -> TailscaleStatus {
    if json.backend_state != "Running" {
        return signed_out();
    }
    let Some(node) = json.node else {
        return signed_out();
    };
    make_status(
        TailscaleState::Running,
        first_ipv4(&node),
        machine_name(&node),
    )
}

fn not_installed() -> TailscaleStatus {
    make_status(TailscaleState::NotInstalled, None, None)
}

fn signed_out() -> TailscaleStatus {
    make_status(TailscaleState::SignedOut, None, None)
}

fn make_status(
    state: TailscaleState,
    address: Option<String>,
    machine_name: Option<String>,
) -> TailscaleStatus {
    TailscaleStatus {
        state,
        address,
        machine_name,
    }
}

/// A run that never started, or a shell that could not find the program.
fn never_started_or_missing(output: &CmdOutput) -> bool {
    output.status.is_none()
        || output.stderr.to_ascii_lowercase().contains("not found")
        || output.stderr.contains("No such file")
}

#[derive(Debug, Deserialize)]
struct StatusJson {
    #[serde(default, rename = "BackendState")]
    backend_state: String,
    #[serde(rename = "Self")]
    node: Option<SelfNode>,
}

#[derive(Debug, Deserialize)]
struct SelfNode {
    #[serde(default, rename = "TailscaleIPs")]
    tailscale_ips: Vec<String>,
    #[serde(default, rename = "DNSName")]
    dns_name: String,
    #[serde(default, rename = "MagicDNSSuffix")]
    magic_dns_suffix: String,
    #[serde(default, rename = "HostName")]
    host_name: String,
}

fn first_ipv4(node: &SelfNode) -> Option<String> {
    node.tailscale_ips
        .iter()
        .map(String::as_str)
        .filter_map(|ip| ip.parse::<IpAddr>().ok())
        .find_map(|ip| match ip {
            IpAddr::V4(ip) => Some(ip.to_string()),
            IpAddr::V6(_) => None,
        })
}

/// With `MagicDNS` on the DNS name is the machine name, without its trailing dot.
/// Without it only the host name is meaningful.
fn machine_name(node: &SelfNode) -> Option<String> {
    let name = if node.magic_dns_suffix.is_empty() {
        node.host_name.clone()
    } else {
        match node.dns_name.strip_suffix('.') {
            Some(without_dot) => without_dot.to_owned(),
            None => node.dns_name.clone(),
        }
    };
    if name.is_empty() { None } else { Some(name) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::FakeRunner;

    const RUNNING_JSON: &str = r#"{
        "BackendState": "Running",
        "Self": {
            "TailscaleIPs": ["100.64.0.9", "fd7a:115c:a1e0::9"],
            "DNSName": "work-laptop.tail-scale.ts.net.",
            "MagicDNSSuffix": "tail-scale.ts.net",
            "HostName": "work-laptop"
        }
    }"#;

    #[test]
    fn running_state_takes_the_first_ipv4_and_the_dns_name_without_a_dot() {
        let status = parse_status(&CmdOutput::ok(RUNNING_JSON));
        assert_eq!(status.state, TailscaleState::Running);
        assert_eq!(status.address.as_deref(), Some("100.64.0.9"));
        assert_eq!(
            status.machine_name.as_deref(),
            Some("work-laptop.tail-scale.ts.net")
        );
    }

    #[test]
    fn running_state_takes_the_host_name_when_magic_dns_is_off() {
        let json = r#"{
            "BackendState": "Running",
            "Self": {
                "TailscaleIPs": ["100.64.0.9"],
                "DNSName": "work-laptop.tail-scale.ts.net.",
                "MagicDNSSuffix": "",
                "HostName": "work-laptop"
            }
        }"#;
        let status = parse_status(&CmdOutput::ok(json));
        assert_eq!(status.machine_name.as_deref(), Some("work-laptop"));
    }

    #[test]
    fn needs_login_state_is_signed_out_without_an_address() {
        let json = r#"{ "BackendState": "NeedsLogin" }"#;
        assert_eq!(
            parse_status(&CmdOutput::ok(json)),
            make_status(TailscaleState::SignedOut, None, None)
        );
    }

    #[test]
    fn stopped_state_is_signed_out_without_an_address() {
        let json = r#"{ "BackendState": "Stopped" }"#;
        assert_eq!(
            parse_status(&CmdOutput::ok(json)),
            make_status(TailscaleState::SignedOut, None, None)
        );
    }

    #[test]
    fn unparseable_output_with_a_zero_exit_is_signed_out() {
        let output = CmdOutput {
            status: Some(0),
            stdout: "plain text\n".to_owned(),
            stderr: String::new(),
        };
        assert_eq!(
            parse_status(&output),
            make_status(TailscaleState::SignedOut, None, None)
        );
    }

    #[test]
    fn a_never_started_command_is_not_installed() {
        assert_eq!(
            parse_status(&CmdOutput::never_started()),
            make_status(TailscaleState::NotInstalled, None, None)
        );
    }

    #[test]
    fn a_failed_command_without_the_program_is_not_installed() {
        let output = CmdOutput::failure(127, "/bin/sh: tailscale: command not found\n");
        assert_eq!(parse_status(&output).state, TailscaleState::NotInstalled);
    }

    #[test]
    fn some_other_failure_is_signed_out() {
        let output = CmdOutput::failure(1, "permission denied\n");
        assert_eq!(parse_status(&output).state, TailscaleState::SignedOut);
    }

    #[tokio::test]
    async fn status_runs_the_real_status_command() {
        let runner = FakeRunner::new().with("tailscale status --json", CmdOutput::ok(RUNNING_JSON));
        assert_eq!(status(&runner).await.state, TailscaleState::Running);
    }

    #[test]
    fn bind_address_is_the_tailnet_address_when_running() {
        let running = make_status(TailscaleState::Running, Some("100.64.0.9".to_owned()), None);
        assert_eq!(
            bind_address(&running, BindMode::Tailscale),
            Ok("100.64.0.9".parse().unwrap())
        );
    }

    #[test]
    fn bind_address_reports_no_tailscale_when_not_running() {
        let signed_out_status = make_status(TailscaleState::SignedOut, None, None);
        assert_eq!(
            bind_address(&signed_out_status, BindMode::Tailscale),
            Err(BindError::NoTailscale(TailscaleState::SignedOut))
        );
        let not_installed_status = make_status(TailscaleState::NotInstalled, None, None);
        assert_eq!(
            bind_address(&not_installed_status, BindMode::Tailscale),
            Err(BindError::NoTailscale(TailscaleState::NotInstalled))
        );
    }

    #[test]
    fn bind_address_refuses_the_wildcard_status_addresses() {
        for refused in ["0.0.0.0", "::"] {
            let wildcard = make_status(TailscaleState::Running, Some(refused.to_owned()), None);
            assert_eq!(
                bind_address(&wildcard, BindMode::Tailscale),
                Err(BindError::Refused)
            );
        }
    }

    #[test]
    fn bind_address_refuses_an_address_it_cannot_read() {
        let broken = make_status(TailscaleState::Running, Some("not-an-ip".to_owned()), None);
        assert_eq!(
            bind_address(&broken, BindMode::Tailscale),
            Err(BindError::Refused)
        );
    }

    #[test]
    fn dev_loopback_always_binds_localhost() {
        let running = make_status(TailscaleState::Running, Some("100.64.0.9".to_owned()), None);
        assert_eq!(
            bind_address(&running, BindMode::DevLoopback),
            Ok(IpAddr::V4(Ipv4Addr::LOCALHOST))
        );
    }

    #[tokio::test]
    async fn sign_in_reports_ok_when_tailscale_up_succeeds() {
        let runner = FakeRunner::new().with(
            "tailscale up",
            CmdOutput {
                status: Some(0),
                stdout: "Success.\n".to_owned(),
                stderr: String::new(),
            },
        );
        let signing_in = sign_in(&runner).await;
        assert!(signing_in.ok);
        assert_eq!(signing_in.message, "Success.");
    }

    #[tokio::test]
    async fn sign_in_reports_the_failure_text_from_stderr() {
        let runner = FakeRunner::new().with(
            "tailscale up",
            CmdOutput::failure(1, "not logged in\nHealth check failed\n"),
        );
        let signing_in = sign_in(&runner).await;
        assert!(!signing_in.ok);
        assert_eq!(signing_in.message, "not logged in\nHealth check failed");
    }
}
