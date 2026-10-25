//! `sentinel app`: what the macOS app runs when you open it. It picks a project (the last one, or
//! a folder dialog), starts the daemon on this computer, and opens the UI in the browser. Open the
//! app again and it only opens the browser. Quit with Settings, General, or `sentinel stop`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use sentinel_daemon::folders::{Pick, alert, pick_native};
use sentinel_daemon::hub::Hub;
use sentinel_daemon::server::{self, BoundServer, ServerOptions};
use sentinel_daemon::starter::{self, FirstChoice};
use sentinel_daemon::tailscale::BindMode;

use crate::commands::home_dir;
use crate::projects;
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

/// The project to serve: the one named, the last one, one the owner picks, or a new starter
/// project. `None` when the owner quits or closes the dialog.
pub(crate) fn choose_repo(named: Option<PathBuf>) -> Result<Option<PathBuf>> {
    if let Some(path) = named.or_else(remembered_repo) {
        return checked_repo(&path);
    }
    match starter::ask_first_choice() {
        FirstChoice::PickFolder => match pick_native(Some(&home_dir())) {
            Pick::Chosen(path) => checked_repo(&path),
            Pick::Cancelled | Pick::Unavailable => Ok(None),
        },
        FirstChoice::MakeStarter => make_starter(),
        FirstChoice::Quit => Ok(None),
    }
}

/// The folder as a full path, when it holds a git repository. Otherwise a message, and `None`.
fn checked_repo(picked: &Path) -> Result<Option<PathBuf>> {
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

/// Make the starter project in the home folder and open it.
fn make_starter() -> Result<Option<PathBuf>> {
    let folder = starter::create_starter_project(&home_dir())
        .context("could not make the starter project")?;
    starter::set_starter_test_command(&folder).context("could not save the starter settings")?;
    checked_repo(&folder)
}

/// The tailnet address when Tailscale is up, so a phone can reach it. Otherwise this computer
/// only: 127.0.0.1, which nothing else can reach. Port 7878, or a free one when it is taken.
pub(crate) async fn bind_here(hub: &Hub) -> Result<BoundServer> {
    let attempts = [
        (BindMode::Tailscale, PORT),
        (BindMode::DevLoopback, PORT),
        (BindMode::DevLoopback, 0),
    ];
    let mut last = None;
    for (bind, port) in attempts {
        match server::bind_hub(hub.clone(), &ServerOptions { port, bind }).await {
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
    let engine = serve::prepare(&repo, false).await?;
    let hub = projects::hub_of(&engine, false).await;
    let bound = bind_here(&hub).await?;
    serve::publish(&repo, &bound)?;
    serve::open(Some(repo.clone()), false)?;
    serve::run_until_stopped(&engine, &repo, bound).await
}
