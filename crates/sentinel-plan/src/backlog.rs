//! Backlog parser: reads a markdown backlog and extracts tasks.

#![allow(clippy::too_many_lines)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::single_match_else)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::doc_markdown)]

use std::collections::HashSet;

use crate::{PlanError, Result};
use sentinel_core::TaskId;
use serde::{Deserialize, Serialize};

/// A single backlog item parsed from markdown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacklogItem {
    pub id: TaskId,
    pub title: String,
    pub body: String,
    pub claims_done: bool,
}

/// Parse a backlog markdown text into a list of backlog items.
pub fn parse_backlog(text: &str) -> Result<Vec<BacklogItem>> {
    let text = text.replace('\r', "");
    let lines: Vec<&str> = text.lines().collect();

    let mut items = Vec::new();
    let mut used_ids = HashSet::new();
    let mut auto_counter = 0;

    let mut i = 0;
    // Skip a single top-level `#` title line if present
    if let Some(first) = lines.first() {
        if first.starts_with("# ") && !first.starts_with("## ") {
            i = 1;
        }
    }

    while i < lines.len() {
        let line = lines[i];
        let (level, raw) = match heading_info(line) {
            Some((lvl, r)) => (lvl, r),
            None => {
                i += 1;
                continue;
            }
        };

        // Only ## and ### are task headings
        if level < 2 || level > 3 {
            i += 1;
            continue;
        }

        // Extract task id from heading if present
        let (task_id, title) = extract_id_and_title(raw);

        let task_id = if let Some(id) = task_id {
            if used_ids.contains(&id) {
                return Err(PlanError::Parse(format!("duplicate task id: {}", id)));
            }
            used_ids.insert(id.clone());
            TaskId::new(id)
        } else {
            // Auto-assign T-XX
            loop {
                auto_counter += 1;
                let candidate = format!("T-{:02}", auto_counter);
                if !used_ids.contains(&candidate) {
                    used_ids.insert(candidate.clone());
                    break TaskId::new(candidate);
                }
            }
        };

        // Collect body until next heading
        i += 1;
        let mut body_lines = Vec::new();
        let mut in_code_fence = false;
        while i < lines.len() {
            let l = lines[i];
            if let Some((lvl, _)) = heading_info(l) {
                if lvl >= 2 && lvl <= 3 && !in_code_fence {
                    break;
                }
            }
            // Track code fences
            if l.starts_with("```") {
                in_code_fence = !in_code_fence;
            }
            body_lines.push(l);
            i += 1;
        }

        let body = body_lines.join("\n").trim().to_string();
        let title = title.trim().to_string();

        // Check claims_done
        let claims_done = check_claims_done(&title, &body);

        items.push(BacklogItem {
            id: task_id,
            title,
            body,
            claims_done,
        });
    }

    if items.is_empty() {
        return Err(PlanError::Parse("no tasks".to_string()));
    }

    Ok(items)
}

/// Returns (level, rest_of_line) for a heading line, or None if not a heading.
fn heading_info(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }
    let mut level = 0;
    for ch in trimmed.chars() {
        if ch == '#' {
            level += 1;
        } else {
            break;
        }
    }
    if level == 0 || level > 6 {
        return None;
    }
    let rest = trimmed[level..].trim_start();
    Some((level, rest))
}

/// Extract task id from heading text.
/// Heading like "T-04 Add a rate limit" or "T-04: Add a rate limit"
fn extract_id_and_title(raw: &str) -> (Option<String>, String) {
    // Match pattern: letters, dash, digits at start
    let trimmed = raw.trim_start();

    // Try to match id pattern: letters, dash, digits
    let mut id_chars = Vec::new();
    let mut has_letter = false;
    let mut has_dash = false;
    let mut has_digit = false;

    for ch in trimmed.chars() {
        if ch.is_ascii_alphabetic() {
            if has_dash || has_digit {
                break;
            }
            has_letter = true;
            id_chars.push(ch);
        } else if ch == '-' {
            if has_dash || !has_letter {
                break;
            }
            has_dash = true;
            id_chars.push(ch);
        } else if ch.is_ascii_digit() {
            if !has_dash {
                break;
            }
            has_digit = true;
            id_chars.push(ch);
        } else {
            break;
        }
    }

    if has_letter && has_dash && has_digit {
        let id: String = id_chars.into_iter().collect();
        let rest = trimmed[id.len()..].trim_start();
        // Strip leading colon if present
        let title = rest.strip_prefix(':').unwrap_or(rest).trim().to_string();
        return (Some(id), title);
    }

    (None, trimmed.to_string())
}

