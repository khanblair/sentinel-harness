//! Pure helpers that pick the tools and the routing from one scan. They take no engine, so the
//! choices can be tested on plain values.

use std::collections::HashMap;

use sentinel_core::{Routing, Tier, ToolId, ToolInfo};
use sentinel_workers::router::Router;

use crate::agents::routing_map;

/// Said when no ready agent is left for a task or a question.
pub(crate) const NO_AGENT: &str = "No agent is ready for this kind of task.";

/// The agent one attempt, question, or plan goes to, or why none can take it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Pick {
    Agent(ToolId),
    /// The reason, in words for the owner.
    Unavailable(String),
}

/// What a pick chooses from: the agents that are on and ready, the paused ones with the reason each
/// one is paused, and the routing of the tiers.
pub(crate) struct Pool<'a> {
    pub ready: &'a [ToolInfo],
    pub paused: &'a HashMap<ToolId, String>,
    pub routing: &'a Routing,
}

/// The agent for one attempt, question, or plan. A forced agent is the only one that may take it:
/// the tier and the routing do not count. Without one, the routing picks among the agents that are
/// not paused.
pub(crate) fn pick_for(forced: Option<&ToolId>, tier: Tier, pool: &Pool<'_>) -> Pick {
    match forced {
        Some(agent) => forced_pick(agent, pool),
        None => routed_pick(tier, pool),
    }
}

fn forced_pick(agent: &ToolId, pool: &Pool<'_>) -> Pick {
    let Some(tool) = pool.ready.iter().find(|tool| &tool.id == agent) else {
        return Pick::Unavailable(format!("{agent} is not ready."));
    };
    match pool.paused.get(agent) {
        Some(reason) => Pick::Unavailable(format!(
            "{} is paused: {}.",
            tool.name,
            reason.trim_end_matches('.')
        )),
        None => Pick::Agent(agent.clone()),
    }
}

fn routed_pick(tier: Tier, pool: &Pool<'_>) -> Pick {
    let open = without_paused(pool.ready.to_vec(), pool.paused);
    Router::new(routing_map(pool.routing), open)
        .pick(tier)
        .map_or_else(|| Pick::Unavailable(NO_AGENT.to_owned()), Pick::Agent)
}

/// The first ready tool of the tier, else the first ready tool of any tier.
pub(crate) fn route_for_tier(ready: &[&ToolInfo], tier: Tier) -> Option<ToolId> {
    ready
        .iter()
        .find(|t| t.tier == tier)
        .or_else(|| ready.first())
        .map(|t| t.id.clone())
}

/// The enabled tools and the routing, with every id the scan did not find taken out.
pub(crate) fn without_missing_tools(
    enabled: &[ToolId],
    routing: &Routing,
    found: &[ToolInfo],
) -> (Vec<ToolId>, Routing) {
    let known = |id: &ToolId| found.iter().any(|t| &t.id == id);
    let keep = |slot: &Option<ToolId>| slot.clone().filter(|id| known(id));
    (
        enabled.iter().filter(|id| known(id)).cloned().collect(),
        Routing {
            cheap: keep(&routing.cheap),
            mid: keep(&routing.mid),
            smart: keep(&routing.smart),
        },
    )
}

/// The ready tools that are not paused. When every ready tool is paused, the list comes back whole,
/// so a task can still reach an agent.
pub(crate) fn without_paused(
    ready: Vec<ToolInfo>,
    paused: &HashMap<ToolId, String>,
) -> Vec<ToolInfo> {
    if ready.iter().all(|tool| paused.contains_key(&tool.id)) {
        return ready;
    }
    ready
        .into_iter()
        .filter(|tool| !paused.contains_key(&tool.id))
        .collect()
}

/// Fill each empty tier of the routing from the usable tools, by tier. A set tier stays as it is.
pub(crate) fn fill_routing(routing: Routing, usable: &[&ToolInfo]) -> Routing {
    Routing {
        cheap: routing
            .cheap
            .or_else(|| route_for_tier(usable, Tier::Cheap)),
        mid: routing.mid.or_else(|| route_for_tier(usable, Tier::Mid)),
        smart: routing
            .smart
            .or_else(|| route_for_tier(usable, Tier::Smart)),
    }
}

