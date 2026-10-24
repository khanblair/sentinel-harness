//! The commands that run or reach the daemon: `daemon`, `open`, `pair`, and `keys`.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use sentinel_core::api::PairStart;
use sentinel_daemon::secrets::{EnvFallback, KeychainStore, SecretStore};
use sentinel_daemon::server::{self, ServerOptions};
use sentinel_daemon::tailscale::BindMode;

use crate::commands::{open_engine, repo_or_current};

/// Where the running daemon writes its address, so `open` and `pair` can find it.
fn addr_file(repo: &Path) -> PathBuf {
    repo.join(".sentinel/daemon.addr")
}

pub(crate) struct DaemonArgs {
    pub repo: Option<PathBuf>,
    pub port: u16,
    pub dev_bind: bool,
    pub fake_agents: bool,
}

pub(crate) async fn daemon(args: DaemonArgs) -> Result<ExitCode> {
    let repo = repo_or_current(args.repo)?;
    let engine = open_engine(&repo, args.fake_agents)?;
    engine.scan_tools().await?;
    engine.ensure_desktop_token()?;
    let bind = if args.dev_bind {
        BindMode::DevLoopback
    } else {
        BindMode::Tailscale
    };
    let options = ServerOptions {
        port: args.port,
        bind,
    };
    let bound = server::bind(engine, &options).await.context(
        "could not bind. Sign in to Tailscale, or pass --dev-bind to serve on 127.0.0.1 for development",
    )?;
    std::fs::write(addr_file(&repo), bound.addr.to_string())
        .context("could not write .sentinel/daemon.addr")?;
    println!(
        "Sentinel is serving {} on http://{}",
        repo.display(),
        bound.addr
    );
    println!("Open the UI on this computer with: sentinel open");
    tokio::select! {
        served = bound.serve() => served.context("the server stopped")?,
        _ = tokio::signal::ctrl_c() => println!("\nStopping."),
    }
    let _ = std::fs::remove_file(addr_file(&repo));
    Ok(ExitCode::SUCCESS)
}

/// The address of the running daemon and this computer's token.
fn daemon_access(repo: &Path) -> Result<(String, String)> {
    let addr = std::fs::read_to_string(addr_file(repo))
        .context("no daemon is running here. Start one with: sentinel daemon")?;
    let token = std::fs::read_to_string(repo.join(".sentinel/desktop.token"))
        .context("no desktop token yet. Start the daemon once with: sentinel daemon")?;
    Ok((addr.trim().to_owned(), token.trim().to_owned()))
}

/// The token travels in the URL fragment: a fragment never leaves the browser (D63).
fn open_url(addr: &str, token: &str) -> String {
    format!("http://{addr}/#t={token}")
}

pub(crate) fn open(repo: Option<PathBuf>, print_only: bool) -> Result<ExitCode> {
    let (addr, token) = daemon_access(&repo_or_current(repo)?)?;
    let url = open_url(&addr, &token);
    if print_only {
        println!("{url}");
        return Ok(ExitCode::SUCCESS);
    }
    let launcher = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    let exit = std::process::Command::new(launcher).arg(&url).status();
    if !exit.is_ok_and(|status| status.success()) {
        println!("Could not open a browser. Open this link yourself:\n{url}");
    }
    Ok(ExitCode::SUCCESS)
}

pub(crate) async fn pair(repo: Option<PathBuf>) -> Result<ExitCode> {
    let (addr, token) = daemon_access(&repo_or_current(repo)?)?;
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/pair/start"))
        .bearer_auth(token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .context("could not reach the daemon")?;
    if !response.status().is_success() {
        bail!("the daemon refused: {}", response.status());
    }
    let start: PairStart = response.json().await.context("unexpected reply")?;
    if start.url.is_empty() {
        bail!(
            "Tailscale is not running, so there is no link for a phone yet. Run: sentinel doctor"
        );
    }
    println!(
        "Open this link on the phone (it must be on your tailnet):\n  {}",
        start.url
    );
    println!("One-time code: {}", start.code);
    println!("The code works for {} seconds.", start.expires_in_seconds);
    println!("Then approve the phone in the Remote panel on this computer.");
    Ok(ExitCode::SUCCESS)
}

/// The keychain, with an environment variable as the fallback for reading.
fn store() -> EnvFallback<KeychainStore> {
    EnvFallback::new(KeychainStore)
}

pub(crate) enum KeysAction {
    Set,
    Delete,
    Status,
}

/// `sentinel keys set openrouter` reads the key from standard input, never from an argument.
pub(crate) fn keys(action: &KeysAction, name: &str) -> Result<ExitCode> {
    let store = store();
    match action {
        KeysAction::Set => {
            let mut value = String::new();
            std::io::stdin()
                .read_to_string(&mut value)
                .context("could not read the key from standard input")?;
            let value = value.trim();
            if value.is_empty() {
                bail!("the key is empty. Pipe it in: echo \"$KEY\" | sentinel keys set {name}");
            }
            store
                .set(name, value)
                .context("the keychain refused the key")?;
            println!("Saved {name} to the keychain.");
        }
        KeysAction::Delete => {
            store.delete(name).context("the keychain refused")?;
            println!("Removed {name} from the keychain.");
        }
        KeysAction::Status => {
            let present = store.get(name).context("the keychain refused")?.is_some();
            println!("{name}: {}", if present { "set" } else { "not set" });
        }
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_open_link_carries_the_token_in_the_fragment() {
        let url = open_url("127.0.0.1:7878", "tok");
        assert_eq!(url, "http://127.0.0.1:7878/#t=tok");
        assert!(!url.contains('?'));
    }

    #[test]
    fn the_address_file_lives_in_the_sentinel_folder() {
        assert_eq!(
            addr_file(Path::new("/repo")),
            PathBuf::from("/repo/.sentinel/daemon.addr")
        );
    }
}
