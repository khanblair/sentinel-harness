//! The agents of a run, chosen with `sentinel run --agents`. A roster lists the agents from the
//! cheapest to the strongest. A tool may appear more than once with different models: each entry
//! is then its own agent, with its own id, so the run can use two models of one tool.

use std::collections::{HashMap, HashSet};

use sentinel_core::{ToolId, ToolInfo, ToolStatus};
use sentinel_workers::profile::{Profile, builtin_profiles};

use crate::config::Config;
use crate::engine::{Engine, EngineError, Result, lock};
use crate::openrouter_tool::is_openrouter_adapter;

/// The agent count the hackathon asks for. A shorter roster still runs; the CLI says so.
pub const HACKATHON_AGENTS: usize = 3;

/// One entry of the roster as the owner typed it: `TOOL` or `TOOL=MODEL`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterSpec {
    pub tool: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RosterError {
    #[error("the agent list is empty")]
    Empty,
    #[error("an agent has no tool name")]
    EmptyTool,
    #[error("{0} has an empty model after the =")]
    EmptyModel(String),
    #[error("{0} is listed twice")]
    Duplicate(String),
    #[error("{0} is not found on this computer")]
    NotFound(String),
    #[error("{tool} is not ready: {hint}")]
    NotReady { tool: String, hint: &'static str },
}

/// One agent of a run: the tool it runs as (its id and name), and the model it runs with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RosterAgent {
    pub info: ToolInfo,
    pub model: Option<String>,
}

/// Read the text of `--agents`. A blank list, an empty tool, an empty model, or a repeated entry
/// is an error.
pub fn parse_roster(text: &str) -> std::result::Result<Vec<RosterSpec>, RosterError> {
    if text.trim().is_empty() {
        return Err(RosterError::Empty);
    }
    let mut specs: Vec<RosterSpec> = Vec::new();
    for part in text.split(',') {
        let spec = parse_spec(part.trim())?;
        if specs.contains(&spec) {
            return Err(RosterError::Duplicate(label(&spec)));
        }
        specs.push(spec);
    }
    Ok(specs)
}

fn parse_spec(text: &str) -> std::result::Result<RosterSpec, RosterError> {
    let (tool, model) = match text.split_once('=') {
        Some((tool, model)) => (tool.trim(), Some(model.trim())),
        None => (text, None),
    };
    if tool.is_empty() {
        return Err(RosterError::EmptyTool);
    }
    let model = match model {
        Some("") => return Err(RosterError::EmptyModel(tool.to_owned())),
        Some(model) => Some(model.to_owned()),
        None => None,
    };
    Ok(RosterSpec {
        tool: tool.to_owned(),
        model,
    })
}

fn label(spec: &RosterSpec) -> String {
    match &spec.model {
        Some(model) => format!("{} ({model})", spec.tool),
        None => spec.tool.clone(),
    }
}

/// The agents of a roster, built from the tools this computer has. The first entry of a tool keeps
/// the tool id. Later entries of the same tool get `TOOL~2`, `TOOL~3`, and so on.
pub(crate) fn roster_agents(
    specs: &[RosterSpec],
    found: &[ToolInfo],
) -> std::result::Result<Vec<RosterAgent>, RosterError> {
    let mut copies: HashMap<&str, usize> = HashMap::new();
    let mut agents = Vec::with_capacity(specs.len());
    for spec in specs {
        let base = base_tool(spec, found)?;
        let copy = copies.entry(spec.tool.as_str()).or_insert(0);
        *copy += 1;
        agents.push(RosterAgent {
            info: virtual_info(base, spec, *copy),
            model: spec.model.clone(),
        });
    }
    Ok(agents)
}

/// The tool a roster entry copies. It must be on this computer and ready.
fn base_tool<'a>(
    spec: &RosterSpec,
    found: &'a [ToolInfo],
) -> std::result::Result<&'a ToolInfo, RosterError> {
    let tool = found
        .iter()
        .find(|tool| tool.id.as_str() == spec.tool)
        .ok_or_else(|| RosterError::NotFound(spec.tool.clone()))?;
    if tool.status.is_ready() {
        return Ok(tool);
    }
    Err(RosterError::NotReady {
        tool: spec.tool.clone(),
        hint: not_ready_hint(tool),
    })
}

/// A copy of the base tool under its roster id. The name of a later copy says its model.
fn virtual_info(base: &ToolInfo, spec: &RosterSpec, copy: usize) -> ToolInfo {
    let id = if copy == 1 {
        base.id.clone()
    } else {
        ToolId::new(format!("{}~{copy}", base.id))
    };
    let name = match &spec.model {
        Some(model) if copy > 1 => format!("{} ({model})", base.name),
        _ => base.name.clone(),
    };
    ToolInfo {
        id,
        name,
        enabled: true,
        ..base.clone()
    }
}

