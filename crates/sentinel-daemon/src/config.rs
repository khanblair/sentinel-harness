//! `<repo>/.sentinel/config.toml`: settings and routing that onboarding writes.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use sentinel_core::{BrainMode, CoreError, Cost, PermissionMode, Routing, ToolId};
use serde::{Deserialize, Serialize};

const CONFIG_FILE: &str = "config.toml";
const CONFIG_DIR: &str = ".sentinel";
const TOP_LEVEL_KEYS: &[&str] = &[
    "repo_path",
    "backlog",
    "cap_usd",
    "permission_mode",
    "brain_mode",
    "routing",
    "enabled_tools",
    "models",
    "free_models",
    "test_command",
    "hackathon_mode",
    "dev_loopback",
    "max_parallel",
    "task_timeout_seconds",
    "decision_timeout_seconds",
    "approval_timeout_seconds",
    "telegram",
];
const ROUTING_KEYS: &[&str] = &["cheap", "mid", "smart"];
const TELEGRAM_KEYS: &[&str] = &["enabled", "detail"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub repo_path: Option<String>,
    pub backlog: String,
    pub cap_usd: String,
    pub permission_mode: PermissionMode,
    pub brain_mode: BrainMode,
    pub routing: Routing,
    pub enabled_tools: Vec<ToolId>,
    /// The model each tool runs with, by tool id. A tool with no entry uses its profile default.
    pub models: std::collections::BTreeMap<String, String>,
    /// Model ids that cost nothing, such as a local model. A model whose name ends in `-free` or
    /// `:free` counts as free without being listed.
    pub free_models: Vec<String>,
    pub test_command: Vec<String>,
    pub hackathon_mode: bool,
    pub dev_loopback: bool,
    pub max_parallel: u32,
    pub task_timeout_seconds: u64,
    /// A contradiction nobody answers in this time skips both tasks.
    pub decision_timeout_seconds: u64,
    /// An approval card nobody answers in this time is denied.
    pub approval_timeout_seconds: u64,
    pub telegram: TelegramConfig,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TelegramConfig {
    pub enabled: bool,
    pub detail: String,
}

impl Config {
    /// True when the model a tool runs costs nothing: listed in `free_models`, or free by name.
    pub fn is_free_model(&self, tool: &ToolId, profile_default: &str) -> bool {
        let model = self
            .models
            .get(tool.as_str())
            .map_or(profile_default, String::as_str);
        self.free_models.iter().any(|free| free == model)
            || model.ends_with("-free")
            || model.ends_with(":free")
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            repo_path: None,
            backlog: "backlog.md".to_owned(),
            cap_usd: "2.00".to_owned(),
            permission_mode: PermissionMode::default(),
            brain_mode: BrainMode::ReadOnly,
            routing: Routing::default(),
            enabled_tools: Vec::new(),
            models: std::collections::BTreeMap::new(),
            free_models: Vec::new(),
            test_command: vec!["cargo".to_owned(), "test".to_owned()],
            hackathon_mode: false,
            dev_loopback: false,
            max_parallel: 3,
            task_timeout_seconds: 900,
            decision_timeout_seconds: 120,
            approval_timeout_seconds: 300,
            telegram: TelegramConfig::default(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("unknown config key: {0}")]
    UnknownKey(String),
    #[error("cannot read config: {0}")]
    Io(#[from] std::io::Error),
    #[error("cannot parse config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("cannot write config: {0}")]
    Write(#[from] toml::ser::Error),
    #[error("bad money amount: {0}")]
    Money(#[from] CoreError),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

impl Config {
    pub fn cap(&self) -> Result<Cost> {
        Cost::parse_usd(&self.cap_usd).map_err(ConfigError::Money)
    }
}

pub fn config_path(repo_root: &Path) -> PathBuf {
    repo_root.join(CONFIG_DIR).join(CONFIG_FILE)
}

pub fn load(path: &Path) -> Result<Config> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Config::default()),
        Err(error) => return Err(ConfigError::Io(error)),
    };
    let table: toml::Table = text.parse()?;
    reject_unknown_keys(&table)?;
    toml::from_str(&text).map_err(ConfigError::Parse)
}

pub fn save(path: &Path, config: &Config) -> Result<()> {
    let text = toml::to_string_pretty(config)?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let temp = temporary_path(path);
    let written = write_and_sync(&temp, &text);
    if let Err(error) = written {
        let _ = fs::remove_file(&temp);
        return Err(ConfigError::Io(error));
    }
    fs::rename(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        ConfigError::Io(error)
    })
}

fn write_and_sync(path: &Path, text: &str) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

fn temporary_path(path: &Path) -> PathBuf {
    let name = path.file_name().map_or_else(
        || CONFIG_FILE.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    path.with_file_name(format!(".{name}.tmp"))
}

fn reject_unknown_keys(table: &toml::Table) -> Result<()> {
    reject_keys(table, TOP_LEVEL_KEYS, "")?;
    if let Some(toml::Value::Table(routing)) = table.get("routing") {
        reject_keys(routing, ROUTING_KEYS, "routing.")?;
    }
    if let Some(toml::Value::Table(telegram)) = table.get("telegram") {
        reject_keys(telegram, TELEGRAM_KEYS, "telegram.")?;
    }
    Ok(())
}

fn reject_keys(table: &toml::Table, allowed: &[&str], prefix: &str) -> Result<()> {
    for key in table.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(ConfigError::UnknownKey(format!("{prefix}{key}")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_matches_the_documented_values() {
        let config = Config::default();
        assert_eq!(config.backlog, "backlog.md");
        assert_eq!(config.cap_usd, "2.00");
        assert_eq!(config.max_parallel, 3);
        assert_eq!(config.task_timeout_seconds, 900);
        assert_eq!(config.test_command, vec!["cargo", "test"]);
        assert_eq!(config.permission_mode, PermissionMode::AutoInsideReserved);
        assert_eq!(config.brain_mode, BrainMode::ReadOnly);
        assert!(!config.hackathon_mode);
        assert!(!config.dev_loopback);
        assert!(!config.telegram.enabled);
    }

    #[test]
    fn load_of_a_missing_file_gives_defaults() {
        let folder = tempfile::tempdir().unwrap();
        let config = load(&config_path(folder.path())).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn save_then_load_round_trips_every_field() {
        let folder = tempfile::tempdir().unwrap();
        let path = config_path(folder.path());
        let config = Config {
            repo_path: Some("/work/repo".to_owned()),
            backlog: "tasks.md".to_owned(),
            cap_usd: "0.50".to_owned(),
            permission_mode: PermissionMode::PlanOnly,
            brain_mode: BrainMode::Learn,
            routing: Routing {
                cheap: Some(ToolId::new("opencode")),
                mid: None,
                smart: Some(ToolId::new("claude")),
            },
            enabled_tools: vec![ToolId::new("claude")],
            models: std::collections::BTreeMap::from([("claude".to_owned(), "sonnet".to_owned())]),
            free_models: vec!["ornith/ornith".to_owned()],
            test_command: vec!["cargo".to_owned(), "nextest".to_owned()],
            hackathon_mode: true,
            dev_loopback: true,
            max_parallel: 2,
            task_timeout_seconds: 300,
            decision_timeout_seconds: 60,
            approval_timeout_seconds: 90,
            telegram: TelegramConfig {
                enabled: true,
                detail: "Status".to_owned(),
            },
        };
        save(&path, &config).unwrap();
        assert_eq!(load(&path).unwrap(), config);
    }

    #[test]
    fn unknown_key_error_names_the_key() {
        let folder = tempfile::tempdir().unwrap();
        let path = config_path(folder.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "cap_usd = \"1.00\"\nnot_a_key = true\n").unwrap();
        let error = load(&path).unwrap_err();
        assert!(error.to_string().contains("not_a_key"), "{error}");
    }

    #[test]
    fn atomic_save_leaves_no_temp_file() {
        let folder = tempfile::tempdir().unwrap();
        let path = config_path(folder.path());
        save(&path, &Config::default()).unwrap();
        let names: Vec<String> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec![CONFIG_FILE.to_owned()]);
    }

    #[test]
    fn cap_parses_the_text_and_rejects_junk() {
        let mut config = Config::default();
        assert_eq!(config.cap().unwrap().micro_usd(), 2_000_000);
        config.cap_usd = "abc".to_owned();
        assert!(config.cap().is_err());
    }

    #[test]
    fn unknown_nested_key_error_names_the_section() {
        let folder = tempfile::tempdir().unwrap();
        let path = config_path(folder.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "[telegram]\nenabled = true\nwrong = 1\n").unwrap();
        let error = load(&path).unwrap_err();
        assert!(error.to_string().contains("telegram.wrong"), "{error}");
    }
}

#[cfg(test)]
mod free_model_tests {
    use super::*;

    #[test]
    fn a_free_name_or_a_listed_model_costs_nothing() {
        let mut config = Config::default();
        let tool = ToolId::new("opencode");
        assert!(config.is_free_model(&tool, "opencode/glm-5.3-flash-free"));
        assert!(config.is_free_model(&tool, "meta/llama:free"));
        assert!(!config.is_free_model(&tool, "claude-sonnet"));
        config
            .models
            .insert("opencode".to_owned(), "ornith/ornith".to_owned());
        assert!(!config.is_free_model(&tool, "claude-sonnet"));
        config.free_models.push("ornith/ornith".to_owned());
        assert!(config.is_free_model(&tool, "claude-sonnet"));
    }
}
