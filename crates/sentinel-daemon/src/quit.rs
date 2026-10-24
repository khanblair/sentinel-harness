//! A way to stop the daemon from the UI. The app has no terminal, so Ctrl+C is not there.

use crate::engine::Engine;

impl Engine {
    /// Ask the process that serves this engine to stop. The command line waits on
    /// [`Engine::quit_requested`] next to its Ctrl+C handler.
    pub fn request_quit(&self) {
        self.inner.quit.notify_one();
    }

    /// Resolves once a quit was asked for.
    pub async fn quit_requested(&self) {
        self.inner.quit.notified().await;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::cmd::SystemRunner;
    use crate::engine::{Engine, EngineOptions};
    use crate::secrets::MemoryStore;
    use std::sync::Arc;

    #[tokio::test]
    async fn a_quit_request_wakes_the_waiter() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::open(EngineOptions {
            repo_root: dir.path().to_path_buf(),
            home: dir.path().to_path_buf(),
            runner: Arc::new(SystemRunner),
            secrets: Arc::new(MemoryStore::new()),
            fake_agents: None,
            in_memory: true,
        })
        .unwrap();
        engine.request_quit();
        let woke = tokio::time::timeout(Duration::from_secs(2), engine.quit_requested()).await;
        assert!(woke.is_ok());
    }
}