/// Check if the task claims to be done (case-insensitive).
fn check_claims_done(title: &str, body: &str) -> bool {
    let text = format!("{} {}", title.to_lowercase(), body.to_lowercase());
    text.contains("[x]")
        || text.contains("(done)")
        || text.contains("already done")
        || text.contains("already implemented")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_explicit_ids() {
        let text = r"## T-04 Add a rate limit
This is the body.

### T-05: Add logging
More body text.
";
        let items = parse_backlog(text).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id.as_str(), "T-04");
        assert_eq!(items[0].title, "Add a rate limit");
        assert_eq!(items[1].id.as_str(), "T-05");
        assert_eq!(items[1].title, "Add logging");
    }

    #[test]
    fn auto_assigns_ids() {
        let text = r"## Add a rate limit
Body one.

## Add logging
Body two.
";
        let items = parse_backlog(text).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id.as_str(), "T-01");
        assert_eq!(items[1].id.as_str(), "T-02");
    }

    #[test]
    fn duplicate_explicit_ids_error() {
        let text = r"## T-04 Task one
Body.

## T-04 Task two
Body.
";
        let err = parse_backlog(text).unwrap_err();
        assert!(matches!(err, PlanError::Parse(s) if s.contains("duplicate")));
    }

    #[test]
    fn code_fence_hash_lines_not_headings() {
        let text = r"## T-01 Task with code
Here is code:

```rust
# This is not a heading
fn main() {}
```

### T-02 Next task
Body.
";
        let items = parse_backlog(text).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id.as_str(), "T-01");
        assert!(items[0].body.contains("# This is not a heading"));
        assert_eq!(items[1].id.as_str(), "T-02");
    }

    #[test]
    fn claims_done_variants() {
        let text = r"## T-01 [x] Done task
Body.

## T-02 (done) Also done
Body.

## T-03 Already done here
Body.

## T-04 Already implemented
Body.

## T-05 Not done
Body.
";
        let items = parse_backlog(text).unwrap();
        assert!(items[0].claims_done);
        assert!(items[1].claims_done);
        assert!(items[2].claims_done);
        assert!(items[3].claims_done);
        assert!(!items[4].claims_done);
    }

    #[test]
    fn empty_backlog_error() {
        let text = "# Title only\n\nNo tasks here.";
        let err = parse_backlog(text).unwrap_err();
        assert!(matches!(err, PlanError::Parse(s) if s == "no tasks"));
    }

    #[test]
    fn crlf_input() {
        let text = "## T-01 Task\r\nBody line 1.\r\n\r\n## T-02 Another\r\nBody line 2.\r\n";
        let items = parse_backlog(text).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].body, "Body line 1.");
        assert_eq!(items[1].body, "Body line 2.");
    }

    #[test]
    fn skips_top_level_title() {
        let text = "# My Backlog\n\n## T-01 First task\nBody.\n";
        let items = parse_backlog(text).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id.as_str(), "T-01");
    }

    #[test]
    fn trims_title_and_body() {
        let text = "## T-01   Trimmed title  \n\n  Body with spaces  \n";
        let items = parse_backlog(text).unwrap();
        assert_eq!(items[0].title, "Trimmed title");
        assert_eq!(items[0].body, "Body with spaces");
    }

    #[test]
    fn auto_ids_skip_used_numbers() {
        let text = r"## T-03 Explicit three
Body.

## Auto one
Body.

## Auto two
Body.
";
        let items = parse_backlog(text).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].id.as_str(), "T-03");
        assert_eq!(items[1].id.as_str(), "T-01");
        assert_eq!(items[2].id.as_str(), "T-02");
    }
}
