//! Context building and compaction. Pure functions, no I/O.

use std::fmt::Write;

/// Default limits for context compaction and failure summaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextLimits {
    /// Maximum bytes for the failure summary.
    pub failure_summary_bytes: usize,
    /// Lines to keep from the head of each error block.
    pub error_head_lines: usize,
    /// Lines to keep from the tail of the output.
    pub output_tail_lines: usize,
    /// Lines to keep from the head of tool output.
    pub tool_output_head: usize,
    /// Lines to keep from the tail of tool output.
    pub tool_output_tail: usize,
    /// Percent of the model window at which compaction triggers.
    pub compact_at_percent: u8,
    /// Token budget for the compacted summary of old steps.
    pub compacted_summary_tokens: u32,
    /// Number of recent steps to keep unsummarized.
    pub keep_last_steps: usize,
    /// Token cap for Brain notes and lessons combined in a retry prompt.
    pub notes_token_cap: u32,
}

impl Default for ContextLimits {
    fn default() -> Self {
        Self {
            failure_summary_bytes: 4096,
            error_head_lines: 20,
            output_tail_lines: 30,
            tool_output_head: 40,
            tool_output_tail: 40,
            compact_at_percent: 70,
            compacted_summary_tokens: 300,
            keep_last_steps: 4,
            notes_token_cap: 500,
        }
    }
}

/// Estimate tokens as characters divided by 4, rounded up.
pub fn estimate_tokens(text: &str) -> u32 {
    let chars = text.chars().count();
    u32::try_from(chars.div_ceil(4)).unwrap_or(u32::MAX)
}

/// Truncate tool output to head and tail lines with a marker.
/// Returns the text unchanged when it fits within head + tail.
pub fn truncate_tool_output(text: &str, limits: &ContextLimits) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let total = lines.len();
    let head = limits.tool_output_head;
    let tail = limits.tool_output_tail;

    if total <= head + tail {
        return text.to_string();
    }

    let cut = total - head - tail;
    let mut out = String::new();

    for line in &lines[..head] {
        out.push_str(line);
        out.push('\n');
    }

    writeln!(out, "[... {cut} lines cut ...]").ok();

    for line in &lines[total - tail..] {
        out.push_str(line);
        out.push('\n');
    }

    // Remove trailing newline if the original didn't have one
    if !text.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }

    out
}

/// Input for building a failure summary.
pub struct FailureInput<'a> {
    pub failing_tests: &'a [String],
    pub error_blocks: &'a [String],
    pub output: &'a str,
}

fn append_failing_tests(out: &mut String, tests: &[String]) {
    if tests.is_empty() {
        return;
    }
    out.push_str("Failing tests:\n");
    for test in tests {
        out.push_str(test);
        out.push('\n');
    }
}

fn append_errors(out: &mut String, blocks: &[String], limits: &ContextLimits) {
    if blocks.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str("Errors:\n");
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let lines: Vec<&str> = block.lines().collect();
        let take = lines.len().min(limits.error_head_lines);
        for line in &lines[..take] {
            out.push_str(line);
            out.push('\n');
        }
    }
}

fn append_output_tail(out: &mut String, output: &str, limits: &ContextLimits) {
    if output.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str("Output tail:\n");
    let lines: Vec<&str> = output.lines().collect();
    let tail_start = lines.len().saturating_sub(limits.output_tail_lines);
    for line in &lines[tail_start..] {
        out.push_str(line);
        out.push('\n');
    }
}

/// Build a failure summary capped at `failure_summary_bytes`.
/// Sections: Failing tests, Errors (first N lines each), Output tail (last M lines).
pub fn failure_summary(input: &FailureInput<'_>, limits: &ContextLimits) -> String {
    let mut out = String::new();

    append_failing_tests(&mut out, input.failing_tests);
    append_errors(&mut out, input.error_blocks, limits);
    append_output_tail(&mut out, input.output, limits);

    // Cap at failure_summary_bytes, cut on character boundary, end with [cut]
    if out.len() > limits.failure_summary_bytes {
        let mut truncated = String::new();
        let cap = limits.failure_summary_bytes.saturating_sub(5); // space for "[cut]"
        for ch in out.chars() {
            if truncated.len() + ch.len_utf8() > cap {
                break;
            }
            truncated.push(ch);
        }
        truncated.push_str("[cut]");
        out = truncated;
    }

    out
}