#[cfg(test)]
mod tests {
    use sentinel_core::ToolStatus;

    use super::*;

    fn ready_tool(id: &str, tier: Tier) -> ToolInfo {
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

    /// A ready tool that is also on, as the pick sees it.
    fn on_tool(id: &str, tier: Tier) -> ToolInfo {
        ToolInfo {
            enabled: true,
            ..ready_tool(id, tier)
        }
    }

    fn paused_by(id: &str, reason: &str) -> HashMap<ToolId, String> {
        HashMap::from([(ToolId::new(id), reason.to_owned())])
    }

    fn routed_to(cheap: &str) -> Routing {
        Routing {
            cheap: Some(ToolId::new(cheap)),
            ..Routing::default()
        }
    }

    #[test]
    fn a_tier_takes_its_own_ready_tool_before_any_other() {
        let cli = ready_tool("claude", Tier::Cheap);
        let key = ready_tool("openrouter", Tier::Cheap);
        let ready = [&cli, &key];
        assert_eq!(
            route_for_tier(&ready, Tier::Cheap),
            Some(ToolId::new("claude"))
        );
    }

    #[test]
    fn a_saved_key_only_serves_a_tier_no_cli_covers() {
        let mid = ready_tool("claude", Tier::Mid);
        let smart = ready_tool("codex", Tier::Smart);
        let key = ready_tool("openrouter", Tier::Cheap);
        let ready = [&mid, &smart, &key];
        assert_eq!(
            route_for_tier(&ready, Tier::Cheap),
            Some(ToolId::new("openrouter"))
        );
        assert_eq!(
            route_for_tier(&ready, Tier::Mid),
            Some(ToolId::new("claude"))
        );
        assert_eq!(
            route_for_tier(&ready, Tier::Smart),
            Some(ToolId::new("codex"))
        );
    }

    #[test]
    fn a_tier_with_no_ready_tool_falls_back_to_the_first_ready_one() {
        let key = ready_tool("openrouter", Tier::Cheap);
        let ready = [&key];
        assert_eq!(
            route_for_tier(&ready, Tier::Smart),
            Some(ToolId::new("openrouter"))
        );
        assert_eq!(route_for_tier(&[], Tier::Smart), None);
    }

    #[test]
    fn a_stale_fake_tool_is_dropped_and_its_tier_refilled_from_the_real_one() {
        let claude = ready_tool("claude", Tier::Mid);
        let stale = ToolId::new("fake-a");
        let routing = Routing {
            cheap: Some(stale.clone()),
            mid: Some(ToolId::new("claude")),
            smart: None,
        };
        let (enabled, kept) = without_missing_tools(
            &[stale, ToolId::new("claude")],
            &routing,
            std::slice::from_ref(&claude),
        );
        assert_eq!(enabled, [ToolId::new("claude")]);
        assert_eq!(kept.cheap, None);
        let filled = fill_routing(kept, &[&claude]);
        assert_eq!(filled.cheap, Some(ToolId::new("claude")));
        assert_eq!(filled.mid, Some(ToolId::new("claude")));
        assert_eq!(filled.smart, Some(ToolId::new("claude")));
    }

    #[test]
    fn a_computer_with_no_ready_tool_ends_with_nothing_enabled_or_routed() {
        let mut login = ready_tool("claude", Tier::Mid);
        login.status = ToolStatus::NeedsLogin;
        let stale = ToolId::new("fake-a");
        let routing = Routing {
            cheap: Some(stale.clone()),
            mid: Some(stale.clone()),
            smart: Some(stale.clone()),
        };
        let (enabled, kept) = without_missing_tools(&[stale], &routing, &[login]);
        assert_eq!(enabled, Vec::<ToolId>::new());
        assert_eq!(fill_routing(kept, &[]), Routing::default());
    }

    #[test]
    fn a_paused_tool_is_left_out_while_another_ready_tool_is_open() {
        let claude = ready_tool("claude", Tier::Mid);
        let codex = ready_tool("codex", Tier::Mid);
        let open = without_paused(vec![claude, codex.clone()], &paused_by("claude", "quota"));
        assert_eq!(open, [codex]);
    }

    #[test]
    fn when_every_ready_tool_is_paused_the_list_comes_back_whole() {
        let claude = ready_tool("claude", Tier::Mid);
        let codex = ready_tool("codex", Tier::Mid);
        let paused = HashMap::from([
            (ToolId::new("claude"), "quota".to_owned()),
            (ToolId::new("codex"), "quota".to_owned()),
        ]);
        let open = without_paused(vec![claude.clone(), codex.clone()], &paused);
        assert_eq!(open, [claude, codex]);
    }

    #[test]
    fn a_forced_agent_is_picked_even_when_the_routing_names_another() {
        let claude = on_tool("claude", Tier::Mid);
        let codex = on_tool("codex", Tier::Cheap);
        let ready = [claude, codex];
        let paused = HashMap::new();
        let routing = routed_to("codex");
        let pool = Pool {
            ready: &ready,
            paused: &paused,
            routing: &routing,
        };
        let forced = ToolId::new("claude");
        assert_eq!(
            pick_for(Some(&forced), Tier::Cheap, &pool),
            Pick::Agent(ToolId::new("claude"))
        );
    }

    #[test]
    fn a_forced_agent_that_is_not_ready_is_unavailable() {
        let codex = on_tool("codex", Tier::Cheap);
        let ready = [codex];
        let paused = HashMap::new();
        let routing = Routing::default();
        let pool = Pool {
            ready: &ready,
            paused: &paused,
            routing: &routing,
        };
        let forced = ToolId::new("claude");
        assert_eq!(
            pick_for(Some(&forced), Tier::Cheap, &pool),
            Pick::Unavailable("claude is not ready.".to_owned())
        );
    }

    #[test]
    fn a_forced_agent_that_is_paused_is_unavailable_with_its_reason() {
        let claude = on_tool("claude", Tier::Mid);
        let ready = [claude];
        let paused = paused_by("claude", "Free usage limit reached.");
        let routing = Routing::default();
        let pool = Pool {
            ready: &ready,
            paused: &paused,
            routing: &routing,
        };
        let forced = ToolId::new("claude");
        assert_eq!(
            pick_for(Some(&forced), Tier::Mid, &pool),
            Pick::Unavailable("claude is paused: Free usage limit reached.".to_owned())
        );
    }

    #[test]
    fn without_a_forced_agent_the_routed_tool_takes_the_tier() {
        let claude = on_tool("claude", Tier::Mid);
        let codex = on_tool("codex", Tier::Cheap);
        let ready = [claude, codex];
        let paused = HashMap::new();
        let routing = routed_to("codex");
        let pool = Pool {
            ready: &ready,
            paused: &paused,
            routing: &routing,
        };
        assert_eq!(
            pick_for(None, Tier::Cheap, &pool),
            Pick::Agent(ToolId::new("codex"))
        );
    }

    #[test]
    fn without_a_forced_agent_a_paused_routed_tool_is_skipped() {
        let claude = on_tool("claude", Tier::Mid);
        let codex = on_tool("codex", Tier::Cheap);
        let ready = [claude, codex];
        let paused = paused_by("codex", "quota");
        let routing = routed_to("codex");
        let pool = Pool {
            ready: &ready,
            paused: &paused,
            routing: &routing,
        };
        assert_eq!(
            pick_for(None, Tier::Cheap, &pool),
            Pick::Agent(ToolId::new("claude"))
        );
    }

    #[test]
    fn without_a_forced_agent_and_no_ready_tool_the_pick_says_so() {
        let paused = HashMap::new();
        let routing = Routing::default();
        let pool = Pool {
            ready: &[],
            paused: &paused,
            routing: &routing,
        };
        assert_eq!(
            pick_for(None, Tier::Cheap, &pool),
            Pick::Unavailable(NO_AGENT.to_owned())
        );
    }
}
