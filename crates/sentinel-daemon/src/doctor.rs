//! The doctor install check (task P8-07, `docs/product-scope.md` F18).
//!
//! It asks each tool its version and reports what is missing with the right install
//! command for this operating system. It never installs anything by itself.

use sentinel_core::api::{CheckRow, CheckStatus, DoctorReport};

use crate::cmd::CommandRunner;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsKind {
    Mac,
    Linux,
    Windows,
}

/// The operating system the daemon runs on.
pub fn current_os() -> OsKind {
    match std::env::consts::OS {
        "macos" => OsKind::Mac,
        "windows" => OsKind::Windows,
        _ => OsKind::Linux,
    }
}

/// One tool of the install check that was not found, used to pick the install text.
#[derive(Debug, Clone, Copy)]
enum MissingTool {
    RustToolchain,
    WasmTarget,
    TrunkTools,
    Git,
    Tailscale,
}

const WASM_TARGET: &str = "wasm32-unknown-unknown";

/// Run the install check, one row per tool, always in the same order.
pub async fn install_check(runner: &dyn CommandRunner, os: OsKind) -> DoctorReport {
    let rustc = version_line(runner, "rustc", &["--version"]).await;
    let wasm = wasm_target(runner).await;
    let trunk_and_tools = trunk_and_tools(runner).await;
    let git = version_line(runner, "git", &["--version"]).await;
    let tailscale = version_line(runner, "tailscale", &["version"]).await;
    DoctorReport {
        rows: vec![
            tool_row("Rust toolchain", os, MissingTool::RustToolchain, rustc),
            wasm_row(os, wasm),
            tool_row(
                "trunk and check tools",
                os,
                MissingTool::TrunkTools,
                trunk_and_tools,
            ),
            tool_row("git", os, MissingTool::Git, git),
            tool_row("Tailscale", os, MissingTool::Tailscale, tailscale),
        ],
    }
}

