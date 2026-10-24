//! Backlog parser: reads a markdown backlog and extracts tasks.

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
    let mut ids = IdAllocator::default();

    let mut i = first_task_line(&lines);
    while i < lines.len() {
        let Some(raw) = task_heading(lines[i]) else {
            i += 1;
            continue;
        };

        let (explicit_id, title) = extract_id_and_title(raw);
        let id = match explicit_id {
            Some(id) => ids.explicit(id)?,
            None => ids.auto(),
        };

        let (body, next) = collect_body(&lines, i + 1);
        i = next;
        let title = title.trim().to_string();
        let claims_done = check_claims_done(&title, &body);

        items.push(BacklogItem {
            id,
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

/// Skip a single top-level `#` title line if present.
fn first_task_line(lines: &[&str]) -> usize {
    match lines.first() {
        Some(first) if first.starts_with("# ") => 1,
        _ => 0,
    }
}

/// Hands out task ids: explicit ids must be unique, missing ids get `T-XX`.
#[derive(Default)]
struct IdAllocator {
    used: HashSet<String>,
    auto_counter: usize,
}

impl IdAllocator {
    fn explicit(&mut self, id: String) -> Result<TaskId> {
        if !self.used.insert(id.clone()) {
            return Err(PlanError::Parse(format!("duplicate task id: {id}")));
        }
        Ok(TaskId::new(id))
    }

    fn auto(&mut self) -> TaskId {
        loop {
            self.auto_counter += 1;
            let candidate = format!("T-{:02}", self.auto_counter);
            if self.used.insert(candidate.clone()) {
                return TaskId::new(candidate);
            }
        }
    }
}

/// Collects body lines from `start` up to the next task heading.
/// A heading inside a code fence is body text. Returns the trimmed body and
/// the index of the line that ends it.
fn collect_body(lines: &[&str], start: usize) -> (String, usize) {
    let mut body_lines = Vec::new();
    let mut in_code_fence = false;
    let mut i = start;
    while i < lines.len() {
        let line = lines[i];
        if !in_code_fence && task_heading(line).is_some() {
            break;
        }
        if line.starts_with("```") {
            in_code_fence = !in_code_fence;
        }
        body_lines.push(line);
        i += 1;
    }
    (body_lines.join("\n").trim().to_string(), i)
}

/// Returns the heading text when the line is a `##` or `###` task heading.
fn task_heading(line: &str) -> Option<&str> {
    match heading_info(line)? {
        (2 | 3, raw) => Some(raw),
        _ => None,
    }
}

/// Returns `(level, rest_of_line)` for a heading line, or `None` if not a heading.
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

/// Length of a task id at the start of `text`, shaped `letters-digits`.
/// Returns None when the text does not start with that shape.
fn id_prefix(text: &str) -> Option<&str> {
    let letters = text.bytes().take_while(u8::is_ascii_alphabetic).count();
    if letters == 0 {
        return None;
    }
    let after_letters = text[letters..].strip_prefix('-')?;
    let digits = after_letters.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    Some(&text[..letters + 1 + digits])
}

/// Extract task id from heading text.
/// Heading like "T-04 Add a rate limit" or "T-04: Add a rate limit"
fn extract_id_and_title(raw: &str) -> (Option<String>, String) {
    let trimmed = raw.trim_start();
    let Some(id) = id_prefix(trimmed) else {
        return (None, trimmed.to_string());
    };
    let rest = trimmed[id.len()..].trim_start();
    // Strip leading colon if present
    let title = rest.strip_prefix(':').unwrap_or(rest).trim().to_string();
    (Some(id.to_string()), title)
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
