//! Quality notes: language-neutral heuristics on agent diffs (warnings only).

use sentinel_core::ids::TaskId;
use sentinel_core::quality::{QualityKind, QualityNote};

/// Produces quality notes for a task's diff.
///
/// Only examines ADDED lines of a unified diff (never removed or context lines).
/// Each rule produces one QualityNote with a stable detail_code and location as `path:line`.
pub fn quality_notes(task: &TaskId, diff: &str) -> Vec<QualityNote> {
    let mut notes = Vec::new();

    // Parse the diff into hunks with file paths and line numbers
    let hunks = parse_diff_hunks(diff);

    for hunk in &hunks {
        // long_added_block: 41+ consecutive added non-blank lines in one hunk
        if let Some(note) = check_long_added_block(task, hunk) {
            notes.push(note);
        }

        // long_file: file ends past line 400 (hunk header +start,count with start+count-1 > 400)
        if let Some(note) = check_long_file(task, hunk) {
            notes.push(note);
        }

        // duplicate_block: 6+ added non-blank lines (at least 60 chars) appearing twice
        if let Some(note) = check_duplicate_block(task, hunk) {
            notes.push(note);
        }

        // placeholder: added line with todo!(, unimplemented!(, TODO not followed by (ID), or FIXME
        if let Some(note) = check_placeholder(task, hunk) {
            notes.push(note);
        }

        // filler_comment: added comment line that only repeats the next added line
        if let Some(note) = check_filler_comment(task, hunk) {
            notes.push(note);
        }

        // test_without_assertion: added test function with no assert/expect/panic!/should
        if let Some(note) = check_test_without_assertion(task, hunk) {
            notes.push(note);
        }
    }

    notes
}

#[derive(Debug)]
struct DiffHunk {
    file_path: String,
    new_start: usize,
    new_count: usize,
    added_lines: Vec<AddedLine>,
}

#[derive(Debug, Clone)]
struct AddedLine {
    line_number: usize, // Line number in the new file
    content: String,
}

fn parse_diff_hunks(diff: &str) -> Vec<DiffHunk> {
    let mut hunks = Vec::new();
    let mut current_file = String::new();
    let mut in_hunk = false;
    let mut hunk_new_start = 0;
    let mut hunk_new_count = 0;
    let mut added_lines = Vec::new();
    let mut new_line_number = 0;

    for line in diff.lines() {
        // File header: +++ b/path
        if let Some(path) = line.strip_prefix("+++ b/") {
            current_file = path.to_string();
            continue;
        }

        // Hunk header: @@ -old_start,old_count +new_start,new_count @@
        if line.starts_with("@@ ") {
            // Save previous hunk
            if in_hunk && !added_lines.is_empty() {
                hunks.push(DiffHunk {
                    file_path: current_file.clone(),
                    new_start: hunk_new_start,
                    new_count: hunk_new_count,
                    added_lines: added_lines.clone(),
                });
            }

            // Parse new hunk header
            in_hunk = true;
            added_lines.clear();
            new_line_number = 0;

            // Parse @@ -a,b +c,d @@
            if let Some(header) = line.strip_prefix("@@ ")
                && let Some(plus_part) = header.split(' ').find(|p| p.starts_with('+'))
            {
                // Parse +start,count
                let plus_part = plus_part.trim_start_matches('+');
                let parts: Vec<&str> = plus_part.split(',').collect();
                hunk_new_start = parts[0].parse().unwrap_or(0);
                hunk_new_count = if parts.len() > 1 {
                    parts[1].parse().unwrap_or(0)
                } else {
                    1
                };
                new_line_number = hunk_new_start;
            }
            continue;
        }

        if !in_hunk {
            continue;
        }

        // Track line numbers in the new file
        if line.starts_with('+') && !line.starts_with("+++") {
            // Added line
            added_lines.push(AddedLine {
                line_number: new_line_number,
                content: line[1..].to_string(), // Remove the +
            });
            new_line_number += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            // Removed line - doesn't advance new file line number
        } else if line.starts_with(' ') {
            // Context line
            new_line_number += 1;
        }
    }

    // Save last hunk
    if in_hunk && !added_lines.is_empty() {
        hunks.push(DiffHunk {
            file_path: current_file,
            new_start: hunk_new_start,
            new_count: hunk_new_count,
            added_lines,
        });
    }

    hunks
}

