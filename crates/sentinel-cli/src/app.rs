//! `sentinel app`: what the macOS app runs when you open it. It picks a project (the last one, or
//! a folder dialog), starts the daemon on this computer, and opens the UI in the browser. Open the
//! app again and it only opens the browser. Quit with Settings, General, or `sentinel stop`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use sentinel_daemon::engine::Engine;
use sentinel_daemon::folders::{alert, pick_native};
use sentinel_daemon::server::{self, BoundServer, ServerOptions};
use sentinel_daemon::tailscale::BindMode;

use crate::commands::home_dir;
use crate::serve;

const PORT: u16 = 7878;

/// Where the last project is remembered between launches.
fn last_repo_file() -> PathBuf {
    home_dir().join(".sentinel/last_repo")
}

fn remembered_repo() -> Option<PathBuf> {
    let text = std::fs::read_to_string(last_repo_file()).ok()?;
    let path = PathBuf::from(text.trim());
    path.join(".git").exists().then_some(path)
}

pub(crate) fn remember(repo: &Path) {
    let file = last_repo_file();
    if let Some(folder) = file.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    // Forgetting the folder only costs one dialog at the next launch.
    let _ = std::fs::write(file, repo.display().to_string());
}

/// The project to serve: the one named, the last one, or one the owner picks. `None` when the
/// owner closes the dialog.
pub(crate) fn choose_repo(named: Option<PathBuf>) -> Result<Option<PathBuf>> {
    let picked = match named.or_else(remembered_repo) {
        Some(path) => path,
        None => match pick_native(Some(&home_dir())) {
            Some(path) => path,
            None => return Ok(None),
        },
    };
    let repo = picked
        .canonicalize()
        .with_context(|| format!("{} is not a folder", picked.display()))?;
    if !repo.join(".git").exists() {
        alert(&format!(
            "{} is not a git repository. Choose the folder that holds your project's .git folder.",
            repo.display()
        ));
        return Ok(None);
    }
    Ok(Some(repo))
}

/// The tailnet address when Tailscale is up, so a phone can reach it. Otherwise this computer
/// only: 127.0.0.1, which nothing else can reach. Port 7878, or a free one when it is taken.
pub(crate) async fn bind_here(engine: &Engine) -> Result<BoundServer> {
    let attempts = [
        (BindMode::Tailscale, PORT),
        (BindMode::DevLoopback, PORT),
        (BindMode::DevLoopback, 0),
    ];
    let mut last = None;
    for (bind, port) in attempts {
        match server::bind(engine.clone(), &ServerOptions { port, bind }).await {
            Ok(bound) => return Ok(bound),
            Err(error) => last = Some(error),
        }
    }
    Err(last.map_or_else(
        || anyhow::anyhow!("no address to serve on"),
        |error| anyhow::anyhow!("could not bind: {error}"),
    ))
}

pub(crate) async fn app(repo: Option<PathBuf>) -> Result<ExitCode> {
    let Some(repo) = choose_repo(repo)? else {
        return Ok(ExitCode::SUCCESS);
    };
    remember(&repo);
    if serve::is_running(&repo) {
        return serve::open(Some(repo), false);
    }
    let engine = serve::prepare(&repo, false, false).await?;
    let bound = bind_here(&engine).await?;
    serve::publish(&repo, &bound)?;
    serve::open(Some(repo.clone()), false)?;
    serve::run_until_stopped(&engine, &repo, bound).await
}
