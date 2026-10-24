//! Agent routing and escalation.

use std::collections::HashMap;

use sentinel_core::{Tier, ToolId, ToolInfo};

/// Router picks the right tool for a tier and handles escalation.
#[derive(Debug, Clone)]
pub struct Router {
    /// Mapping from tier to preferred tool id.
    routing: HashMap<Tier, ToolId>,
    /// All known tools, indexed by id for quick lookup.
    tools: HashMap<ToolId, ToolInfo>,
    /// Tools grouped by tier for fallback.
    tools_by_tier: HashMap<Tier, Vec<ToolId>>,
    /// Tools grouped by adapter for the `has_enough_tools` check.
    tools_by_adapter: HashMap<String, Vec<ToolId>>,
}

impl Router {
    /// Create a new router with the routing map and tool list.
    pub fn new(routing: HashMap<Tier, ToolId>, tools: Vec<ToolInfo>) -> Self {
        let mut tools_by_tier: HashMap<Tier, Vec<ToolId>> = HashMap::new();
        let mut tools_by_adapter: HashMap<String, Vec<ToolId>> = HashMap::new();
        let mut tools_map = HashMap::new();

        for tool in tools {
            let id = tool.id.clone();
            tools_by_tier.entry(tool.tier).or_default().push(id.clone());
            tools_by_adapter
                .entry(tool.adapter.clone())
                .or_default()
                .push(id.clone());
            tools_map.insert(id, tool);
        }

        Self {
            routing,
            tools: tools_map,
            tools_by_tier,
            tools_by_adapter,
        }
    }

    /// Pick the tool for a tier:
    /// 1. The routed tool if enabled and ready.
    /// 2. Any enabled ready tool of the same tier.
    /// 3. Any enabled ready tool.
    ///
    /// Returns `None` when none is ready.
    pub fn pick(&self, tier: Tier) -> Option<ToolId> {
        // 1. Try the routed tool for this tier
        if let Some(routed_id) = self.routing.get(&tier)
            && let Some(tool) = self.tools.get(routed_id)
            && tool.enabled
            && tool.status.is_ready()
        {
            return Some(routed_id.clone());
        }

        // 2. Fall back to any enabled ready tool of the same tier
        if let Some(tier_tools) = self.tools_by_tier.get(&tier) {
            for id in tier_tools {
                if let Some(tool) = self.tools.get(id)
                    && tool.enabled
                    && tool.status.is_ready()
                {
                    return Some(id.clone());
                }
            }
        }

        // 3. Fall back to any enabled ready tool
        for tool in self.tools.values() {
            if tool.enabled && tool.status.is_ready() {
                return Some(tool.id.clone());
            }
        }

        None
    }

    /// Escalate after a failed attempt at `tier` with `failed_tool`.
    /// Returns the next tier up and the tool for it (never the same tool again at the same tier).
    /// Returns `None` at the top tier or when nothing is ready.
    pub fn escalate(&self, tier: Tier, failed_tool: &ToolId) -> Option<(Tier, ToolId)> {
        let next_tier = tier.escalate()?;

        // Try the routed tool for the next tier
        if let Some(routed_id) = self.routing.get(&next_tier)
            && routed_id != failed_tool
            && let Some(tool) = self.tools.get(routed_id)
            && tool.enabled
            && tool.status.is_ready()
        {
            return Some((next_tier, routed_id.clone()));
        }

        // Fall back to any enabled ready tool of the next tier, skipping the failed tool
        if let Some(tier_tools) = self.tools_by_tier.get(&next_tier) {
            for id in tier_tools {
                if id == failed_tool {
                    continue;
                }
                if let Some(tool) = self.tools.get(id)
                    && tool.enabled
                    && tool.status.is_ready()
                {
                    return Some((next_tier, id.clone()));
                }
            }
        }

        // Fall back to any enabled ready tool (any tier), skipping the failed tool
        for tool in self.tools.values() {
            if &tool.id == failed_tool {
                continue;
            }
            if tool.enabled && tool.status.is_ready() {
                return Some((next_tier, tool.id.clone()));
            }
        }

        None
    }

    /// Maximum attempts per task (3 total).
    pub fn retry_limit(&self) -> u32 {
        3
    }