fn check_long_added_block(task: &TaskId, hunk: &DiffHunk) -> Option<QualityNote> {
    let mut consecutive = 0;
    let mut start_line = 0;

    for added in &hunk.added_lines {
        if !added.content.trim().is_empty() {
            if consecutive == 0 {
                start_line = added.line_number;
            }
            consecutive += 1;
            if consecutive >= 41 {
                return Some(QualityNote {
                    task: task.clone(),
                    kind: QualityKind::Size,
                    location: format!("{}:{}", hunk.file_path, start_line),
                    detail_code: "long_added_block".to_string(),
                });
            }
        } else {
            consecutive = 0;
        }
    }
    None
}

fn check_long_file(task: &TaskId, hunk: &DiffHunk) -> Option<QualityNote> {
    // File ends past line 400: hunk header +start,count with start+count-1 > 400
    if hunk.new_start + hunk.new_count.saturating_sub(1) > 400 {
        // Location is the last line of the hunk in the new file
        let last_line = hunk.new_start + hunk.new_count.saturating_sub(1);
        return Some(QualityNote {
            task: task.clone(),
            kind: QualityKind::Size,
            location: format!("{}:{}", hunk.file_path, last_line),
            detail_code: "long_file".to_string(),
        });
    }
    None
}

fn check_duplicate_block(task: &TaskId, hunk: &DiffHunk) -> Option<QualityNote> {
    // Find runs of 6+ added non-blank lines (at least 60 chars total) that appear twice
    let mut blocks: Vec<(usize, String)> = Vec::new(); // (start_line, content)

    for window in hunk.added_lines.windows(6) {
        let non_blank: Vec<_> = window
            .iter()
            .filter(|a| !a.content.trim().is_empty())
            .collect();
        if non_blank.len() == 6 {
            let total_chars: usize = non_blank.iter().map(|a| a.content.len()).sum();
            if total_chars >= 60 {
                let content: String = non_blank
                    .iter()
                    .map(|a| a.content.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                blocks.push((non_blank[0].line_number, content));
            }
        }
    }

    // Check for duplicates
    for i in 0..blocks.len() {
        for j in i + 1..blocks.len() {
            if blocks[i].1 == blocks[j].1 {
                return Some(QualityNote {
                    task: task.clone(),
                    kind: QualityKind::Duplication,
                    location: format!("{}:{}", hunk.file_path, blocks[i].0),
                    detail_code: "duplicate_block".to_string(),
                });
            }
        }
    }
    None
}

fn check_placeholder(task: &TaskId, hunk: &DiffHunk) -> Option<QualityNote> {
    for added in &hunk.added_lines {
        let content = added.content.trim();
        let lower = content.to_lowercase();

        // todo!( or unimplemented!(
        if content.contains("todo!") || content.contains("unimplemented!") {
            return Some(QualityNote {
                task: task.clone(),
                kind: QualityKind::Slop,
                location: format!("{}:{}", hunk.file_path, added.line_number),
                detail_code: "placeholder".to_string(),
            });
        }

        // TODO not followed by (ID) pattern like TODO(P3-05)
        if lower.contains("todo") {
            // Check if it's followed by (ID) pattern
            if let Some(idx) = lower.find("todo") {
                let after = &lower[idx + 4..];
                let after = after.trim_start();
                if !after.starts_with('(')
                    || !after.chars().nth(1).is_some_and(|c| c.is_ascii_alphabetic())
                {
                    return Some(QualityNote {
                        task: task.clone(),
                        kind: QualityKind::Slop,
                        location: format!("{}:{}", hunk.file_path, added.line_number),
                        detail_code: "placeholder".to_string(),
                    });
                }
            }
        }

        // FIXME
        if lower.contains("fixme") {
            return Some(QualityNote {
                task: task.clone(),
                kind: QualityKind::Slop,
                location: format!("{}:{}", hunk.file_path, added.line_number),
                detail_code: "placeholder".to_string(),
            });
        }
    }
    None
}

fn check_filler_comment(task: &TaskId, hunk: &DiffHunk) -> Option<QualityNote> {
    for i in 0..hunk.added_lines.len().saturating_sub(1) {
        let current = &hunk.added_lines[i];
        let next = &hunk.added_lines[i + 1];

        // Check if current is a comment line
        let current_trimmed = current.content.trim();
        if !is_comment_line(current_trimmed) {
            continue;
        }

        // Compare comment text (lowercased, stripped of punctuation) with next code line
        let comment_text = normalize_for_comparison(current_trimmed);
        let next_text = normalize_for_comparison(next.content.trim());

        if comment_text == next_text || next_text.contains(&comment_text) {
            return Some(QualityNote {
                task: task.clone(),
                kind: QualityKind::Slop,
                location: format!("{}:{}", hunk.file_path, current.line_number),
                detail_code: "filler_comment".to_string(),
            });
        }
    }
    None
}

fn is_comment_line(line: &str) -> bool {
    let line = line.trim_start();
    // Common comment prefixes
    line.starts_with("//")
        || line.starts_with("#")
        || line.starts_with("/*")
        || line.starts_with("*")
        || line.starts_with("<!--")
        || line.starts_with("-->")
}

fn normalize_for_comparison(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn check_test_without_assertion(task: &TaskId, hunk: &DiffHunk) -> Option<QualityNote> {
    // Find added test functions and check their bodies for assertions
    let mut in_test_function = false;
    let mut test_start_line = 0;
    let mut has_assertion = false;
    let mut test_indent = 0;

    for added in &hunk.added_lines {
        let line = added.content.trim();

        // Detect test function start
        let is_test_start = line.starts_with("#[test]")
            || line.starts_with("#[tokio::test]")
            || line.starts_with("def test_")
            || line.starts_with("func Test")
            || line.starts_with("@Test")
            || (line.starts_with("fn ") && line.contains("test_"))
            || (line.starts_with("async fn ") && line.contains("test_"));

        if is_test_start {
            in_test_function = true;
            test_start_line = added.line_number;
            has_assertion = false;
            // Estimate indentation
            test_indent = added.content.len() - added.content.trim_start().len();
            continue;
        }

        if in_test_function {
            // Check for assertion keywords
            let lower = line.to_lowercase();
            if lower.contains("assert")
                || lower.contains("expect(")
                || lower.contains("panic!")
                || lower.contains("should")
            {
                has_assertion = true;
            }

            // Detect end of function (simplified: next non-indented line or end of hunk)
            let current_indent = added.content.len() - added.content.trim_start().len();
            if current_indent <= test_indent
                && !line.is_empty()
                && added.line_number > test_start_line
            {
                // Function ended
                if !has_assertion {
                    return Some(QualityNote {
                        task: task.clone(),
                        kind: QualityKind::Slop,
                        location: format!("{}:{}", hunk.file_path, test_start_line),
                        detail_code: "test_without_assertion".to_string(),
                    });
                }
                in_test_function = false;
            }
        }
    }

    // Check at end of hunk
    if in_test_function && !has_assertion {
        return Some(QualityNote {
            task: task.clone(),
            kind: QualityKind::Slop,
            location: format!("{}:{}", hunk.file_path, test_start_line),
            detail_code: "test_without_assertion".to_string(),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::ids::TaskId;
    use sentinel_core::quality::QualityKind;

    fn task_id() -> TaskId {
        TaskId::new("T-1")
    }

    #[test]
    fn quality_notes_long_added_block() {
        // 41 consecutive non-blank added lines
        let mut diff = String::from("+++ b/src/lib.rs\n@@ -0,0 +1,41 @@\n");
        for i in 1..=41 {
            diff.push_str(&format!("+fn foo{}() {{}}\n", i));
        }

        let notes = quality_notes(&task_id(), &diff);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].kind, QualityKind::Size);
        assert_eq!(notes[0].detail_code, "long_added_block");
    }

    #[test]
    fn quality_notes_long_added_block_exactly_40_no_note() {
        let mut diff = String::from("+++ b/src/lib.rs\n@@ -0,0 +1,40 @@\n");
        for i in 1..=40 {
            diff.push_str(&format!("+fn foo{}() {{}}\n", i));
        }

        let notes = quality_notes(&task_id(), &diff);
        assert!(notes.iter().all(|n| n.detail_code != "long_added_block"));
    }

    #[test]
    fn quality_notes_long_file() {
        // Hunk shows file ends past line 400
        let diff = "+++ b/src/lib.rs\n@@ -390,20 +390,20 @@\n+fn foo() {}\n".to_string();

        let notes = quality_notes(&task_id(), &diff);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].kind, QualityKind::Size);
        assert_eq!(notes[0].detail_code, "long_file");
    }

    #[test]
    fn quality_notes_long_file_under_400_no_note() {
        let diff = "+++ b/src/lib.rs\n@@ -10,20 +10,20 @@\n+fn foo() {}\n".to_string();

        let notes = quality_notes(&task_id(), &diff);
        assert!(notes.iter().all(|n| n.detail_code != "long_file"));
    }

    #[test]
    fn quality_notes_duplicate_block() {
        // Two identical blocks of 6 lines, 60+ chars
        let block = "+fn alpha_one() {}\n+fn beta_two() {}\n+fn gamma_three() {}\n+fn delta_four() {}\n+fn epsilon_five() {}\n+fn zeta_six() {}\n";
        let diff = format!("+++ b/src/lib.rs\n@@ -0,0 +1,12 @@\n{}{}", block, block);

        let notes = quality_notes(&task_id(), &diff);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].kind, QualityKind::Duplication);
        assert_eq!(notes[0].detail_code, "duplicate_block");
    }

    #[test]
    fn quality_notes_duplicate_block_too_small_no_note() {
        // 5 lines only
        let block = "+fn a() {}\n+fn b() {}\n+fn c() {}\n+fn d() {}\n+fn e() {}\n";
        let diff = format!("+++ b/src/lib.rs\n@@ -0,0 +1,10 @@\n{}{}", block, block);

        let notes = quality_notes(&task_id(), &diff);
        assert!(notes.iter().all(|n| n.detail_code != "duplicate_block"));
    }

    #[test]
    fn quality_notes_placeholder_todo_macro() {
        let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,1 @@\n+todo!()\n".to_string();
        let notes = quality_notes(&task_id(), &diff);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].detail_code, "placeholder");
    }

    #[test]
    fn quality_notes_placeholder_unimplemented_macro() {
        let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,1 @@\n+unimplemented!()\n".to_string();
        let notes = quality_notes(&task_id(), &diff);
        assert_eq!(notes[0].detail_code, "placeholder");
    }

    #[test]
    fn quality_notes_placeholder_todo_no_id() {
        let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,1 @@\n+// TODO: fix this\n".to_string();
        let notes = quality_notes(&task_id(), &diff);
        assert_eq!(notes[0].detail_code, "placeholder");
    }

    #[test]
    fn quality_notes_placeholder_todo_with_id_no_note() {
        let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,1 @@\n+// TODO(P3-05): fix this\n".to_string();
        let notes = quality_notes(&task_id(), &diff);
        assert!(notes.iter().all(|n| n.detail_code != "placeholder"));
    }

    #[test]
    fn quality_notes_placeholder_fixme() {
        let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,1 @@\n+// FIXME: broken\n".to_string();
        let notes = quality_notes(&task_id(), &diff);
        assert_eq!(notes[0].detail_code, "placeholder");
    }

    #[test]
    fn quality_notes_filler_comment() {
        let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,2 @@\n+// addfoo\n+fn add_foo() {}\n".to_string();
        let notes = quality_notes(&task_id(), &diff);
        assert_eq!(notes[0].detail_code, "filler_comment");
    }

    #[test]
    fn quality_notes_filler_comment_not_triggered_on_different() {
        let diff =
            "+++ b/src/lib.rs\n@@ -0,0 +1,2 @@\n+// this adds foo\n+fn add_bar() {}\n".to_string();
        let notes = quality_notes(&task_id(), &diff);
        assert!(notes.iter().all(|n| n.detail_code != "filler_comment"));
    }

    #[test]
    fn quality_notes_test_without_assertion_rust() {
        let diff =
            "+++ b/src/lib.rs\n@@ -0,0 +1,3 @@\n+#[test]\n+fn test_foo() {}\n+}\n".to_string();
        let notes = quality_notes(&task_id(), &diff);
        assert_eq!(notes[0].detail_code, "test_without_assertion");
    }

    #[test]
    fn quality_notes_test_with_assertion_no_note() {
        let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,4 @@\n+#[test]\n+fn test_foo() {\n+    assert!(true);\n+}\n".to_string();
        let notes = quality_notes(&task_id(), &diff);
        assert!(
            notes
                .iter()
                .all(|n| n.detail_code != "test_without_assertion")
        );
    }

    #[test]
    fn quality_notes_ignores_removed_lines() {
        // Removed lines should not trigger any notes
        let diff = "+++ b/src/lib.rs\n@@ -1,2 +1,1 @@\n-todo!()\n fn foo() {}\n".to_string();
        let notes = quality_notes(&task_id(), &diff);
        assert!(notes.is_empty());
    }

    #[test]
    fn quality_notes_clean_diff_no_notes() {
        let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,2 @@\n+fn foo() {}\n+fn bar() {}\n".to_string();
        let notes = quality_notes(&task_id(), &diff);
        assert!(notes.is_empty());
    }

    #[test]
    fn quality_notes_multiple_rules() {
        let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,50 @@\n+// TODO: fix\n+fn func_a() {}\n+fn func_b() {}\n+fn func_c() {}\n+fn func_d() {}\n+fn func_e() {}\n+fn func_f() {}\n+fn func_a() {}\n+fn func_b() {}\n+fn func_c() {}\n+fn func_d() {}\n+fn func_e() {}\n+fn func_f() {}\n".to_string();
        let notes = quality_notes(&task_id(), &diff);
        // Should have: placeholder (TODO), duplicate_block, maybe long_added_block
        assert!(notes.iter().any(|n| n.detail_code == "placeholder"));
        assert!(notes.iter().any(|n| n.detail_code == "duplicate_block"));
    }
}
