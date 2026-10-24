//! Adapter profiles: data files that describe one agent CLI.
//!
//! Adding a tool is a new file in `profiles/`. No Rust code changes.

use std::collections::BTreeMap;
use std::path::Path;

use sentinel_core::Tier;
use serde::Deserialize;
use thiserror::Error;

use crate::worker::{CommandLine, RunSpec};

/// The id of the template profile. Only it may leave `binary` empty.
pub const GENERIC_ID: &str = "generic";

const PROMPT_PLACEHOLDER: &str = "{prompt}";
const MODEL_PLACEHOLDER: &str = "{model}";
const WORKDIR_PLACEHOLDER: &str = "{workdir}";
const TASK_ENV: &str = "SENTINEL_TASK_ID";
const AGENT_ENV: &str = "SENTINEL_AGENT";

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("profile file is not valid toml: {0}")]
    InvalidToml(#[from] toml::de::Error),
    #[error("profile id must not be empty")]
    EmptyId,
    #[error("profile {0}: binary must not be empty, only the generic template leaves it out")]
    EmptyBinary(String),
    #[error("profile {0}: prompt_via arg needs {{prompt}} in args exactly once, stdin needs none")]
    PromptPlaceholder(String),
    #[error("profile {0}: the generic template needs needs_template = true")]
    GenericNeedsTemplate(String),
}

/// Where the prompt goes. `Arg` puts it in the command line, `Stdin` writes it to stdin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptVia {
    Arg,
    Stdin,
}

/// How detection checks that a tool is logged in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginKind {
    /// No check. The tool runs without an account.
    #[default]
    None,
    /// A folder exists, such as `~/.claude`.
    Dir,
    /// An environment variable is set, such as an API key.
    Env,
    /// A harmless command exits 0.
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LoginCheck {
    pub kind: LoginKind,
    pub value: String,
}

impl Default for LoginCheck {
    fn default() -> Self {
        Self {
            kind: LoginKind::None,
            value: String::new(),
        }
    }
}

/// One tool description, read from a file in `profiles/`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub binary: String,
    pub version_args: Vec<String>,
    pub tier: Tier,
    pub prompt_via: PromptVia,
    pub args: Vec<String>,
    #[serde(default)]
    pub default_model: String,
    pub verified: bool,
    pub needs_template: bool,
    #[serde(default)]
    pub cost_prefix: String,
    #[serde(default)]
    pub login: LoginCheck,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl Profile {
    /// Read one profile from its toml text and check the rules the adapter relies on.
    pub fn parse(text: &str) -> Result<Profile, ProfileError> {
        let profile: Profile = toml::from_str(text)?;
        profile.check()?;
        Ok(profile)
    }

    /// Build the program to start for one task. `binary` is the path detection found.
    pub fn command_line(&self, spec: &RunSpec, binary: &Path) -> CommandLine {
        let model = if spec.model.is_empty() {
            self.default_model.as_str()
        } else {
            spec.model.as_str()
        };
        let args = drop_empty_model(&self.args, model, spec);
        let stdin = match self.prompt_via {
            PromptVia::Arg => None,
            PromptVia::Stdin => Some(spec.prompt.clone()),
        };
        CommandLine {
            program: binary.to_string_lossy().into_owned(),
            args,
            env: self.environment(spec),
            stdin,
            workdir: spec.workdir.clone(),
        }
    }

    fn check(&self) -> Result<(), ProfileError> {
        if self.id.is_empty() {
            return Err(ProfileError::EmptyId);
        }
        self.check_binary()?;
        self.check_prompt_placeholder()?;
        Ok(())
    }

    fn check_binary(&self) -> Result<(), ProfileError> {
        if !self.binary.is_empty() {
            return Ok(());
        }
        match (self.id == GENERIC_ID, self.needs_template) {
            (true, true) => Ok(()),
            (true, false) => Err(ProfileError::GenericNeedsTemplate(self.id.clone())),
            (false, _) => Err(ProfileError::EmptyBinary(self.id.clone())),
        }
    }

    fn check_prompt_placeholder(&self) -> Result<(), ProfileError> {
        let placed = self
            .args
            .iter()
            .filter(|arg| arg.contains(PROMPT_PLACEHOLDER))
            .count();
        let wanted = match self.prompt_via {
            PromptVia::Arg => 1,
            PromptVia::Stdin => 0,
        };
        if placed == wanted {
            Ok(())
        } else {
            Err(ProfileError::PromptPlaceholder(self.id.clone()))
        }
    }

    fn environment(&self, spec: &RunSpec) -> Vec<(String, String)> {
        let mut env: Vec<(String, String)> = self
            .env
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        // Fake agents and guard hooks read these to know who they are.
        env.push((TASK_ENV.to_string(), spec.task.to_string()));
        env.push((AGENT_ENV.to_string(), spec.agent.to_string()));
        env
    }
}

