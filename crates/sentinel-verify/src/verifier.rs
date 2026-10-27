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
/// Colour codes and blank lines are dropped first. The first line that starts with the word YES
/// or NO (any case) is the verdict: YES means ok. A tool may print a banner before the answer, so
/// the lines before the verdict are skipped. An answer with no such line is not ok, and its first
/// line is the reason. The reason is the rest of the verdict line, at most 300 chars.
pub fn parse_review_answer(text: &str, reviewer: &str) -> Review {
    let lines: Vec<String> = text
        .lines()
        .map(|line| strip_colour(line).trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect();
    let (ok, reason) = match lines.iter().find_map(|line| verdict_of(line)) {
        Some(found) => found,
        None => (false, lines.first().cloned().unwrap_or_default()),
    };
    Review {
        ok,
        reason: reason.chars().take(300).collect(),
        reviewer: reviewer.to_string(),
    }
}

/// True when the answer holds a line that starts with the word YES or NO. An answer without one is
/// not a rejection: the reviewer gave no verdict.
pub fn has_verdict(text: &str) -> bool {
    text.lines()
        .map(|line| strip_colour(line).trim().to_owned())
        .any(|line| verdict_of(&line).is_some())
}

/// `Some((ok, reason))` when the line starts with the word YES or NO.
fn verdict_of(line: &str) -> Option<(bool, String)> {
    let head: String = line
        .chars()
        .take_while(|ch| ch.is_alphabetic())
        .collect::<String>()
        .to_uppercase();
    let ok = match head.as_str() {
        "YES" => true,
        "NO" => false,
        _ => return None,
    };
    let rest = line[head.len()..]
        .trim_start_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace())
        .to_owned();
    Some((ok, rest))
}

/// Removes terminal colour codes, such as `ESC[0m`, and the bare `[0m` that is left when the
/// escape character was lost on the way.
fn strip_colour(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        let starts_code = ch == '\u{1b}' || (ch == '[' && is_bare_code(&mut chars.clone()));
        if !starts_code {
            out.push(ch);
            continue;
        }
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
        }
        while chars
            .next_if(|next| next.is_ascii_digit() || *next == ';')
            .is_some()
        {}
        chars.next_if(|next| next.is_ascii_alphabetic());
    }
    out
}

/// True when the characters after a `[` are digits and semicolons and then a letter, as in `[0m`.
fn is_bare_code(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    let mut digits = 0;
    while chars
        .next_if(|next| next.is_ascii_digit() || *next == ';')
        .is_some()
    {
        digits += 1;
    }
    digits > 0 && chars.next_if(char::is_ascii_alphabetic).is_some()
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
    fn an_answer_with_only_a_banner_has_no_verdict() {
        assert!(!has_verdict("\u{1b}[0m> build · glm-5.3-flash-free\n"));
        assert!(has_verdict("> banner\nNO missing test"));
    }

    #[test]
    fn a_banner_and_colour_codes_before_the_verdict_are_skipped() {
        let text = "\u{1b}[0m> build · glm-5.3-flash-free\n\u{1b}[0m\nYES the rounding is right.";
        let review = parse_review_answer(text, "opencode");
        assert!(review.ok);
        assert_eq!(review.reason, "the rounding is right.");
    }

    #[test]
    fn a_bare_colour_fragment_is_not_a_reason() {
        let review = parse_review_answer("[0m\n[0m", "opencode");
        assert!(!review.ok);
        assert_eq!(review.reason, "");
    }

    #[test]
    fn a_word_that_only_starts_with_no_is_not_a_verdict() {
        let review = parse_review_answer("Note: all fine\nNO the test is missing.", "r");
        assert!(!review.ok);
        assert_eq!(review.reason, "the test is missing.");
    }

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
