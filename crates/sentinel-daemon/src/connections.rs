//! Live state of the things Sentinel connects to (`GET /api/connections`).

use std::path::Path;
use std::time::Duration;

use sentinel_core::ConnectionState;

use crate::cmd::{CmdOutput, CommandRunner};
use crate::secrets::{SecretError, SecretStore};

const KEYCHAIN_PROBE: &str = "sentinel-probe";
const PROBE_VALUE: &str = "probe";
const OPENROUTER_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionStatus {
    pub name: String,
    pub state: ConnectionState,
    pub detail: String,
}

impl ConnectionStatus {
    fn new(name: &str, state: ConnectionState, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_owned(),
            state,
            detail: detail.into(),
        }
    }
}

pub async fn git(runner: &dyn CommandRunner) -> ConnectionStatus {
    let output = runner
        .run("git", &["rev-parse", "--is-inside-work-tree"])
        .await;
    git_status(&output)
}

fn git_status(output: &CmdOutput) -> ConnectionStatus {
    match output.status {
        Some(0) => ConnectionStatus::new("git", ConnectionState::Connected, "git answers"),
        Some(code) => ConnectionStatus::new(
            "git",
            ConnectionState::Failed,
            format!("git exited with code {code}"),
        ),
        None => ConnectionStatus::new(
            "git",
            ConnectionState::Missing,
            "git was not found on this computer",
        ),
    }
}

pub fn keychain(store: &dyn SecretStore) -> ConnectionStatus {
    if let Err(error) = store.set(KEYCHAIN_PROBE, PROBE_VALUE) {
        return probe_failure(&error);
    }
    let read = store.get(KEYCHAIN_PROBE);
    let _ = store.delete(KEYCHAIN_PROBE);
    match read {
        Ok(Some(value)) if value == PROBE_VALUE => {
            ConnectionStatus::new("keychain", ConnectionState::Connected, "The keychain works")
        }
        Ok(_) => ConnectionStatus::new(
            "keychain",
            ConnectionState::Failed,
            "The keychain did not return the probe value",
        ),
        Err(error) => probe_failure(&error),
    }
}

fn probe_failure(error: &SecretError) -> ConnectionStatus {
    let state = match error {
        SecretError::Unavailable => ConnectionState::Missing,
        SecretError::Failed(_) => ConnectionState::Failed,
    };
    ConnectionStatus::new("keychain", state, error.to_string())
}

pub async fn openrouter(secrets: &dyn SecretStore, base_url: &str) -> ConnectionStatus {
    let key = match secrets.get("openrouter") {
        Ok(Some(key)) => key,
        Ok(None) => {
            return ConnectionStatus::new(
                "OpenRouter",
                ConnectionState::Missing,
                "No OpenRouter key saved",
            );
        }
        Err(error) => {
            return ConnectionStatus::new("OpenRouter", ConnectionState::Failed, error.to_string());
        }
    };
    let client = match reqwest::Client::builder()
        .timeout(OPENROUTER_TIMEOUT)
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return ConnectionStatus::new(
                "OpenRouter",
                ConnectionState::Failed,
                format!("Cannot build the HTTP client: {error}"),
            );
        }
    };
    let url = format!("{}/api/v1/auth/key", base_url.trim_end_matches('/'));
    match client.get(url).bearer_auth(key).send().await {
        Ok(response) => openrouter_answer(response.status()),
        Err(error) => ConnectionStatus::new(
            "OpenRouter",
            ConnectionState::Failed,
            format!("OpenRouter did not answer: {error}"),
        ),
    }
}

fn openrouter_answer(status: reqwest::StatusCode) -> ConnectionStatus {
    if status.is_success() {
        return ConnectionStatus::new("OpenRouter", ConnectionState::Connected, "The key works");
    }
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return ConnectionStatus::new(
            "OpenRouter",
            ConnectionState::NeedsLogin,
            "OpenRouter rejected the key",
        );
    }
    ConnectionStatus::new(
        "OpenRouter",
        ConnectionState::Failed,
        format!("OpenRouter answered with status {status}"),
    )
}

pub fn mcp_file_server(binary: Option<&Path>) -> ConnectionStatus {
    match binary {
        None => ConnectionStatus::new(
            "MCP file server",
            ConnectionState::Missing,
            "No MCP file server configured",
        ),
        Some(path) if path.is_file() => ConnectionStatus::new(
            "MCP file server",
            ConnectionState::Connected,
            "The MCP file server is installed",
        ),
        Some(_) => ConnectionStatus::new(
            "MCP file server",
            ConnectionState::Missing,
            "The MCP file server is not installed",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::BoxFuture;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::cmd::CmdOutput;
    use crate::secrets::MemoryStore;

    struct FakeRunner {
        status: Option<i32>,
    }

    impl FakeRunner {
        fn new(status: Option<i32>) -> Self {
            Self { status }
        }
    }

    impl CommandRunner for FakeRunner {
        fn run<'a>(&'a self, _program: &'a str, _args: &'a [&'a str]) -> BoxFuture<'a, CmdOutput> {
            Box::pin(async move {
                CmdOutput {
                    status: self.status,
                    stdout: String::new(),
                    stderr: String::new(),
                }
            })
        }
    }

    #[tokio::test]
    async fn git_is_connected_when_the_runner_succeeds() {
        let runner = FakeRunner::new(Some(0));
        assert_eq!(git(&runner).await.state, ConnectionState::Connected);
    }

    #[tokio::test]
    async fn git_is_missing_when_the_program_is_absent() {
        let runner = FakeRunner::new(None);
        assert_eq!(git(&runner).await.state, ConnectionState::Missing);
    }

    #[test]
    fn keychain_probe_passes_on_a_working_store() {
        let store = MemoryStore::new();
        let status = keychain(&store);
        assert_eq!(status.state, ConnectionState::Connected);
        assert_eq!(store.get(KEYCHAIN_PROBE).unwrap(), None);
    }

    #[tokio::test]
    async fn openrouter_without_a_key_is_missing() {
        let status = openrouter(&MemoryStore::new(), "http://127.0.0.1:1").await;
        assert_eq!(status.state, ConnectionState::Missing);
    }

    #[tokio::test]
    async fn openrouter_accepts_a_working_key() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/auth/key"))
            .and(header("authorization", "Bearer good-key"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let store = MemoryStore::new();
        store.set("openrouter", "good-key").unwrap();
        let status = openrouter(&store, &server.uri()).await;
        assert_eq!(status.state, ConnectionState::Connected);
    }

    #[tokio::test]
    async fn openrouter_rejects_a_bad_key_without_showing_it() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/auth/key"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let store = MemoryStore::new();
        let key = "sk-very-secret-key-that-must-not-leak";
        store.set("openrouter", key).unwrap();
        let status = openrouter(&store, &server.uri()).await;
        assert_eq!(status.state, ConnectionState::NeedsLogin);
        assert!(!status.detail.contains(key));
    }

    #[test]
    fn mcp_file_server_is_connected_only_for_a_file() {
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("mcp-server");
        std::fs::write(&file, "binary").unwrap();
        assert_eq!(
            mcp_file_server(Some(&file)).state,
            ConnectionState::Connected
        );
        assert_eq!(
            mcp_file_server(Some(folder.path())).state,
            ConnectionState::Missing
        );
        assert_eq!(mcp_file_server(None).state, ConnectionState::Missing);
    }
}
