//! The projects the app serves besides the first one. The first project starts the host loop, the
//! routines, and Telegram. The others only need an engine that answers requests.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sentinel_daemon::engine::Engine;
use sentinel_daemon::hub::{Hub, OpenFuture, ProjectOpener};

use crate::commands::open_engine;

/// Opens the engine of one more project, the way the command line opens the first one.
pub(crate) struct CliOpener {
    pub headless: bool,
}

impl ProjectOpener for CliOpener {
    fn open(&self, path: PathBuf) -> OpenFuture {
        let headless = self.headless;
        Box::pin(async move { open_project(&path, headless).await })
    }
}

async fn open_project(path: &Path, headless: bool) -> Result<Engine, String> {
    let engine = open_engine(path).map_err(|error| format!("{error:#}"))?;
    engine.set_headless(headless);
    engine
        .scan_tools()
        .await
        .map_err(|error| format!("{error:#}"))?;
    Ok(engine)
}

/// The hub of the app: the first project, and the projects kept from the last run.
pub(crate) async fn hub_of(engine: &Engine, headless: bool) -> Hub {
    let opener = Arc::new(CliOpener { headless });
    let hub = Hub::new(engine.clone(), Some(opener));
    hub.load_saved().await;
    hub
}
