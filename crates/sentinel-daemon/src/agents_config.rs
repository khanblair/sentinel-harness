//! Pure helpers that pick the tools and the routing from one scan. They take no engine, so the
//! choices can be tested on plain values.

use std::collections::HashSet;

use sentinel_core::{Routing, Tier, ToolId, ToolInfo};

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
pub(crate) fn without_paused(ready: Vec<ToolInfo>, paused: &HashSet<ToolId>) -> Vec<ToolInfo> {
    if ready.iter().all(|tool| paused.contains(&tool.id)) {
        return ready;
    }
    ready
        .into_iter()
        .filter(|tool| !paused.contains(&tool.id))
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
        let paused = HashSet::from([ToolId::new("claude")]);
        let open = without_paused(vec![claude, codex.clone()], &paused);
        assert_eq!(open, [codex]);
    }

    #[test]
    fn when_every_ready_tool_is_paused_the_list_comes_back_whole() {
        let claude = ready_tool("claude", Tier::Mid);
        let codex = ready_tool("codex", Tier::Mid);
        let paused = HashSet::from([ToolId::new("claude"), ToolId::new("codex")]);
        let open = without_paused(vec![claude.clone(), codex.clone()], &paused);
        assert_eq!(open, [claude, codex]);
    }
}
