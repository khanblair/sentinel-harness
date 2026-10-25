//! Agents that fail for a reason a retry will not fix (not signed in, a usage limit, a broken
//! install) are paused for a while, so routing goes around them and no task waits on them.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use sentinel_core::{Event, Tier, ToolId, ToolInfo};
use sentinel_workers::router::Router;
use sentinel_workers::{EndKind, RunOutcome};

use crate::agent_test::mask_secrets;
use crate::agents::routing_map;
use crate::agents_config::without_paused;
use crate::engine::{Engine, Result, lock};

/// How long an agent stays out of routing after a failure that a retry will not fix.
const PAUSE: Duration = Duration::from_mins(30);
const MAX_REASON_CHARS: usize = 100;
/// Lower-case text that marks a failure a retry will not fix. Matched one output line at a time.
const PAUSE_MARKERS: [&str; 16] = [
    "login",
    "log in",
    "sign in",
    "authenticate",
    "unauthorized",
    "401",
    "403",
    "api key",
    "free usage limit",
    "usage limit",
    "quota",
    "rate limit",
    "429",
    "keytar",
    "ineligible",
    "project id",
];

struct Pause {
    until: Instant,
    reason: String,
}

/// The paused agents of one engine. A pause that has run out is dropped when it is next read.
#[derive(Default)]
pub(crate) struct Health {
    pauses: Mutex<HashMap<ToolId, Pause>>,
}

impl Health {
    /// Pause an agent. Returns `false` and keeps the first pause when the agent is already paused.
    pub(crate) fn pause(&self, tool: &ToolId, reason: &str) -> bool {
        if self.is_paused(tool).is_some() {
            return false;
        }
        let until = Instant::now() + PAUSE;
        lock(&self.pauses).insert(
            tool.clone(),
            Pause {
                until,
                reason: reason.to_owned(),
            },
        );
        true
    }

    /// Lift a pause, for example after a Test of the agent passes.
    pub(crate) fn clear(&self, tool: &ToolId) {
        lock(&self.pauses).remove(tool);
    }

    /// The reason for a pause still in force, else `None`.
    pub(crate) fn is_paused(&self, tool: &ToolId) -> Option<String> {
        let mut pauses = lock(&self.pauses);
        let pause = pauses.get(tool)?;
        if pause.until > Instant::now() {
            return Some(pause.reason.clone());
        }
        pauses.remove(tool);
        None
    }

    /// The ids of every agent paused now.
    pub(crate) fn paused_ids(&self) -> HashSet<ToolId> {
        let mut pauses = lock(&self.pauses);
        let now = Instant::now();
        pauses.retain(|_, pause| pause.until > now);
        pauses.keys().cloned().collect()
    }

    /// The tool among these whose pause ends first, if any of them is paused.
    pub(crate) fn first_to_resume(&self, tools: &[ToolInfo]) -> Option<ToolInfo> {
        let pauses = lock(&self.pauses);
        tools
            .iter()
            .filter_map(|tool| pauses.get(&tool.id).map(|pause| (pause.until, tool)))
            .min_by_key(|(until, _)| *until)
            .map(|(_, tool)| tool.clone())
    }
}

/// A short reason from the first output line that names a failure a retry will not fix. Keys on
/// that line are masked, because the line reaches the owner's screen.
pub(crate) fn pause_reason(adapter_output: &str) -> Option<String> {
    adapter_output
        .lines()
        .find(|line| names_a_pause(line))
        .map(|line| {
            mask_secrets(line.trim())
                .chars()
                .take(MAX_REASON_CHARS)
                .collect()
        })
}

fn names_a_pause(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    PAUSE_MARKERS.iter().any(|marker| lower.contains(marker))
}

impl Engine {
    /// The agent for the next attempt at a tier. A paused agent is left out while another ready one
    /// is open. When every ready agent is paused, the one that resumes first is tried anyway.
    pub(crate) fn pick_agent(&self, tier: Tier) -> Result<Option<ToolId>> {
        let ready: Vec<ToolInfo> = self
            .store()
            .list_tools()?
            .into_iter()
            .filter(|t| t.enabled && t.status.is_ready())
            .collect();
        let paused = self.inner.health.paused_ids();
        let open = without_paused(ready, &paused);
        if open.is_empty() {
            return Ok(None);
        }
        if open.iter().all(|t| paused.contains(&t.id)) {
            return Ok(self.pick_when_all_paused(&open));
        }
        Ok(Router::new(routing_map(&self.config().routing), open).pick(tier))
    }

    /// Every ready agent is paused: try the one that resumes first, and say so in the stream.
    fn pick_when_all_paused(&self, tools: &[ToolInfo]) -> Option<ToolId> {
        let next = self
            .inner
            .health
            .first_to_resume(tools)
            .or_else(|| tools.first().cloned())?;
        self.emit_quiet(&Event::Message {
            from: "sentinel".to_owned(),
            to: None,
            text: format!("All agents are paused. Trying {} anyway.", next.name),
        });
        Some(next.id)
    }
}

/// An attempt ended with an error exit. When its output names a failure a retry will not fix,
/// pause the agent. The owner hears about it once per pause.
pub(crate) fn pause_after_failure(engine: &Engine, tool: &ToolInfo, outcome: &RunOutcome) {
    if outcome.end != EndKind::Failed {
        return;
    }
    let Some(reason) = pause_reason(&outcome.output_tail.join("\n")) else {
        return;
    };
    if engine.inner.health.pause(&tool.id, &reason) {
        engine.emit_quiet(&Event::Message {
            from: "sentinel".to_owned(),
            to: None,
            text: pause_notice(&tool.name, &reason),
        });
    }
}

