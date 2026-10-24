use std::path::PathBuf;

use futures::future::BoxFuture;

use crate::profile::Profile;
use crate::supervisor::{CostSource, run_command};
use crate::worker::{RunControl, RunOutcome, RunSpec, Worker};

/// A CLI worker that runs a command through the supervisor.
pub struct CliWorker {
    profile: Profile,
    binary: PathBuf,
    free: bool,
}

impl CliWorker {
    pub fn new(profile: Profile, binary: PathBuf) -> Self {
        Self {
            profile,
            binary,
            free: false,
        }
    }

    /// Mark the model as free: running time adds no estimated cost.
    #[must_use]
    pub fn with_free_cost(mut self, free: bool) -> Self {
        self.free = free;
        self
    }
}

impl Worker for CliWorker {
    fn run(&self, spec: RunSpec, ctrl: RunControl) -> BoxFuture<'_, RunOutcome> {
        let profile = self.profile.clone();
        let binary = self.binary.clone();
        let free = self.free;

        Box::pin(async move {
            let command_line = profile.command_line(&spec, &binary);

            let reported_prefix = if profile.cost_prefix.is_empty() {
                None
            } else {
                Some(profile.cost_prefix.clone())
            };

            let source = CostSource {
                tier: profile.tier,
                reported_prefix,
                free,
            };

            run_command(command_line, &spec, ctrl, &source).await
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::time::Duration;

    use sentinel_core::{TaskId, Tier, ToolId};

    use crate::adapter_cli::CliWorker;
    use crate::budget::Budget;
    use crate::profile::{LoginCheck, Profile, PromptVia};
    use crate::worker::{RunControl, RunSpec, Worker};
    use tokio::sync::{mpsc, watch};

    fn make_worker(args: &[&str]) -> CliWorker {
        let profile = Profile {
            id: "test".to_string(),
            name: "Test".to_string(),
            binary: "test".to_string(),
            version_args: vec!["--version".to_string()],
            tier: Tier::Cheap,
            prompt_via: PromptVia::Arg,
            args: args.iter().map(ToString::to_string).collect(),
            default_model: "test/model".to_string(),
            verified: false,
            needs_template: false,
            cost_prefix: "cost:".to_string(),
            login: LoginCheck::default(),
            env: BTreeMap::default(),
        };
        CliWorker::new(profile, PathBuf::from("/bin/test"))
    }

    fn make_spec(model: &str) -> RunSpec {
        RunSpec {
            scope: Vec::new(),
            task: TaskId::new("T-01"),
            agent: ToolId::new("test"),
            model: model.to_string(),
            prompt: "test prompt".to_string(),
            workdir: PathBuf::from("/tmp/work"),
            timeout: Duration::from_secs(60),
        }
    }

    fn make_ctrl() -> RunControl {
        let (budget, _kill) = Budget::new(sentinel_core::Cost::from_micro_usd(1_000_000, true));
        let (tx, _rx) = mpsc::unbounded_channel();
        let (_kill_tx, kill_rx) = watch::channel(false);
        RunControl {
            kill: kill_rx,
            lines: tx,
            budget,
        }
    }

    #[tokio::test]
    async fn cli_worker_runs_and_returns_failed() {
        let worker = make_worker(&["run", "{model}", "{prompt}"]);
        let spec = make_spec("custom-model");
        let ctrl = make_ctrl();

        let outcome = worker.run(spec, ctrl).await;

        assert_eq!(outcome.end, crate::worker::EndKind::Failed);
    }

    #[tokio::test]
    async fn cli_worker_uses_default_model_when_spec_empty() {
        let worker = make_worker(&["run", "{model}", "{prompt}"]);
        let spec = make_spec("");
        let ctrl = make_ctrl();

        let outcome = worker.run(spec, ctrl).await;

        assert_eq!(outcome.end, crate::worker::EndKind::Failed);
    }
}