    /// True when at least 3 enabled ready tools exist from at least 2 different adapters.
    pub fn has_enough_tools(&self) -> bool {
        let mut ready_count = 0;
        let mut adapters_with_ready = 0;

        for tool_ids in self.tools_by_adapter.values() {
            let mut adapter_ready = false;
            for id in tool_ids {
                if let Some(tool) = self.tools.get(id)
                    && tool.enabled
                    && tool.status.is_ready()
                {
                    ready_count += 1;
                    adapter_ready = true;
                }
            }
            if adapter_ready {
                adapters_with_ready += 1;
            }
        }

        ready_count >= 3 && adapters_with_ready >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{Tier, ToolId, ToolInfo, ToolStatus};
    use std::collections::HashMap;

    #[allow(clippy::too_many_arguments)]
    fn make_tool(
        id: &str,
        tier: Tier,
        adapter: &str,
        enabled: bool,
        status: ToolStatus,
    ) -> ToolInfo {
        ToolInfo {
            id: ToolId::new(id),
            name: id.to_string(),
            path: format!("/bin/{id}"),
            version: "1.0".to_string(),
            adapter: adapter.to_string(),
            status,
            tier,
            enabled,
        }
    }

    #[test]
    fn routed_tool_is_picked() {
        let routing = HashMap::from([
            (Tier::Cheap, ToolId::new("opencode")),
            (Tier::Mid, ToolId::new("codex")),
            (Tier::Smart, ToolId::new("claude")),
        ]);
        let tools = vec![
            make_tool("opencode", Tier::Cheap, "generic", true, ToolStatus::Ready),
            make_tool("codex", Tier::Mid, "generic", true, ToolStatus::Ready),
            make_tool("claude", Tier::Smart, "generic", true, ToolStatus::Ready),
        ];
        let router = Router::new(routing, tools);

        assert_eq!(router.pick(Tier::Cheap), Some(ToolId::new("opencode")));
        assert_eq!(router.pick(Tier::Mid), Some(ToolId::new("codex")));
        assert_eq!(router.pick(Tier::Smart), Some(ToolId::new("claude")));
    }

    #[test]
    fn falls_back_to_same_tier_when_routed_tool_not_ready() {
        let routing = HashMap::from([
            (Tier::Cheap, ToolId::new("opencode")),
            (Tier::Mid, ToolId::new("codex")),
            (Tier::Smart, ToolId::new("claude")),
        ]);
        let tools = vec![
            make_tool(
                "opencode",
                Tier::Cheap,
                "generic",
                true,
                ToolStatus::NeedsLogin,
            ),
            make_tool("goose", Tier::Cheap, "generic", true, ToolStatus::Ready),
            make_tool("codex", Tier::Mid, "generic", true, ToolStatus::Ready),
            make_tool("claude", Tier::Smart, "generic", true, ToolStatus::Ready),
        ];
        let router = Router::new(routing, tools);

        assert_eq!(router.pick(Tier::Cheap), Some(ToolId::new("goose")));
    }

    #[test]
    fn falls_back_to_any_ready_tool() {
        let routing = HashMap::from([
            (Tier::Cheap, ToolId::new("opencode")),
            (Tier::Mid, ToolId::new("codex")),
            (Tier::Smart, ToolId::new("claude")),
        ]);
        let tools = vec![
            make_tool(
                "opencode",
                Tier::Cheap,
                "generic",
                true,
                ToolStatus::NeedsLogin,
            ),
            make_tool(
                "goose",
                Tier::Cheap,
                "generic",
                true,
                ToolStatus::NeedsLogin,
            ),
            make_tool("codex", Tier::Mid, "generic", true, ToolStatus::NeedsLogin),
            make_tool("claude", Tier::Smart, "generic", true, ToolStatus::Ready),
        ];
        let router = Router::new(routing, tools);

        assert_eq!(router.pick(Tier::Cheap), Some(ToolId::new("claude")));
    }

    #[test]
    fn none_ready_gives_none() {
        let routing = HashMap::from([(Tier::Cheap, ToolId::new("opencode"))]);
        let tools = vec![make_tool(
            "opencode",
            Tier::Cheap,
            "generic",
            true,
            ToolStatus::NeedsLogin,
        )];
        let router = Router::new(routing, tools);

        assert_eq!(router.pick(Tier::Cheap), None);
    }

    #[test]
    fn disabled_tool_is_never_picked() {
        let routing = HashMap::from([(Tier::Cheap, ToolId::new("opencode"))]);
        let tools = vec![make_tool(
            "opencode",
            Tier::Cheap,
            "generic",
            false,
            ToolStatus::Ready,
        )];
        let router = Router::new(routing, tools);

        assert_eq!(router.pick(Tier::Cheap), None);
    }

    #[test]
    fn escalate_climbs_cheap_to_mid_to_smart_and_stops() {
        let routing = HashMap::from([
            (Tier::Cheap, ToolId::new("opencode")),
            (Tier::Mid, ToolId::new("codex")),
            (Tier::Smart, ToolId::new("claude")),
        ]);
        let tools = vec![
            make_tool("opencode", Tier::Cheap, "generic", true, ToolStatus::Ready),
            make_tool("codex", Tier::Mid, "generic", true, ToolStatus::Ready),
            make_tool("claude", Tier::Smart, "generic", true, ToolStatus::Ready),
        ];
        let router = Router::new(routing, tools);

        assert_eq!(
            router.escalate(Tier::Cheap, &ToolId::new("opencode")),
            Some((Tier::Mid, ToolId::new("codex")))
        );
        assert_eq!(
            router.escalate(Tier::Mid, &ToolId::new("codex")),
            Some((Tier::Smart, ToolId::new("claude")))
        );
        assert_eq!(router.escalate(Tier::Smart, &ToolId::new("claude")), None);
    }

    #[test]
    fn escalate_skips_failed_tool() {
        let routing = HashMap::from([
            (Tier::Cheap, ToolId::new("opencode")),
            (Tier::Mid, ToolId::new("codex")),
            (Tier::Smart, ToolId::new("claude")),
        ]);
        let tools = vec![
            make_tool("opencode", Tier::Cheap, "generic", true, ToolStatus::Ready),
            make_tool("goose", Tier::Cheap, "generic", true, ToolStatus::Ready),
            make_tool("codex", Tier::Mid, "generic", true, ToolStatus::Ready),
            make_tool("gemini", Tier::Mid, "generic", true, ToolStatus::Ready),
            make_tool("claude", Tier::Smart, "generic", true, ToolStatus::Ready),
        ];
        let router = Router::new(routing, tools);

        assert_eq!(
            router.escalate(Tier::Cheap, &ToolId::new("codex")),
            Some((Tier::Mid, ToolId::new("gemini")))
        );
    }

    #[test]
    fn has_enough_tools_passes_with_3_tools_from_2_adapters() {
        let tools = vec![
            make_tool("opencode", Tier::Cheap, "adapter1", true, ToolStatus::Ready),
            make_tool("goose", Tier::Cheap, "adapter1", true, ToolStatus::Ready),
            make_tool("codex", Tier::Mid, "adapter2", true, ToolStatus::Ready),
        ];
        let routing = HashMap::new();
        let router = Router::new(routing, tools);
        assert!(router.has_enough_tools());
    }

    #[test]
    fn has_enough_tools_fails_with_3_tools_from_1_adapter() {
        let tools = vec![
            make_tool("opencode", Tier::Cheap, "adapter1", true, ToolStatus::Ready),
            make_tool("goose", Tier::Cheap, "adapter1", true, ToolStatus::Ready),
            make_tool("pi", Tier::Cheap, "adapter1", true, ToolStatus::Ready),
        ];
        let routing = HashMap::new();
        let router = Router::new(routing, tools);
        assert!(!router.has_enough_tools());
    }

    #[test]
    fn has_enough_tools_fails_with_2_tools() {
        let tools = vec![
            make_tool("opencode", Tier::Cheap, "adapter1", true, ToolStatus::Ready),
            make_tool("codex", Tier::Mid, "adapter2", true, ToolStatus::Ready),
        ];
        let routing = HashMap::new();
        let router = Router::new(routing, tools);
        assert!(!router.has_enough_tools());
    }

    #[test]
    fn has_enough_tools_fails_when_disabled_tools_dont_count() {
        let tools = vec![
            make_tool("opencode", Tier::Cheap, "adapter1", true, ToolStatus::Ready),
            make_tool("goose", Tier::Cheap, "adapter1", false, ToolStatus::Ready),
            make_tool("codex", Tier::Mid, "adapter2", true, ToolStatus::Ready),
        ];
        let routing = HashMap::new();
        let router = Router::new(routing, tools);
        assert!(!router.has_enough_tools());
    }

    #[test]
    fn has_enough_tools_fails_when_not_ready_tools_dont_count() {
        let tools = vec![
            make_tool("opencode", Tier::Cheap, "adapter1", true, ToolStatus::Ready),
            make_tool(
                "goose",
                Tier::Cheap,
                "adapter1",
                true,
                ToolStatus::NeedsLogin,
            ),
            make_tool("codex", Tier::Mid, "adapter2", true, ToolStatus::Ready),
        ];
        let routing = HashMap::new();
        let router = Router::new(routing, tools);
        assert!(!router.has_enough_tools());
    }
}