/// The first line a version command printed, when it answered.
async fn version_line(runner: &dyn CommandRunner, program: &str, args: &[&str]) -> Option<String> {
    let output = runner.run(program, args).await;
    if output.status != Some(0) {
        return None;
    }
    output
        .stdout
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

/// `rustup target list --installed` must list the WASM target that the UI needs.
async fn wasm_target(runner: &dyn CommandRunner) -> Option<String> {
    let output = runner
        .run("rustup", &["target", "list", "--installed"])
        .await;
    if output.status != Some(0) {
        return None;
    }
    output
        .stdout
        .lines()
        .map(str::trim)
        .find(|line| *line == WASM_TARGET)
        .map(ToOwned::to_owned)
}

/// trunk, nextest, deny, and machete. Found only when all four answer.
async fn trunk_and_tools(runner: &dyn CommandRunner) -> Option<String> {
    let trunk = version_line(runner, "trunk", &["--version"]).await?;
    let _nextest = version_line(runner, "cargo", &["nextest", "--version"]).await?;
    let _deny = version_line(runner, "cargo", &["deny", "--version"]).await?;
    let _machete = version_line(runner, "cargo", &["machete", "--version"]).await?;
    Some(trunk)
}

fn tool_row(name: &str, os: OsKind, missing: MissingTool, found: Option<String>) -> CheckRow {
    match found {
        Some(detail) => CheckRow {
            name: name.to_owned(),
            status: CheckStatus::Ok,
            detail,
            fix: None,
        },
        None => CheckRow {
            name: name.to_owned(),
            status: CheckStatus::NeedsAction,
            detail: install_text(os, missing).to_owned(),
            fix: Some("Install".to_owned()),
        },
    }
}

fn wasm_row(os: OsKind, found: Option<String>) -> CheckRow {
    match found {
        Some(target) => CheckRow {
            name: "WASM target".to_owned(),
            status: CheckStatus::Ok,
            detail: target,
            fix: None,
        },
        None => CheckRow {
            name: "WASM target".to_owned(),
            status: CheckStatus::NeedsAction,
            detail: install_text(os, MissingTool::WasmTarget).to_owned(),
            fix: Some("Install".to_owned()),
        },
    }
}

/// The exact command that installs one tool on one operating system.
fn install_text(os: OsKind, missing: MissingTool) -> &'static str {
    match missing {
        MissingTool::RustToolchain => match os {
            OsKind::Mac => "brew install rust",
            OsKind::Linux => "sudo apt install rustc",
            OsKind::Windows => "winget install Rustlang.Rustup",
        },
        MissingTool::WasmTarget => "rustup target add wasm32-unknown-unknown",
        MissingTool::TrunkTools => "cargo install trunk cargo-nextest cargo-deny cargo-machete",
        MissingTool::Git => match os {
            OsKind::Mac => "brew install git",
            OsKind::Linux => "sudo apt install git",
            OsKind::Windows => "winget install Git.Git",
        },
        MissingTool::Tailscale => match os {
            OsKind::Mac => "brew install tailscale",
            OsKind::Linux => "curl -fsSL https://tailscale.com/install.sh | sh",
            OsKind::Windows => "winget install tailscale.tailscale",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::{CmdOutput, FakeRunner};

    #[tokio::test]
    async fn every_row_is_ok_when_every_tool_answers() {
        let runner = FakeRunner::new()
            .with("rustc --version", CmdOutput::ok("rustc 1.80.0 (abc)\n"))
            .with(
                "rustup target list --installed",
                CmdOutput::ok("wasm32-unknown-unknown\n"),
            )
            .with("trunk --version", CmdOutput::ok("trunk 0.21.1\n"))
            .with(
                "cargo nextest --version",
                CmdOutput::ok("cargo-nextest 0.9.87\n"),
            )
            .with("cargo deny --version", CmdOutput::ok("cargo-deny 0.15.1\n"))
            .with(
                "cargo machete --version",
                CmdOutput::ok("cargo-machete 0.7.0\n"),
            )
            .with("git --version", CmdOutput::ok("git version 2.46.0\n"))
            .with("tailscale version", CmdOutput::ok("1.72.1\n"));
        let report = install_check(&runner, OsKind::Mac).await;
        let names: Vec<&str> = report.rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Rust toolchain",
                "WASM target",
                "trunk and check tools",
                "git",
                "Tailscale"
            ]
        );
        for row in &report.rows {
            assert_eq!(row.status, CheckStatus::Ok, "row {}", row.name);
            assert_eq!(row.fix, None);
            assert_ne!(row.detail, "");
        }
        let toolchain = &report.rows[0];
        assert_eq!(toolchain.detail, "rustc 1.80.0 (abc)");
        assert_eq!(report.rows[1].detail, "wasm32-unknown-unknown");
    }

    #[tokio::test]
    async fn a_missing_tool_names_the_install_command_for_the_operating_system() {
        let runner = FakeRunner::new();
        for (os, rust_text, git_text, tailscale_text) in [
            (
                OsKind::Mac,
                "brew install rust",
                "brew install git",
                "brew install tailscale",
            ),
            (
                OsKind::Linux,
                "sudo apt install rustc",
                "sudo apt install git",
                "curl -fsSL https://tailscale.com/install.sh | sh",
            ),
            (
                OsKind::Windows,
                "winget install Rustlang.Rustup",
                "winget install Git.Git",
                "winget install tailscale.tailscale",
            ),
        ] {
            let report = install_check(&runner, os).await;
            for row in &report.rows {
                assert_eq!(
                    row.status,
                    CheckStatus::NeedsAction,
                    "row {} on {os:?}",
                    row.name
                );
                assert_eq!(row.fix.as_deref(), Some("Install"));
            }
            assert_eq!(report.rows[0].detail, rust_text);
            assert_eq!(
                report.rows[1].detail,
                "rustup target add wasm32-unknown-unknown"
            );
            assert_eq!(
                report.rows[2].detail,
                "cargo install trunk cargo-nextest cargo-deny cargo-machete"
            );
            assert_eq!(report.rows[3].detail, git_text);
            assert_eq!(report.rows[4].detail, tailscale_text);
        }
    }

    #[tokio::test]
    async fn missing_check_tools_put_the_whole_row_needs_action() {
        let runner = FakeRunner::new()
            .with("rustc --version", CmdOutput::ok("rustc 1.80.0\n"))
            .with(
                "rustup target list --installed",
                CmdOutput::ok("wasm32-unknown-unknown\n"),
            )
            .with("trunk --version", CmdOutput::ok("trunk 0.21.1\n"))
            .with("git --version", CmdOutput::ok("git version 2.46.0\n"))
            .with("tailscale version", CmdOutput::ok("1.72.1\n"));
        let report = install_check(&runner, OsKind::Linux).await;
        let row = &report.rows[2];
        assert_eq!(row.name, "trunk and check tools");
        assert_eq!(row.status, CheckStatus::NeedsAction);
        assert_eq!(
            row.detail,
            "cargo install trunk cargo-nextest cargo-deny cargo-machete"
        );
        assert_eq!(row.fix.as_deref(), Some("Install"));
        assert_eq!(report.rows[0].status, CheckStatus::Ok);
    }
}