/// Why a tool cannot take work, in words for the owner.
fn not_ready_hint(tool: &ToolInfo) -> &'static str {
    match tool.status {
        ToolStatus::Ready => "it is ready",
        ToolStatus::NeedsLogin if is_openrouter_adapter(&tool.adapter) => "save a key in Settings",
        ToolStatus::NeedsLogin => "sign in first",
        ToolStatus::NeedsCommand => "confirm its command first",
        ToolStatus::Missing => "it is not installed",
        ToolStatus::Failed => "its last check failed",
    }
}

fn invalid(error: &RosterError) -> EngineError {
    EngineError::Invalid(error.to_string())
}

impl Engine {
    /// Make this roster the agents of the next run. Each entry is stored as a tool, every other tool
    /// is turned off, and the models go into the config. Returns the roster ids, cheapest first.
    pub fn apply_roster(&self, specs: &[RosterSpec]) -> Result<Vec<ToolId>> {
        if specs.is_empty() {
            return Err(invalid(&RosterError::Empty));
        }
        let found = self.store().list_tools()?;
        let agents = roster_agents(specs, &found).map_err(|error| invalid(&error))?;
        for agent in &agents {
            self.store().upsert_tool(&agent.info)?;
        }
        let roster: Vec<ToolId> = agents.iter().map(|agent| agent.info.id.clone()).collect();
        for tool in self.store().list_tools()? {
            self.store()
                .set_tool_enabled(&tool.id, roster.contains(&tool.id))?;
        }
        self.update_config(|c| {
            c.enabled_tools.clone_from(&roster);
            for agent in &agents {
                if let Some(model) = &agent.model {
                    c.models
                        .insert(agent.info.id.as_str().to_owned(), model.clone());
                }
            }
        })?;
        lock(&self.inner.roster).clone_from(&roster);
        Ok(roster)
    }

    /// The agents that cannot take a turn now: not ready, or paused after a failure.
    pub(crate) fn skipped_agents(&self, roster: &[ToolId]) -> Result<HashSet<ToolId>> {
        let tools = self.store().list_tools()?;
        let paused = self.inner.health.paused_map();
        let not_ready = |id: &ToolId| {
            !tools
                .iter()
                .any(|tool| &tool.id == id && tool.status.is_ready())
        };
        Ok(roster
            .iter()
            .filter(|&id| not_ready(id) || paused.contains_key(id))
            .cloned()
            .collect())
    }

    /// One line per agent of the roster, for the terminal before a run starts.
    pub fn roster_lines(&self, roster: &[ToolId]) -> Result<Vec<String>> {
        let config = self.config();
        let tools = self.store().list_tools()?;
        Ok(roster
            .iter()
            .enumerate()
            .filter_map(|(index, id)| {
                let tool = tools.iter().find(|tool| &tool.id == id)?;
                Some(roster_line(index + 1, tool, &config))
            })
            .collect())
    }
}

fn roster_line(number: usize, tool: &ToolInfo, config: &Config) -> String {
    let label = match config
        .models
        .get(tool.id.as_str())
        .filter(|m| !m.is_empty())
    {
        Some(model) => format!("{} ({model})", tool.id),
        None => tool.id.to_string(),
    };
    format!(
        "agent {number}: {label}, spend: {}",
        spend_word(tool, config)
    )
}

/// How Sentinel knows what an agent spends: the agent reports it, the model is free, or Sentinel
/// guesses from the clock.
pub(crate) fn spend_word(tool: &ToolInfo, config: &Config) -> &'static str {
    let profile = builtin_profile(&tool.adapter);
    let reports = is_openrouter_adapter(&tool.adapter)
        || profile.as_ref().is_some_and(|p| !p.cost_prefix.is_empty());
    if reports {
        return "reported";
    }
    let default_model = profile.as_ref().map_or("", |p| p.default_model.as_str());
    if config.is_free_model(&tool.id, default_model) {
        "free"
    } else {
        "time guess"
    }
}

