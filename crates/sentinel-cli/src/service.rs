//! `sentinel service`: run the daemon at boot and restart it when it stops. A launchd job on
//! macOS, a systemd user unit on Linux (`docs/architecture.md`, Always-On Host).

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{Context, Result, bail};

use crate::commands::{home_dir, repo_or_current};

const LABEL: &str = "dev.sentinel.daemon";
const LAUNCHD_FILE: &str = "Library/LaunchAgents/dev.sentinel.daemon.plist";
const SYSTEMD_FILE: &str = ".config/systemd/user/sentinel.service";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Platform {
    Mac,
    Linux,
}

pub(crate) fn platform() -> Result<Platform> {
    if cfg!(target_os = "macos") {
        Ok(Platform::Mac)
    } else if cfg!(target_os = "linux") {
        Ok(Platform::Linux)
    } else {
        bail!(
            "The service is available on macOS (launchd) and Linux (systemd). Windows comes later."
        )
    }
}

/// What to run: the program, the project, and the port.
pub(crate) struct ServiceSpec {
    pub program: PathBuf,
    pub repo: PathBuf,
    pub port: u16,
}

fn service_file(platform: Platform, home: &Path) -> PathBuf {
    match platform {
        Platform::Mac => home.join(LAUNCHD_FILE),
        Platform::Linux => home.join(SYSTEMD_FILE),
    }
}

/// The launchd job: starts at login, restarts when it stops, logs to the project's `.sentinel`.
pub(crate) fn render_launchd(spec: &ServiceSpec) -> String {
    let repo = spec.repo.display();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n<dict>\n\
  <key>Label</key><string>{LABEL}</string>\n\
  <key>ProgramArguments</key>\n  <array>\n\
    <string>{program}</string>\n    <string>daemon</string>\n    <string>--headless</string>\n\
    <string>--repo</string>\n    <string>{repo}</string>\n\
    <string>--port</string>\n    <string>{port}</string>\n  </array>\n\
  <key>RunAtLoad</key><true/>\n  <key>KeepAlive</key><true/>\n\
  <key>StandardOutPath</key><string>{repo}/.sentinel/daemon.log</string>\n\
  <key>StandardErrorPath</key><string>{repo}/.sentinel/daemon.log</string>\n\
</dict>\n</plist>\n",
        program = spec.program.display(),
        port = spec.port,
    )
}

/// The systemd user unit: starts at boot (with linger), restarts on failure.
pub(crate) fn render_systemd(spec: &ServiceSpec) -> String {
    format!(
        "[Unit]\nDescription=Sentinel daemon\nAfter=network-online.target\n\n\
[Service]\nExecStart={program} daemon --headless --repo {repo} --port {port}\n\
Restart=on-failure\nRestartSec=5\n\n\
[Install]\nWantedBy=default.target\n",
        program = spec.program.display(),
        repo = spec.repo.display(),
        port = spec.port,
    )
}

pub(crate) fn install(repo: Option<PathBuf>, port: u16, start: bool) -> Result<ExitCode> {
    let platform = platform()?;
    let spec = ServiceSpec {
        program: std::env::current_exe().context("could not find this program")?,
        repo: repo_or_current(repo)?,
        port,
    };
    let file = service_file(platform, &home_dir());
    let text = match platform {
        Platform::Mac => render_launchd(&spec),
        Platform::Linux => render_systemd(&spec),
    };
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(&file, text).with_context(|| format!("could not write {}", file.display()))?;
    println!("Wrote {}", file.display());
    if start {
        start_service(platform, &file)?;
    }
    if platform == Platform::Linux {
        println!("To start at boot without a login, run once: loginctl enable-linger $USER");
    }
    Ok(ExitCode::SUCCESS)
}

fn start_service(platform: Platform, file: &Path) -> Result<()> {
    let steps: Vec<(&str, Vec<String>)> = match platform {
        Platform::Mac => vec![(
            "launchctl",
            vec!["load".into(), "-w".into(), file.display().to_string()],
        )],
        Platform::Linux => vec![
            ("systemctl", vec!["--user".into(), "daemon-reload".into()]),
            (
                "systemctl",
                vec![
                    "--user".into(),
                    "enable".into(),
                    "--now".into(),
                    "sentinel.service".into(),
                ],
            ),
        ],
    };
    for (program, args) in steps {
        let status = Command::new(program).args(&args).status();
        if !status.is_ok_and(|s| s.success()) {
            bail!(
                "`{program} {}` failed. Start it yourself, or check that it is installed.",
                args.join(" ")
            );
        }
    }
    Ok(())
}

pub(crate) fn uninstall() -> Result<ExitCode> {
    let platform = platform()?;
    let file = service_file(platform, &home_dir());
    if !file.is_file() {
        println!("No service is installed.");
        return Ok(ExitCode::SUCCESS);
    }
    stop_service(platform, &file);
    std::fs::remove_file(&file).with_context(|| format!("could not remove {}", file.display()))?;
    println!("Removed {}", file.display());
    Ok(ExitCode::SUCCESS)
}

/// Stopping is best effort: the service may not be running.
fn stop_service(platform: Platform, file: &Path) {
    let _ = match platform {
        Platform::Mac => Command::new("launchctl")
            .args(["unload", "-w"])
            .arg(file)
            .status(),
        Platform::Linux => Command::new("systemctl")
            .args(["--user", "disable", "--now", "sentinel.service"])
            .status(),
    };
}

pub(crate) fn status() -> Result<ExitCode> {
    let platform = platform()?;
    let file = service_file(platform, &home_dir());
    if file.is_file() {
        println!("Installed: {}", file.display());
    } else {
        println!("Not installed. Run: sentinel service install");
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> ServiceSpec {
        ServiceSpec {
            program: PathBuf::from("/usr/local/bin/sentinel"),
            repo: PathBuf::from("/srv/project"),
            port: 7878,
        }
    }

    #[test]
    fn the_launchd_job_runs_headless_and_restarts() {
        let text = render_launchd(&spec());
        assert!(text.contains("<string>--headless</string>"));
        assert!(text.contains("<key>KeepAlive</key><true/>"));
        assert!(text.contains("/srv/project/.sentinel/daemon.log"));
    }

    #[test]
    fn the_systemd_unit_restarts_on_failure() {
        let text = render_systemd(&spec());
        assert!(text.contains(
            "ExecStart=/usr/local/bin/sentinel daemon --headless --repo /srv/project --port 7878"
        ));
        assert!(text.contains("Restart=on-failure"));
    }

    #[test]
    fn the_service_file_depends_on_the_platform() {
        let home = Path::new("/home/u");
        assert!(service_file(Platform::Linux, home).ends_with("sentinel.service"));
        assert!(service_file(Platform::Mac, home).ends_with("dev.sentinel.daemon.plist"));
    }
}
