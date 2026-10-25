//! A new project starts with the agent choices of the first project. The first scan of a fresh
//! project turns on every ready agent, so the choices made in the setup wizard would be lost. The
//! pure part works on plain values. The engine part writes the result into the new project.

use sentinel_core::{ToolId, ToolInfo};

use crate::agents_config::{fill_routing, without_missing_tools};
use crate::config::Config;
use crate::engine::{Engine, Result};

/// The config a new project starts with: the agent and cost choices of the primary project, cut
/// down to the tools that are ready on this computer. The test command and the Brain mode stay at
/// their defaults, because they belong to one repository.
pub(crate) fn inherited_config(primary: &Config, found: &[ToolInfo]) -> Config {
    let ready: Vec<ToolInfo> = found
        .iter()
        .filter(|tool| tool.status.is_ready())
        .cloned()
        .collect();
    let (enabled, routing) =
        without_missing_tools(&primary.enabled_tools, &primary.routing, &ready);
    let usable: Vec<&ToolInfo> = ready
        .iter()
        .filter(|tool| enabled.contains(&tool.id))
        .collect();
    Config {
        routing: fill_routing(routing, &usable),
        enabled_tools: enabled,
        models: primary.models.clone(),
        free_models: primary.free_models.clone(),
        cap_usd: primary.cap_usd.clone(),
        permission_mode: primary.permission_mode,
        max_parallel: primary.max_parallel,
        task_timeout_seconds: primary.task_timeout_seconds,
        decision_timeout_seconds: primary.decision_timeout_seconds,
        approval_timeout_seconds: primary.approval_timeout_seconds,
        ..Config::default()
    }
}

/// Writes the inherited choices into a new project. The router reads the stored tool flags, not the
/// config, so both are set. When no primary tool is left here, the project keeps what its own first
/// scan chose, so it never ends with no agent at all.
pub(crate) fn inherit_choices(engine: &Engine, primary: &Config) -> Result<()> {
    let tools = engine.store().list_tools()?;
    let inherited = inherited_config(primary, &tools);
    if inherited.enabled_tools.is_empty() {
        return Ok(());
    }
    for tool in &tools {
        let on = inherited.enabled_tools.contains(&tool.id);
        engine.store().set_tool_enabled(&tool.id, on)?;
    }
    engine.update_config(|config| *config = inherited)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sentinel_core::{BrainMode, PermissionMode, Routing, Tier, ToolStatus};

    use super::*;

    fn tool(id: &str, tier: Tier) -> ToolInfo {
        ToolInfo {
            id: ToolId::new(id),
            name: id.to_owned(),
            path: format!("/usr/bin/{id}"),
            version: "1.0".to_owned(),
            adapter: id.to_owned(),
            status: ToolStatus::Ready,
            tier,
            enabled: false,
        }
    }

    fn ids(list: &[&str]) -> Vec<ToolId> {
        list.iter().map(|id| ToolId::new(*id)).collect()
    }

    fn primary_with(enabled: &[&str], routing: Routing) -> Config {
        Config {
            enabled_tools: ids(enabled),
            routing,
            ..Config::default()
        }
    }

    #[test]
    fn a_tool_the_primary_turned_off_stays_off() {
        let primary = primary_with(&["a", "c"], Routing::default());
        let found = [
            tool("a", Tier::Cheap),
            tool("b", Tier::Mid),
            tool("c", Tier::Smart),
        ];
        let inherited = inherited_config(&primary, &found);
        assert_eq!(inherited.enabled_tools, ids(&["a", "c"]));
    }

    #[test]
    fn a_tool_missing_or_not_ready_here_is_dropped() {
        let primary = primary_with(&["a", "gemini", "b"], Routing::default());
        let mut login = tool("b", Tier::Mid);
        login.status = ToolStatus::NeedsLogin;
        let found = [tool("a", Tier::Cheap), login];
        let inherited = inherited_config(&primary, &found);
        assert_eq!(inherited.enabled_tools, ids(&["a"]));
    }

    #[test]
    fn a_routing_tier_on_a_dropped_tool_is_refilled_from_the_enabled_tools() {
        let routing = Routing {
            cheap: Some(ToolId::new("gemini")),
            mid: Some(ToolId::new("b")),
            smart: Some(ToolId::new("c")),
        };
        let primary = primary_with(&["a", "b", "c"], routing);
        let found = [
            tool("a", Tier::Cheap),
            tool("b", Tier::Mid),
            tool("c", Tier::Smart),
        ];
        let inherited = inherited_config(&primary, &found);
        assert_eq!(inherited.routing.cheap, Some(ToolId::new("a")));
        assert_eq!(inherited.routing.mid, Some(ToolId::new("b")));
        assert_eq!(inherited.routing.smart, Some(ToolId::new("c")));
    }

    #[test]
    fn a_refilled_tier_never_picks_a_tool_the_primary_turned_off() {
        let routing = Routing {
            cheap: Some(ToolId::new("gemini")),
            mid: None,
            smart: None,
        };
        let primary = primary_with(&["b"], routing);
        let found = [tool("a", Tier::Cheap), tool("b", Tier::Mid)];
        let inherited = inherited_config(&primary, &found);
        assert_eq!(inherited.routing.cheap, Some(ToolId::new("b")));
    }

    #[test]
    fn the_test_command_and_brain_mode_are_not_copied() {
        let primary = Config {
            test_command: vec!["make".to_owned(), "test".to_owned()],
            brain_mode: BrainMode::ReadOnly,
            ..primary_with(&["a"], Routing::default())
        };
        let inherited = inherited_config(&primary, &[tool("a", Tier::Cheap)]);
        assert_eq!(inherited.test_command, Config::default().test_command);
        assert_eq!(inherited.brain_mode, Config::default().brain_mode);
    }

    #[test]
    fn cost_permission_timeouts_and_models_are_copied() {
        let mut primary = primary_with(&["a"], Routing::default());
        primary.cap_usd = "0.50".to_owned();
        primary.permission_mode = PermissionMode::PlanOnly;
        primary.max_parallel = 2;
        primary.task_timeout_seconds = 300;
        primary.decision_timeout_seconds = 60;
        primary.approval_timeout_seconds = 90;
        primary.models.insert("a".to_owned(), "sonnet".to_owned());
        primary.free_models.push("ornith/ornith".to_owned());
        let inherited = inherited_config(&primary, &[tool("a", Tier::Cheap)]);
        assert_eq!(inherited.cap_usd, "0.50");
        assert_eq!(inherited.permission_mode, PermissionMode::PlanOnly);
        assert_eq!(inherited.max_parallel, 2);
        assert_eq!(inherited.task_timeout_seconds, 300);
        assert_eq!(inherited.decision_timeout_seconds, 60);
        assert_eq!(inherited.approval_timeout_seconds, 90);
        assert_eq!(inherited.models, primary.models);
        assert_eq!(inherited.free_models, primary.free_models);
    }

    #[test]
    fn a_primary_with_no_enabled_tool_gives_none_to_inherit() {
        let primary = primary_with(&[], Routing::default());
        let inherited = inherited_config(&primary, &[tool("a", Tier::Cheap)]);
        assert!(inherited.enabled_tools.is_empty());
        assert_eq!(inherited.routing, Routing::default());
    }
}