fn builtin_profile(adapter: &str) -> Option<Profile> {
    builtin_profiles()
        .ok()?
        .into_iter()
        .find(|profile| profile.id == adapter)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(names: &[&str]) -> Vec<ToolId> {
        names.iter().map(|name| ToolId::new(*name)).collect()
    }

    fn tool(id: &str, adapter: &str, status: ToolStatus) -> ToolInfo {
        ToolInfo {
            id: ToolId::new(id),
            name: id.to_owned(),
            path: format!("/usr/bin/{id}"),
            version: "1.0".to_owned(),
            adapter: adapter.to_owned(),
            status,
            tier: sentinel_core::Tier::Mid,
            enabled: false,
        }
    }

    #[test]
    fn a_plain_list_keeps_the_order_and_has_no_models() {
        let specs = parse_roster("claude,codex").unwrap();
        assert_eq!(
            specs,
            [
                RosterSpec {
                    tool: "claude".to_owned(),
                    model: None
                },
                RosterSpec {
                    tool: "codex".to_owned(),
                    model: None
                },
            ]
        );
    }

    #[test]
    fn a_model_follows_the_equals_sign() {
        let specs = parse_roster("claude=haiku, openrouter=deepseek/deepseek-v3.2").unwrap();
        assert_eq!(specs[0].model.as_deref(), Some("haiku"));
        assert_eq!(specs[1].tool, "openrouter");
        assert_eq!(specs[1].model.as_deref(), Some("deepseek/deepseek-v3.2"));
    }

    #[test]
    fn the_same_tool_with_two_models_is_two_entries() {
        let specs = parse_roster("openrouter=deepseek/deepseek-v3.2,openrouter=qwen/qwen3-coder");
        assert_eq!(specs.map(|s| s.len()), Ok(2));
    }

    #[test]
    fn a_repeated_entry_is_refused_by_name() {
        assert_eq!(
            parse_roster("claude=haiku,claude=haiku"),
            Err(RosterError::Duplicate("claude (haiku)".to_owned()))
        );
    }

    #[test]
    fn a_blank_list_is_refused() {
        assert_eq!(parse_roster("  "), Err(RosterError::Empty));
    }

    #[test]
    fn an_empty_tool_is_refused() {
        assert_eq!(parse_roster("claude,,codex"), Err(RosterError::EmptyTool));
        assert_eq!(parse_roster("=haiku"), Err(RosterError::EmptyTool));
    }

    #[test]
    fn an_empty_model_is_refused() {
        assert_eq!(
            parse_roster("claude="),
            Err(RosterError::EmptyModel("claude".to_owned()))
        );
    }

    #[test]
    fn a_later_entry_of_a_tool_becomes_a_virtual_agent_with_its_model() {
        let found = [tool("openrouter", "openrouter-loop", ToolStatus::Ready)];
        let specs =
            parse_roster("openrouter=deepseek/deepseek-v3.2,openrouter=qwen/qwen3-coder").unwrap();
        let agents = roster_agents(&specs, &found).unwrap();
        assert_eq!(agents[0].info.id, ToolId::new("openrouter"));
        assert_eq!(agents[1].info.id, ToolId::new("openrouter~2"));
        assert_eq!(agents[1].info.name, "openrouter (qwen/qwen3-coder)");
        assert_eq!(agents[1].info.adapter, "openrouter-loop");
        assert_eq!(agents[1].model.as_deref(), Some("qwen/qwen3-coder"));
        assert!(agents.iter().all(|agent| agent.info.enabled));
    }

    #[test]
    fn a_missing_tool_is_named_in_the_error() {
        let specs = parse_roster("claude").unwrap();
        assert_eq!(
            roster_agents(&specs, &[]),
            Err(RosterError::NotFound("claude".to_owned()))
        );
    }

    #[test]
    fn a_tool_that_is_not_ready_says_how_to_fix_it() {
        let found = [tool(
            "openrouter",
            "openrouter-loop",
            ToolStatus::NeedsLogin,
        )];
        let specs = parse_roster("openrouter").unwrap();
        let error = roster_agents(&specs, &found).unwrap_err();
        assert_eq!(
            error.to_string(),
            "openrouter is not ready: save a key in Settings"
        );
    }

    #[test]
    fn the_roster_ids_come_back_in_roster_order() {
        let found = [
            tool("claude", "claude", ToolStatus::Ready),
            tool("codex", "codex", ToolStatus::Ready),
        ];
        let specs = parse_roster("codex,claude=haiku").unwrap();
        let agents = roster_agents(&specs, &found).unwrap();
        let order: Vec<ToolId> = agents.into_iter().map(|a| a.info.id).collect();
        assert_eq!(order, ids(&["codex", "claude"]));
    }

    #[test]
    fn spend_is_reported_for_claude_and_openrouter() {
        let config = Config::default();
        let claude = tool("claude", "claude", ToolStatus::Ready);
        let openrouter = tool("openrouter", "openrouter-loop", ToolStatus::Ready);
        assert_eq!(spend_word(&claude, &config), "reported");
        assert_eq!(spend_word(&openrouter, &config), "reported");
    }

    #[test]
    fn spend_is_free_for_a_free_model_of_a_profile_without_a_cost_line() {
        let mut config = Config::default();
        config
            .models
            .insert("agy".to_owned(), "vendor/model:free".to_owned());
        let agy = tool("agy", "agy", ToolStatus::Ready);
        assert_eq!(spend_word(&agy, &config), "free");
    }

    #[test]
    fn spend_is_a_time_guess_for_a_profile_without_a_cost_line() {
        let agy = tool("agy", "agy", ToolStatus::Ready);
        assert_eq!(spend_word(&agy, &Config::default()), "time guess");
    }
}