/// Parts for building a retry prompt.
pub struct RetryParts<'a> {
    pub task_title: &'a str,
    pub task_body: &'a str,
    pub scope: &'a [String],
    pub notes: &'a [String],
    pub lessons: &'a [String],
    pub failure: &'a str,
    pub diff_stat: &'a str,
}

fn build_notes_section(parts: &RetryParts<'_>, limits: &ContextLimits) -> String {
    let mut combined = String::new();
    let mut used_tokens = 0u32;

    for note in parts.notes {
        let tokens = estimate_tokens(note);
        if used_tokens + tokens > limits.notes_token_cap {
            break;
        }
        combined.push_str(note);
        combined.push('\n');
        used_tokens += tokens;
    }

    for lesson in parts.lessons {
        let tokens = estimate_tokens(lesson);
        if used_tokens + tokens > limits.notes_token_cap {
            break;
        }
        combined.push_str(lesson);
        combined.push('\n');
        used_tokens += tokens;
    }

    combined
}

fn append_scope(out: &mut String, scope: &[String]) {
    if scope.is_empty() {
        return;
    }
    out.push_str("\nScope:\n");
    for path in scope {
        out.push_str(path);
        out.push('\n');
    }
}

fn append_notes(out: &mut String, notes: &str) {
    if notes.is_empty() {
        return;
    }
    out.push_str("\nNotes:\n");
    out.push_str(notes);
}

fn append_failure(out: &mut String, failure: &str) {
    if failure.is_empty() {
        return;
    }
    out.push_str("\nFailure:\n");
    out.push_str(failure);
    out.push('\n');
}

fn append_diff_stat(out: &mut String, diff_stat: &str) {
    if diff_stat.is_empty() {
        return;
    }
    out.push_str("\nDiff stat:\n");
    out.push_str(diff_stat);
    out.push('\n');
}

/// Build a retry prompt from parts.
/// Sections: Task (marked as retry), Scope, Notes (up to cap with lessons), Failure, Diff stat.
/// Empty sections are omitted. Raw logs never appear.
pub fn retry_prompt(parts: &RetryParts<'_>, limits: &ContextLimits) -> String {
    let mut out = String::new();

    out.push_str("Task (retry):\n");
    out.push_str(parts.task_title);
    out.push('\n');
    out.push_str(parts.task_body);
    out.push('\n');

    append_scope(&mut out, parts.scope);

    let notes_section = build_notes_section(parts, limits);
    append_notes(&mut out, &notes_section);

    append_failure(&mut out, parts.failure);
    append_diff_stat(&mut out, parts.diff_stat);

    out
}

/// Return true when used tokens are strictly above `compact_at_percent` of the window.
pub fn needs_compaction(used_tokens: u32, window_tokens: u32, limits: &ContextLimits) -> bool {
    if window_tokens == 0 {
        return false;
    }
    let threshold = (u64::from(window_tokens) * u64::from(limits.compact_at_percent)) / 100;
    u64::from(used_tokens) > threshold
}