/// A Test that fails for a login or limit reason pauses the agent. A passing Test lifts the pause.
pub(crate) fn note_test_result(health: &Health, tool: &ToolId, ok: bool, message: &str) {
    if ok {
        health.clear(tool);
    } else if let Some(reason) = pause_reason(message) {
        health.pause(tool, &reason);
    }
}

fn pause_notice(name: &str, reason: &str) -> String {
    let minutes = PAUSE.as_secs() / 60;
    format!(
        "{name} is paused for {minutes} minutes: {}. Press Test in Settings, Agents to try it again.",
        reason.trim_end_matches('.')
    )
}

#[cfg(test)]
mod tests {
    use sentinel_core::{Tier, ToolStatus};

    use super::*;

    fn tool(id: &str) -> ToolInfo {
        ToolInfo {
            id: ToolId::new(id),
            name: id.to_owned(),
            path: format!("/usr/bin/{id}"),
            version: "1.0".to_owned(),
            adapter: id.to_owned(),
            status: ToolStatus::Ready,
            tier: Tier::Mid,
            enabled: true,
        }
    }

    #[test]
    fn a_usage_limit_line_is_the_pause_reason() {
        assert_eq!(
            pause_reason("Free usage limit reached"),
            Some("Free usage limit reached".to_owned())
        );
    }

    #[test]
    fn a_broken_keychain_module_is_a_pause_reason() {
        let line = "Keychain initialization encountered an error: Cannot find module keytar.node";
        assert_eq!(pause_reason(line), Some(line.to_owned()));
    }

    #[test]
    fn the_first_sign_in_or_project_line_is_the_pause_reason() {
        let output = "Starting up\nPlease sign in to continue\nProject ID is missing";
        assert_eq!(
            pause_reason(output),
            Some("Please sign in to continue".to_owned())
        );
    }

    #[test]
    fn a_normal_compile_error_does_not_pause_the_agent() {
        let output =
            "error[E0308]: mismatched types\n  --> src/lib.rs:12:5\n   = note: expected `u32`";
        assert_eq!(pause_reason(output), None);
    }

    #[test]
    fn a_key_in_the_pause_reason_is_masked() {
        let output = "Unauthorized: key sk-abcdefghijklmnopqrstuvwx1234 was refused";
        assert_eq!(
            pause_reason(output),
            Some("Unauthorized: key ... was refused".to_owned())
        );
    }

    #[test]
    fn a_pause_reason_is_cut_to_one_hundred_characters() {
        let output = format!("quota {}", "a".repeat(200));
        assert_eq!(
            pause_reason(&output).map(|reason| reason.chars().count()),
            Some(MAX_REASON_CHARS)
        );
    }

    #[test]
    fn a_pause_starts_once_and_keeps_its_first_reason() {
        let health = Health::default();
        let claude = ToolId::new("claude");
        assert!(health.pause(&claude, "Free usage limit reached"));
        assert!(!health.pause(&claude, "Keychain broke"));
        assert_eq!(
            health.is_paused(&claude),
            Some("Free usage limit reached".to_owned())
        );
    }

    #[test]
    fn a_pause_that_has_run_out_no_longer_holds_the_agent() {
        let health = Health::default();
        let claude = ToolId::new("claude");
        lock(&health.pauses).insert(
            claude.clone(),
            Pause {
                until: Instant::now(),
                reason: "quota".to_owned(),
            },
        );
        assert_eq!(health.is_paused(&claude), None);
        assert!(health.paused_ids().is_empty());
    }

    #[test]
    fn clearing_a_pause_puts_the_agent_back_in_routing() {
        let health = Health::default();
        let claude = ToolId::new("claude");
        health.pause(&claude, "quota");
        health.clear(&claude);
        assert_eq!(health.is_paused(&claude), None);
        assert!(health.paused_ids().is_empty());
    }

    #[test]
    fn the_pause_that_ends_first_is_the_one_tried_when_all_are_paused() {
        let health = Health::default();
        let claude = tool("claude");
        let codex = tool("codex");
        let now = Instant::now();
        {
            let mut pauses = lock(&health.pauses);
            pauses.insert(
                claude.id.clone(),
                Pause {
                    until: now + Duration::from_mins(30),
                    reason: "quota".to_owned(),
                },
            );
            pauses.insert(
                codex.id.clone(),
                Pause {
                    until: now + Duration::from_mins(5),
                    reason: "quota".to_owned(),
                },
            );
        }
        assert_eq!(
            health.first_to_resume(&[claude, codex.clone()]),
            Some(codex)
        );
    }

    #[test]
    fn a_failed_test_that_says_not_signed_in_pauses_the_agent() {
        let health = Health::default();
        let claude = ToolId::new("claude");
        note_test_result(
            &health,
            &claude,
            false,
            "It is not signed in. Sign in to Claude in a terminal, then test again.",
        );
        assert!(health.is_paused(&claude).is_some());
    }

    #[test]
    fn a_passing_test_clears_the_pause() {
        let health = Health::default();
        let claude = ToolId::new("claude");
        health.pause(&claude, "quota");
        note_test_result(&health, &claude, true, "Works. It answered in 1.2 s.");
        assert_eq!(health.is_paused(&claude), None);
    }

    #[test]
    fn a_failed_test_with_an_ordinary_error_leaves_the_agent_running() {
        let health = Health::default();
        let claude = ToolId::new("claude");
        note_test_result(&health, &claude, false, "bad token ...");
        assert_eq!(health.is_paused(&claude), None);
    }
}
