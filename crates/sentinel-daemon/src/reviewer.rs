//! Who reviews a task's diff. A different agent gives the real review. Without one (fake mode, or
//! only one tool ready) the offline scope check stands in, and the receipt says so.

use std::sync::{Arc, Mutex};

use sentinel_core::{Scope, TaskId, ToolId, ToolInfo};
use sentinel_store::Store;
use sentinel_verify::Result as VerifyResult;
use sentinel_verify::verifier::{
    DiffReviewer, Review, ReviewRequest, parse_review_answer, review_prompt,
};
use tokio::sync::watch;

use crate::agents::{TextJob, run_for_text};
use crate::engine::{Engine, lock};
use crate::run_ctx::RunMemory;

/// Bytes of diff sent to a reviewing agent.
const MAX_DIFF_BYTES: usize = 40_000;

/// Checks that the diff touches only the files of the task, and at least one of them. It cannot
/// judge quality, so it is used only in fake mode.
pub(crate) struct ScopeReviewer {
    pub store: Store,
}

impl DiffReviewer for ScopeReviewer {
    fn review<'a>(
        &'a self,
        request: &'a ReviewRequest,
    ) -> futures::future::BoxFuture<'a, VerifyResult<Review>> {
        Box::pin(async move {
            let scope = self
                .store
                .get_task(&request.task_id)
                .ok()
                .flatten()
                .map_or_else(Scope::new, |t| t.scope);
            Ok(scope_review(&request.diff, &scope))
        })
    }
}

/// The pure part of the scope check.
pub(crate) fn scope_review(diff: &str, scope: &Scope) -> Review {
    let files = diff_files(diff);
    let outside: Vec<&String> = files.iter().filter(|f| !scope.covers(f)).collect();
    let (ok, reason) = if files.is_empty() {
        (false, "The diff changes no file.".to_owned())
    } else if outside.is_empty() {
        (
            true,
            "The diff touches only the files of the task.".to_owned(),
        )
    } else {
        (
            false,
            format!(
                "The diff touches files outside the task: {}.",
                outside
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
    };
    Review {
        ok,
        reason,
        reviewer: "scope check (no second agent)".to_owned(),
    }
}

/// Paths from the `+++ b/PATH` lines of a unified diff.
pub(crate) fn diff_files(diff: &str) -> Vec<String> {
    diff.lines()
        .filter_map(|line| line.strip_prefix("+++ b/"))
        .map(str::to_owned)
        .collect()
}

/// Asks a different ready agent whether the diff does what the task says.
pub(crate) struct AgentReviewer {
    pub engine: Engine,
    pub memory: Arc<Mutex<RunMemory>>,
    pub stop: watch::Receiver<bool>,
    pub budget: Arc<sentinel_workers::Budget>,
}

/// The best of the ready agents that did not write the diff. The highest tier is tried first. The
/// author is the chat's forced agent when the chat has one, so a forced agent never reviews itself.
pub(crate) fn pick_reviewer(mut tools: Vec<ToolInfo>, author: Option<&ToolId>) -> Option<ToolInfo> {
    tools.sort_by_key(|t| std::cmp::Reverse(t.tier));
    tools.into_iter().find(|t| Some(&t.id) != author)
}

impl AgentReviewer {
    fn pick(&self, task: &TaskId) -> Option<ToolInfo> {
        let author: Option<ToolId> = lock(&self.memory).agents.get(task).cloned();
        let tools: Vec<ToolInfo> = self
            .engine
            .store()
            .list_tools()
            .ok()?
            .into_iter()
            .filter(|t| t.enabled && t.status.is_ready())
            .collect();
        pick_reviewer(tools, author.as_ref())
    }
}

impl DiffReviewer for AgentReviewer {
    fn review<'a>(
        &'a self,
        request: &'a ReviewRequest,
    ) -> futures::future::BoxFuture<'a, VerifyResult<Review>> {
        Box::pin(async move {
            let Some(tool) = self.pick(&request.task_id) else {
                return Ok(Review {
                    ok: false,
                    reason: "No second agent was ready to review this diff.".to_owned(),
                    reviewer: "none".to_owned(),
                });
            };
            let Ok(worker) = self.engine.worker_for(&tool) else {
                return Ok(Review {
                    ok: false,
                    reason: format!("{} could not be started for the review.", tool.name),
                    reviewer: tool.id.to_string(),
                });
            };
            let prompt = review_prompt(request, MAX_DIFF_BYTES);
            let job = TextJob {
                stop: self.stop.clone(),
                budget: self.budget.clone(),
                worker: worker.as_ref(),
                tool: &tool,
                prompt,
            };
            let text = run_for_text(&self.engine, job).await;
            Ok(parse_review_answer(&text, &tool.name))
        })
    }
}

#[cfg(test)]
mod tests {
    use sentinel_core::{Tier, ToolStatus};

    use super::*;

    const DIFF: &str =
        "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n-x\n+y\n";

    #[test]
    fn a_diff_inside_the_scope_is_ok() {
        let scope = Scope::from_paths(["src/a.rs"]).unwrap();
        assert!(scope_review(DIFF, &scope).ok);
    }

    #[test]
    fn a_diff_outside_the_scope_is_not_ok_and_names_the_file() {
        let scope = Scope::from_paths(["src/b.rs"]).unwrap();
        let review = scope_review(DIFF, &scope);
        assert!(!review.ok);
        assert!(review.reason.contains("src/a.rs"), "{}", review.reason);
    }

    #[test]
    fn an_empty_diff_is_not_ok() {
        assert!(!scope_review("", &Scope::new()).ok);
    }

    #[test]
    fn diff_files_reads_the_new_path_of_each_file() {
        assert_eq!(diff_files(DIFF), ["src/a.rs"]);
    }

    fn reviewer_tool(id: &str, tier: Tier) -> ToolInfo {
        ToolInfo {
            id: ToolId::new(id),
            name: id.to_owned(),
            path: format!("/usr/bin/{id}"),
            version: "1.0".to_owned(),
            adapter: id.to_owned(),
            status: ToolStatus::Ready,
            tier,
            enabled: true,
        }
    }

    #[test]
    fn the_reviewer_is_never_the_agent_that_wrote_the_change() {
        let tools = vec![
            reviewer_tool("claude", Tier::Smart),
            reviewer_tool("codex", Tier::Mid),
        ];
        let reviewer = pick_reviewer(tools, Some(&ToolId::new("claude")));
        assert_eq!(reviewer.map(|tool| tool.id), Some(ToolId::new("codex")));
    }

    #[test]
    fn with_no_other_agent_there_is_no_reviewer() {
        let tools = vec![reviewer_tool("claude", Tier::Smart)];
        assert_eq!(pick_reviewer(tools, Some(&ToolId::new("claude"))), None);
    }

    #[test]
    fn the_highest_tier_other_agent_reviews_first() {
        let tools = vec![
            reviewer_tool("author", Tier::Smart),
            reviewer_tool("cheap", Tier::Cheap),
            reviewer_tool("mid", Tier::Mid),
        ];
        let reviewer = pick_reviewer(tools, Some(&ToolId::new("author")));
        assert_eq!(reviewer.map(|tool| tool.id), Some(ToolId::new("mid")));
    }
}
