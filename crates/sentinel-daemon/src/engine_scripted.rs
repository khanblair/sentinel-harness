//! Options for an engine that runs scripted agents on a repository, with state in memory. The
//! tests and `sentinel selftest` use it.

use std::path::Path;
use std::sync::Arc;

use crate::cmd::SystemRunner;
use crate::engine::{EngineOptions, FakeAgents};
use crate::secrets::MemoryStore;

impl EngineOptions {
    /// Scripted agents on `repo_root`. The state lives in memory and the home is the repository.
    pub fn scripted(repo_root: &Path, fake: FakeAgents) -> EngineOptions {
        EngineOptions {
            repo_root: repo_root.to_path_buf(),
            home: repo_root.to_path_buf(),
            runner: Arc::new(SystemRunner),
            secrets: Arc::new(MemoryStore::new()),
            fake_agents: Some(fake),
            in_memory: true,
        }
    }
}
