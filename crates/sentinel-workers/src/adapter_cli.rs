use std::path::PathBuf;

use futures::future::BoxFuture;

use crate::profile::{Profile, is_free_name};
use crate::supervisor::{CostSource, run_command};
use crate::worker::{RunControl, RunOutcome, RunSpec, Worker};

/// Where a run's spend comes from. Only some sources may end a run at the task ceiling.
///
/// Named apart from `supervisor::CostSource`, which is the struct the runner takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpendSource {
    /// The agent printed its own dollar figure.
    Reported,
    /// The agent printed token counts, priced from the price table.
    FromTokens,
    /// Only elapsed time is known.
    TimeGuess,
    /// The model is free. Spend stays zero.
    Free,
}

/// True when the task ceiling may stop a run with this source. A time guess never does: the task
/// timeout still ends a stuck agent, and a guess must not kill a run that may cost nothing.
#[must_use]
pub fn ceiling_applies(source: SpendSource) -> bool {
    match source {
        SpendSource::Reported | SpendSource::FromTokens => true,
        SpendSource::TimeGuess | SpendSource::Free => false,
    }
}

/// The model an agent names in one output line, if the line names one. Each adapter prints it in
/// its own format. Other adapters and other lines give `None`.
#[must_use]
pub fn model_from_line(adapter: &str, line: &str) -> Option<String> {
    match adapter {
        "goose" => goose_model(line),
        "opencode" => opencode_model(line),
        _ => None,
    }
}

/// Goose prints `new session · openrouter <model>` or `new session · <model>`.
fn goose_model(line: &str) -> Option<String> {
    if !line.contains("new session") {
        return None;
    }
    let (_, rest) = line
        .split_once("openrouter ")
        .or_else(|| line.split_once("new session · "))?;
    first_word(rest)
}

/// `OpenCode` prints `build · <model>` or `plan · <model>`.
fn opencode_model(line: &str) -> Option<String> {
    let (_, rest) = line
        .split_once("build · ")
        .or_else(|| line.split_once("plan · "))?;
    first_word(rest)
}

fn first_word(text: &str) -> Option<String> {
    text.split_whitespace().next().map(str::to_owned)
}

/// A CLI worker that runs a command through the supervisor.
pub struct CliWorker {
    profile: Profile,
    binary: PathBuf,
    free: bool,
    /// Variables added to the profile's own, such as a saved provider key.
    extra_env: Vec<(String, String)>,
}

impl CliWorker {
    pub fn new(profile: Profile, binary: PathBuf) -> Self {
        Self {
            profile,
            binary,
            free: false,
            extra_env: Vec::new(),
        }
    }

    /// Mark the model as free: running time adds no estimated cost. A run also counts as free when
    /// its own model name ends in `:free` or `-free`, even if this flag is off.
    #[must_use]
    pub fn with_free_cost(mut self, free: bool) -> Self {
        self.free = free;
        self
    }

    /// Add variables to the agent's environment. They come after the profile's own, so they win.
    #[must_use]
    pub fn with_env(mut self, env: Vec<(String, String)>) -> Self {
        self.extra_env = env;
        self
    }
}

impl Worker for CliWorker {
    fn run(&self, spec: RunSpec, ctrl: RunControl) -> BoxFuture<'_, RunOutcome> {
        let profile = self.profile.clone();
        let binary = self.binary.clone();
        let free = self.free || is_free_name(profile.model_for(&spec));
        let extra_env = self.extra_env.clone();

        Box::pin(async move {
            let mut command_line = profile.command_line(&spec, &binary);
            command_line.env.extend(extra_env);

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

    use crate::adapter_cli::{CliWorker, SpendSource, ceiling_applies, model_from_line};
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

    #[test]
    fn goose_names_the_openrouter_model_in_its_session_line() {
        let line = "new session · openrouter poolside/laguna-s-2.1:free";
        assert_eq!(
            model_from_line("goose", line).as_deref(),
            Some("poolside/laguna-s-2.1:free")
        );
    }

    #[test]
    fn goose_names_a_plain_model_in_its_session_line() {
        let line = "new session · claude-sonnet-5-5";
        assert_eq!(
            model_from_line("goose", line).as_deref(),
            Some("claude-sonnet-5-5")
        );
    }

    #[test]
    fn goose_ignores_an_openrouter_mention_outside_a_session_line() {
        assert_eq!(
            model_from_line("goose", "using openrouter for this call"),
            None
        );
    }

    #[test]
    fn opencode_names_the_build_model() {
        assert_eq!(
            model_from_line("opencode", "build · glm-5.3-flash-free").as_deref(),
            Some("glm-5.3-flash-free")
        );
    }

    #[test]
    fn opencode_names_the_plan_model() {
        assert_eq!(
            model_from_line("opencode", "plan · glm-5.3-flash-free  ").as_deref(),
            Some("glm-5.3-flash-free")
        );
    }

    #[test]
    fn a_line_from_another_adapter_names_no_model() {
        assert_eq!(
            model_from_line("claude", "build · glm-5.3-flash-free"),
            None
        );
        assert_eq!(
            model_from_line("opencode", "new session · openrouter x:free"),
            None
        );
    }

    #[test]
    fn a_time_guess_alone_never_ends_a_run_at_the_ceiling() {
        assert!(!ceiling_applies(SpendSource::TimeGuess));
        assert!(!ceiling_applies(SpendSource::Free));
    }

    #[test]
    fn reported_and_token_spend_can_end_a_run_at_the_ceiling() {
        assert!(ceiling_applies(SpendSource::Reported));
        assert!(ceiling_applies(SpendSource::FromTokens));
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
