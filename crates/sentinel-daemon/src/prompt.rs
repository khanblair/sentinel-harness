//! The text each worker gets. Rules go in the prompt as guidance; the guard enforces the real
//! limits (reserved files, no edits to existing tests).

use sentinel_core::Task;
use sentinel_workers::context::{ContextLimits, RetryParts, retry_prompt};

/// What a retry adds on top of the task: a capped failure summary and the Brain notes loaded.
pub struct Retry<'a> {
    pub failure: &'a str,
    pub diff_stat: &'a str,
    pub notes: &'a [String],
    pub lessons: &'a [String],
}

const RULES: &str = "Rules:\n\
- Write only the files listed above. Another agent works in the same folder on other files.\n\
- Add or change a test for what you build. Never edit an existing test file to make code pass.\n\
- Keep functions short, name things for what they are, and leave no placeholder code.\n\
- Do not run git commands that change history. Do not install or remove tools.\n\
- When you are done, stop. Do not claim success in text: the test suite decides.";

/// The prompt for one attempt. The first attempt gets the task and the rules. A retry also gets
/// what went wrong, already cut down to a few lines.
pub fn task_prompt(task: &Task, retry: Option<&Retry<'_>>, limits: &ContextLimits) -> String {
    let scope: Vec<String> = task.scope.entries().map(str::to_owned).collect();
    if let Some(retry) = retry {
        let parts = RetryParts {
            task_title: &task.title,
            task_body: &task.body,
            scope: &scope,
            notes: retry.notes,
            lessons: retry.lessons,
            failure: retry.failure,
            diff_stat: retry.diff_stat,
        };
        return format!("{}\n\n{RULES}", retry_prompt(&parts, limits));
    }
    let files = if scope.is_empty() {
        "(none: this task is read only)".to_owned()
    } else {
        scope
            .iter()
            .map(|p| format!("- {p}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "You are one of several coding agents working in the same folder. Do this one task.\n\n\
         Task {id}: {title}\n{body}\n\nFiles you may write:\n{files}\n\n{RULES}",
        id = task.id,
        title = task.title,
        body = task.body.trim(),
    )
}

#[cfg(test)]
mod tests {
    use sentinel_core::{Scope, Task, TaskId};

    use super::*;

    fn task() -> Task {
        let mut task = Task::new(
            TaskId::new("T-02"),
            "Reject negative amounts",
            "Post fails for < 0.",
        );
        task.scope = Scope::from_paths(["src/ledger.rs", "tests/negative.rs"]).unwrap();
        task
    }

    #[test]
    fn first_prompt_names_the_task_the_files_and_the_rules() {
        let text = task_prompt(&task(), None, &ContextLimits::default());
        for needle in [
            "Task T-02: Reject negative amounts",
            "- src/ledger.rs",
            "- tests/negative.rs",
            "Never edit an existing test",
        ] {
            assert!(text.contains(needle), "{needle} missing in {text}");
        }
    }

    #[test]
    fn a_task_without_files_says_it_is_read_only() {
        let mut t = task();
        t.scope = Scope::new();
        assert!(task_prompt(&t, None, &ContextLimits::default()).contains("read only"));
    }

    #[test]
    fn a_retry_carries_the_failure_summary_and_the_rules() {
        let retry = Retry {
            failure: "Failing tests:\nnegative_amount_is_refused",
            diff_stat: "1 file",
            notes: &[],
            lessons: &[],
        };
        let text = task_prompt(&task(), Some(&retry), &ContextLimits::default());
        assert!(text.contains("negative_amount_is_refused"));
        assert!(text.contains("retry"));
        assert!(text.contains("Never edit an existing test"));
    }
}