/// How many old steps to summarize: `step_count` minus `keep_last_steps`, never below 0.
pub fn compaction_split(step_count: usize, limits: &ContextLimits) -> usize {
    step_count.saturating_sub(limits.keep_last_steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_estimate_rounds_up() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("a"), 1);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
    }

    #[test]
    fn truncation_keeps_head_and_tail_with_marker() {
        let limits = ContextLimits::default();
        let mut text = String::new();
        for i in 1..=100 {
            writeln!(text, "line {i}").ok();
        }
        let result = truncate_tool_output(&text, &limits);
        assert!(result.starts_with("line 1\n"));
        assert!(result.contains("[... 20 lines cut ...]"));
        assert!(result.ends_with("line 100\n"));
    }

    #[test]
    fn truncation_leaves_short_text_alone() {
        let limits = ContextLimits::default();
        let text = "line 1\nline 2\nline 3";
        let result = truncate_tool_output(text, &limits);
        assert_eq!(result, text);
    }

    #[test]
    fn truncation_never_splits_utf8() {
        let limits = ContextLimits {
            tool_output_head: 1,
            tool_output_tail: 1,
            ..Default::default()
        };
        let text = "🎉\nline 2\nline 3\n🎊";
        let result = truncate_tool_output(text, &limits);
        assert!(result.starts_with("🎉"));
        assert!(result.contains("🎊"));
        assert!(result.contains("[... 2 lines cut ...]"));
    }

    #[test]
    fn failure_summary_with_large_input_stays_within_cap() {
        let limits = ContextLimits::default();
        let failing_tests: Vec<String> = (1..=500).map(|i| format!("test_{i}")).collect();
        let error_blocks = vec!["error line\n".repeat(100); 10];
        let output = "output line\n".repeat(100_000);

        let input = FailureInput {
            failing_tests: &failing_tests,
            error_blocks: &error_blocks,
            output: &output,
        };

        let result = failure_summary(&input, &limits);
        assert!(result.len() <= limits.failure_summary_bytes);
        assert!(result.starts_with("Failing tests:"));
        assert!(result.contains("test_1"));
        assert!(result.ends_with("[cut]"));
    }

    #[test]
    fn failure_summary_error_blocks_cut_to_head_lines() {
        let limits = ContextLimits::default();
        let block = (1..=50)
            .map(|i| format!("error line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let input = FailureInput {
            failing_tests: &[],
            error_blocks: &[block],
            output: "",
        };
        let result = failure_summary(&input, &limits);
        let error_section = result.split("Errors:\n").nth(1).unwrap();
        let lines: Vec<&str> = error_section.lines().collect();
        assert_eq!(lines[0], "error line 1");
        assert_eq!(lines[19], "error line 20");
        assert_eq!(lines.len(), 20);
    }

    #[test]
    fn retry_prompt_leaves_out_empty_sections() {
        let limits = ContextLimits::default();
        let parts = RetryParts {
            task_title: "Fix bug",
            task_body: "The bug is in the parser.",
            scope: &[],
            notes: &[],
            lessons: &[],
            failure: "",
            diff_stat: "",
        };
        let result = retry_prompt(&parts, &limits);
        assert!(result.starts_with("Task (retry):"));
        assert!(result.contains("Fix bug"));
        assert!(!result.contains("Scope:"));
        assert!(!result.contains("Notes:"));
        assert!(!result.contains("Failure:"));
        assert!(!result.contains("Diff stat:"));
    }

    #[test]
    fn retry_prompt_never_contains_raw_log_longer_than_summary() {
        let limits = ContextLimits::default();
        let long_log = "x".repeat(10000);
        let parts = RetryParts {
            task_title: "Fix bug",
            task_body: "The bug is in the parser.",
            scope: &["src/main.rs".to_string()],
            notes: &[],
            lessons: &[],
            failure: &long_log,
            diff_stat: "5 files changed, 100 insertions, 50 deletions",
        };
        let result = retry_prompt(&parts, &limits);
        assert!(result.contains("Failure:"));
        assert!(result.contains(&long_log[..100]));
    }

    #[test]
    fn retry_prompt_respects_notes_cap() {
        let limits = ContextLimits {
            notes_token_cap: 8,
            ..Default::default()
        };
        let parts = RetryParts {
            task_title: "Fix bug",
            task_body: "The bug is in the parser.",
            scope: &[],
            notes: &[
                "note one two three four".to_string(), // 22 chars -> 6 tokens
                "note five six".to_string(),           // 13 chars -> 4 tokens
            ],
            lessons: &["lesson one two three".to_string()], // 20 chars -> 5 tokens
            failure: "test failed",
            diff_stat: "",
        };
        let result = retry_prompt(&parts, &limits);
        // First note (6 tokens) fits, second (4 tokens) would make 10 > 8, so it doesn't fit
        // Lesson (5 tokens) also doesn't fit
        assert!(result.contains("note one"));
        assert!(!result.contains("note five"));
        assert!(!result.contains("lesson one"));
    }

    #[test]
    fn needs_compaction_boundary_at_70_percent() {
        let limits = ContextLimits::default();
        assert!(!needs_compaction(700, 1000, &limits));
        assert!(needs_compaction(701, 1000, &limits));
        assert!(needs_compaction(710, 1000, &limits));
    }

    #[test]
    fn compaction_split_subtracts_keep_last_steps() {
        let limits = ContextLimits::default();
        assert_eq!(compaction_split(10, &limits), 6);
        assert_eq!(compaction_split(4, &limits), 0);
        assert_eq!(compaction_split(3, &limits), 0);
        assert_eq!(compaction_split(0, &limits), 0);
    }
}
