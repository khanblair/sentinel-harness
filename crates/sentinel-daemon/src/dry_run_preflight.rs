//! The agent check before a dry run. Each agent the dry run may use is tested once, with the same
//! probe as the Test button. An agent that fails is turned off in the second engine only, so the
//! owner's own settings stay as they were.

use std::time::Duration;

use futures::future::join_all;
use sentinel_core::ToolId;

use crate::engine::{Engine, Result};

/// The most agents the check tests. The rest stay as they are and are not tested.
const MAX_TESTED: usize = 5;
/// A test that takes longer counts as a failure, so one slow agent cannot hold the dry run.
const PREFLIGHT_LIMIT: Duration = Duration::from_secs(45);
/// The most of an agent's own message a note keeps.
const MESSAGE_CHARS: usize = 120;
/// Shown when no tested agent passed, so the owner knows where to go.
pub(crate) const NO_AGENT_PASSED: &str =
    "No agent passed its test. Open Settings, Agents and press Test.";

/// What the check found: the agents that passed, and one note for each that did not.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Preflight {
    pub usable: Vec<ToolId>,
    pub notes: Vec<String>,
}

/// Sort the test results into the agents that passed and a note for each that failed.
pub(crate) fn plan_preflight(results: &[(ToolId, bool, String)]) -> Preflight {
    let mut preflight = Preflight::default();
    for (id, ok, message) in results {
        if *ok {
            preflight.usable.push(id.clone());
        } else {
            let reason: String = message.chars().take(MESSAGE_CHARS).collect();
            preflight.notes.push(format!("Skipped {id}: {reason}"));
        }
    }
    preflight
}

/// Test the enabled, ready agents of the second engine, then turn off the ones that failed there.
pub(crate) async fn check_agents(second: &Engine) -> Result<Preflight> {
    let candidates = testable(second)?;
    let results = join_all(candidates.iter().map(|id| probe(second, id))).await;
    let preflight = plan_preflight(&results);
    for id in candidates.iter().filter(|id| !preflight.usable.contains(id)) {
        second.toggle_tool(id, false)?;
    }
    Ok(preflight)
}

/// The enabled tools that are ready now, at most `MAX_TESTED` of them.
fn testable(second: &Engine) -> Result<Vec<ToolId>> {
    let tools = second.store().list_tools()?;
    Ok(tools
        .into_iter()
        .filter(|tool| tool.enabled && tool.status.is_ready())
        .take(MAX_TESTED)
        .map(|tool| tool.id)
        .collect())
}

/// One agent's test under the time limit. When the limit ends the test, the agent is stopped too.
async fn probe(second: &Engine, id: &ToolId) -> (ToolId, bool, String) {
    let outcome = tokio::time::timeout(PREFLIGHT_LIMIT, second.test_agent(id)).await;
    let (ok, message) = match outcome {
        Err(_) => (
            false,
            format!("It did not answer in {} seconds.", PREFLIGHT_LIMIT.as_secs()),
        ),
        Ok(Ok(Some(test))) => (test.ok, test.message),
        Ok(Ok(None)) => (false, "This agent is not on this computer.".to_owned()),
        Ok(Err(error)) => (false, error.to_string()),
    };
    (id.clone(), ok, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(id: &str) -> ToolId {
        ToolId::new(id)
    }

    #[test]
    fn a_passing_agent_is_kept_and_a_failing_one_is_noted() {
        let results = vec![
            (tool("claude"), true, "Works. It answered in 1.2 s.".to_owned()),
            (tool("goose"), false, "It is not signed in.".to_owned()),
        ];
        assert_eq!(
            plan_preflight(&results),
            Preflight {
                usable: vec![tool("claude")],
                notes: vec!["Skipped goose: It is not signed in.".to_owned()],
            }
        );
    }

    #[test]
    fn a_long_failure_message_is_cut_to_120_characters() {
        let results = vec![(tool("opencode"), false, "x".repeat(200))];
        let preflight = plan_preflight(&results);
        assert_eq!(
            preflight.notes,
            [format!("Skipped opencode: {}", "x".repeat(120))]
        );
    }

    #[test]
    fn when_every_test_fails_no_agent_is_usable() {
        let results = vec![(tool("goose"), false, "No reply.".to_owned())];
        assert!(plan_preflight(&results).usable.is_empty());
    }
}