/// Fill the placeholders. With no model at all, an argument that is only `{model}` goes away with
/// the flag before it, so the tool picks its own default instead of getting an empty `--model`.
fn drop_empty_model(args: &[String], model: &str, spec: &RunSpec) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(args.len());
    for arg in args {
        if model.is_empty() && arg == MODEL_PLACEHOLDER {
            if out.last().is_some_and(|last| last.starts_with('-')) {
                out.pop();
            }
            continue;
        }
        out.push(fill_placeholders(arg, model, spec));
    }
    out
}

fn fill_placeholders(arg: &str, model: &str, spec: &RunSpec) -> String {
    arg.replace(MODEL_PLACEHOLDER, model)
        .replace(PROMPT_PLACEHOLDER, &spec.prompt)
        .replace(WORKDIR_PLACEHOLDER, &spec.workdir.to_string_lossy())
}

/// Every built-in profile, in file order.
pub fn builtin_profiles() -> Result<Vec<Profile>, ProfileError> {
    BUILTIN_FILES
        .iter()
        .map(|(_, text)| Profile::parse(text))
        .collect()
}

const BUILTIN_FILES: [(&str, &str); 10] = [
    ("opencode.toml", include_str!("../profiles/opencode.toml")),
    ("goose.toml", include_str!("../profiles/goose.toml")),
    ("pi.toml", include_str!("../profiles/pi.toml")),
    ("claude.toml", include_str!("../profiles/claude.toml")),
    ("codex.toml", include_str!("../profiles/codex.toml")),
    ("gemini.toml", include_str!("../profiles/gemini.toml")),
    ("agy.toml", include_str!("../profiles/agy.toml")),
    ("deepseek.toml", include_str!("../profiles/deepseek.toml")),
    ("mcode.toml", include_str!("../profiles/mcode.toml")),
    ("generic.toml", include_str!("../profiles/generic.toml")),
];

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::time::Duration;

    use sentinel_core::{TaskId, ToolId};

    use super::*;

    fn run_spec(model: &str) -> RunSpec {
        RunSpec {
            scope: Vec::new(),
            task: TaskId::new("T-07"),
            agent: ToolId::new("opencode"),
            model: model.to_string(),
            prompt: "fix the test".to_string(),
            workdir: PathBuf::from("/tmp/work"),
            timeout: Duration::from_secs(60),
        }
    }

    fn profile_text(extra: &str) -> String {
        format!(
            "id = \"fake\"\nname = \"Fake\"\n\
             version_args = [\"--version\"]\n\
             tier = \"cheap\"\nprompt_via = \"arg\"\n\
             verified = false\nneeds_template = false\n{extra}\n"
        )
    }

    #[test]
    fn builtin_profiles_parse_with_unique_ids() {
        let profiles = builtin_profiles().unwrap();
        assert_eq!(profiles.len(), 10);
        let ids: HashSet<&str> = profiles.iter().map(|profile| profile.id.as_str()).collect();
        assert_eq!(ids.len(), 10);
    }

    #[test]
    fn the_verified_profiles_are_opencode_goose_and_pi() {
        let profiles = builtin_profiles().unwrap();
        let verified: Vec<&str> = profiles
            .iter()
            .filter(|profile| profile.verified)
            .map(|profile| profile.id.as_str())
            .collect();
        assert_eq!(verified, ["opencode", "goose", "pi"]);
    }

    #[test]
    fn command_line_fills_every_placeholder() {
        let profile = Profile::parse(&profile_text(
            "binary = \"fake\"\nargs = [\"run\", \"{model}\", \"{prompt}\", \"{workdir}\"]",
        ))
        .unwrap();
        let command = profile.command_line(&run_spec("mid-model"), Path::new("/bin/fake"));
        assert_eq!(command.program, "/bin/fake");
        assert_eq!(
            command.args,
            ["run", "mid-model", "fix the test", "/tmp/work"]
        );
        assert_eq!(command.stdin, None);
        assert_eq!(command.workdir, PathBuf::from("/tmp/work"));
    }

    #[test]
    fn command_line_uses_the_default_model_when_the_spec_model_is_empty() {
        let profile = Profile::parse(&profile_text(
            "binary = \"fake\"\ndefault_model = \"fake/default-1\"\nargs = [\"{model}\", \"{prompt}\"]",
        ))
        .unwrap();
        let command = profile.command_line(&run_spec(""), Path::new("/bin/fake"));
        assert_eq!(command.args, ["fake/default-1", "fix the test"]);
    }

    #[test]
    fn an_empty_model_drops_its_flag_and_keeps_the_rest() {
        let profile = Profile::parse(&profile_text(
            "binary = \"fake\"\nargs = [\"-p\", \"{prompt}\", \"--model\", \"{model}\", \"--auto\"]",
        ))
        .unwrap();
        let none = profile.command_line(&run_spec(""), Path::new("/bin/fake"));
        assert_eq!(none.args, ["-p", "fix the test", "--auto"]);
        let some = profile.command_line(&run_spec("haiku"), Path::new("/bin/fake"));
        assert_eq!(
            some.args,
            ["-p", "fix the test", "--model", "haiku", "--auto"]
        );
    }

    #[test]
    fn command_line_appends_the_task_and_agent_environment() {
        let profile = Profile::parse(&profile_text(
            "binary = \"fake\"\nargs = [\"{prompt}\"]\nenv = { Z_KEY = \"z\" }",
        ))
        .unwrap();
        let command = profile.command_line(&run_spec("m"), Path::new("/bin/fake"));
        assert_eq!(
            command.env,
            [
                ("Z_KEY".to_string(), "z".to_string()),
                ("SENTINEL_TASK_ID".to_string(), "T-07".to_string()),
                ("SENTINEL_AGENT".to_string(), "opencode".to_string()),
            ]
        );
    }

    #[test]
    fn command_line_writes_the_prompt_to_stdin_for_stdin_profiles() {
        let profile = Profile::parse(
            "id = \"s\"\nname = \"S\"\nbinary = \"s\"\nversion_args = [\"--version\"]\n\
             tier = \"cheap\"\nprompt_via = \"stdin\"\nargs = [\"chat\"]\n\
             verified = false\nneeds_template = false\n",
        )
        .unwrap();
        let command = profile.command_line(&run_spec("m"), Path::new("/bin/s"));
        assert_eq!(command.stdin, Some("fix the test".to_string()));
        assert_eq!(command.args, ["chat"]);
    }

    #[test]
    fn arg_prompt_without_the_placeholder_is_rejected() {
        let profile = Profile::parse(&profile_text(
            "binary = \"fake\"\nargs = [\"run\", \"chat\"]",
        ));
        assert!(matches!(
            profile,
            Err(ProfileError::PromptPlaceholder(id)) if id == "fake"
        ));
    }

    #[test]
    fn stdin_prompt_may_not_leave_the_placeholder_in_args() {
        let profile = Profile::parse(
            "id = \"s\"\nname = \"S\"\nbinary = \"s\"\nversion_args = [\"--version\"]\n\
             tier = \"cheap\"\nprompt_via = \"stdin\"\nargs = [\"{prompt}\"]\n\
             verified = false\nneeds_template = false\n",
        );
        assert!(matches!(profile, Err(ProfileError::PromptPlaceholder(_))));
    }

    #[test]
    fn an_unknown_field_is_rejected() {
        let profile = Profile::parse(&profile_text("spped = \"fast\""));
        assert!(matches!(profile, Err(ProfileError::InvalidToml(_))));
    }

    #[test]
    fn empty_binary_is_rejected_outside_the_generic_template() {
        let profile = Profile::parse(&profile_text("binary = \"\"\nargs = [\"{prompt}\"]"));
        assert!(matches!(
            profile,
            Err(ProfileError::EmptyBinary(id)) if id == "fake"
        ));
    }

    #[test]
    fn the_generic_template_parses_with_an_empty_binary() {
        let profiles = builtin_profiles().unwrap();
        let generic = profiles
            .iter()
            .find(|profile| profile.id == GENERIC_ID)
            .unwrap();
        assert_eq!(generic.binary, "");
        assert!(generic.needs_template);
    }
}
