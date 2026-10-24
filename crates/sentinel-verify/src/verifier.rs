//! Verifier: the second-model diff review contract.

use std::future::Future;
use std::pin::Pin;

use sentinel_core::ids::TaskId;

use crate::Result;

/// Request for a diff review.
#[derive(Debug, Clone)]
pub struct ReviewRequest {
    /// The task being reviewed.
    pub task_id: TaskId,
    /// The task title.
    pub title: String,
    /// The task body/description.
    pub body: String,
    /// The unified diff to review.
    pub diff: String,
}

/// Result of a diff review.
#[derive(Debug, Clone)]
pub struct Review {
    /// Whether the diff matches the task.
    pub ok: bool,
    /// Reason for the decision.
    pub reason: String,
    /// Identifier of the reviewer.
    pub reviewer: String,
}

/// Trait for a diff reviewer (second model).
pub trait DiffReviewer: Send + Sync {
    /// Review a diff against a task.
    fn review<'a>(&'a self, request: &'a ReviewRequest) -> BoxFuture<'a, Result<Review>>;
}

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Maximum bytes for the diff in the prompt before cutting.
#[allow(dead_code)]
const MAX_DIFF_BYTES_DEFAULT: usize = 50_000;

/// Builds the review prompt for a second model.
///
/// The prompt asks: does this diff do what the task says and nothing else?
/// The answer should start with YES or NO, followed by one sentence.
pub fn review_prompt(request: &ReviewRequest, max_diff_bytes: usize) -> String {
    let diff = cut_diff(&request.diff, max_diff_bytes);

    format!(
        r#"Task: {}
Description: {}

Diff:
{}

Does this diff do what the task says and nothing else?
Answer with a first line YES or NO, then one sentence."#,
        request.title, request.body, diff
    )
}

/// Cuts the diff at max_diff_bytes on a line boundary, adding a marker if cut.
fn cut_diff(diff: &str, max_bytes: usize) -> String {
    if diff.len() <= max_bytes {
        return diff.to_string();
    }

    // Find the last newline before max_bytes
    let cut_at = diff[..max_bytes].rfind('\n').unwrap_or(max_bytes);
    let mut result = diff[..cut_at].to_string();
    result.push_str("\n[diff cut]");
    result
}

/// Parses a review answer from the model's response.
///
/// The first non-empty line starting with YES (any case) means ok.
/// NO or anything else means not ok.
/// The reason is the rest of the text, trimmed, at most 300 chars.
pub fn parse_review_answer(text: &str, reviewer: &str) -> Review {
    let mut ok = false;
    let mut reason = String::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let upper = trimmed.to_uppercase();
        if upper.starts_with("YES") {
            ok = true;
            reason = trimmed[3..]
                .trim_start_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace())
                .to_string();
        } else if upper.starts_with("NO") {
            ok = false;
            reason = trimmed[2..]
                .trim_start_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace())
                .to_string();
        } else {
            // First non-empty line that isn't YES/NO - treat as NO
            ok = false;
            reason = trimmed.to_string();
        }
        break;
    }

    // Truncate reason to 300 chars
    if reason.len() > 300 {
        reason.truncate(300);
    }

    Review {
        ok,
        reason,
        reviewer: reviewer.to_string(),
    }
}

/// A fixed reviewer that returns a stored review (for tests).
#[derive(Debug, Clone)]
pub struct FixedReviewer {
    pub review: Review,
}

impl DiffReviewer for FixedReviewer {
    fn review<'a>(&'a self, _request: &'a ReviewRequest) -> BoxFuture<'a, Result<Review>> {
        Box::pin(async move { Ok(self.review.clone()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_review_answer_yes() {
        let review = parse_review_answer("YES, it matches the task.", "reviewer-1");
        assert!(review.ok);
        assert_eq!(review.reason, "it matches the task.");
        assert_eq!(review.reviewer, "reviewer-1");
    }

    #[test]
    fn parse_review_answer_no() {
        let review = parse_review_answer("no - missing tests", "reviewer-2");
        assert!(!review.ok);
        assert_eq!(review.reason, "missing tests");
    }

    #[test]
    fn parse_review_answer_empty() {
        let review = parse_review_answer("", "reviewer-3");
        assert!(!review.ok);
        assert_eq!(review.reason, "");
    }

    #[test]
    fn parse_review_answer_long_reason_truncated() {
        let long_reason = "a".repeat(400);
        let review = parse_review_answer(&format!("YES {}", long_reason), "reviewer-4");
        assert!(review.ok);
        assert_eq!(review.reason.len(), 300);
    }

    #[test]
    fn parse_review_answer_case_insensitive() {
        let review = parse_review_answer("yes it works", "r");
        assert!(review.ok);
        let review = parse_review_answer("Yes it works", "r");
        assert!(review.ok);
        let review = parse_review_answer("No it fails", "r");
        assert!(!review.ok);
    }

    #[test]
    fn review_prompt_cuts_huge_diff() {
        let huge_diff = "a".repeat(100_000);
        let request = ReviewRequest {
            task_id: TaskId::new("T-1"),
            title: "Test Task".into(),
            body: "Do something".into(),
            diff: huge_diff,
        };

        let prompt = review_prompt(&request, MAX_DIFF_BYTES_DEFAULT);
        assert!(prompt.contains("[diff cut]"));
        assert!(prompt.len() < request.diff.len() + 1000); // prompt overhead
    }

    #[test]
    fn review_prompt_contains_required_parts() {
        let request = ReviewRequest {
            task_id: TaskId::new("T-1"),
            title: "Add feature".into(),
            body: "Implement the thing".into(),
            diff: "+ fn new() {}".into(),
        };

        let prompt = review_prompt(&request, 1000);
        assert!(prompt.contains("Add feature"));
        assert!(prompt.contains("Implement the thing"));
        assert!(prompt.contains("+ fn new() {}"));
        assert!(prompt.contains("YES or NO"));
    }

    #[test]
    fn review_prompt_no_hidden_scope() {
        // The prompt should only contain title, body, and diff - no hidden fields
        let request = ReviewRequest {
            task_id: TaskId::new("T-1"),
            title: "Title".into(),
            body: "Body".into(),
            diff: "diff".into(),
        };

        let prompt = review_prompt(&request, 1000);
        let expected_parts = ["Title", "Body", "diff", "YES or NO"];
        for part in expected_parts {
            assert!(prompt.contains(part), "missing: {}", part);
        }
        // No extra fields
        assert!(!prompt.contains("scope"));
        assert!(!prompt.contains("hidden"));
        assert!(!prompt.contains("backlog"));
    }
}
