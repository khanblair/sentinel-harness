//! Duplicate check: copy-pasted blocks of code.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::files::{read_text, repo_files};
use crate::finding::{CheckResult, Finding};

const CHECK_NAME: &str = "dup-check";
const WINDOW_LINES: usize = 6;
const MIN_OCCURRENCES: usize = 3;
// Six short lines are routine boilerplate, so only wide windows count.
const MIN_WINDOW_CHARS: usize = 60;
const TRIVIAL_LINES: [&str; 7] = ["{", "}", "});", ")", ");", "];", "},"];

/// One kept line with its original 1-based line number.
#[derive(Debug)]
struct KeptLine {
    text: String,
    line: usize,
}

/// A window of kept lines. `slot` is the window index inside its file, which tells
/// windows of one copied region apart from windows of unrelated code.
#[derive(Debug)]
struct Window {
    text: String,
    file: PathBuf,
    start_line: usize,
    slot: usize,
}

pub fn run(root: &Path) -> CheckResult {
    let files = repo_files(root)?;
    Ok(duplicate_findings(&collect_windows(root, &files)))
}

fn collect_windows(root: &Path, files: &[PathBuf]) -> Vec<Window> {
    let mut windows = Vec::new();
    for rel in files {
        if skip_file(rel) {
            continue;
        }
        let Some(content) = read_text(&root.join(rel)) else {
            continue;
        };
        let kept = kept_lines(&content);
        windows.extend(windows_from(&kept, rel));
    }
    windows
}

fn skip_file(rel: &Path) -> bool {
    let is_rust = rel.extension().is_some_and(|ext| ext == "rs");
    let in_skipped_dir = rel
        .components()
        .any(|part| matches!(part.as_os_str().to_str(), Some("fixtures" | "target")));
    let name = rel.file_name().and_then(|n| n.to_str()).unwrap_or("");
    !is_rust || in_skipped_dir || name.ends_with("_test.rs")
}

fn windows_from(kept: &[KeptLine], file: &Path) -> Vec<Window> {
    let mut windows = Vec::new();
    for slot in 0..kept.len().saturating_sub(WINDOW_LINES - 1) {
        let run = &kept[slot..slot + WINDOW_LINES];
        let text = run
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if text.len() >= MIN_WINDOW_CHARS {
            windows.push(Window {
                text,
                file: file.to_path_buf(),
                start_line: run[0].line,
                slot,
            });
        }
    }
    windows
}

/// Kept lines in file order. A `#[cfg(test)]` module holds no product code, so its
/// lines never count. The heuristic ends the module at the first brace in column zero.
fn kept_lines(content: &str) -> Vec<KeptLine> {
    let mut kept = Vec::new();
    let mut in_test_module = false;
    for (idx, raw) in content.lines().enumerate() {
        let trimmed = raw.trim();
        if in_test_module {
            if raw.starts_with('}') {
                in_test_module = false;
            }
            continue;
        }
        if trimmed.starts_with("#[cfg(test)]") {
            in_test_module = true;
            continue;
        }
        if keep_line(trimmed) {
            kept.push(KeptLine {
                text: trimmed.to_owned(),
                line: idx + 1,
            });
        }
    }
    kept
}

fn keep_line(trimmed: &str) -> bool {
    !trimmed.is_empty()
        && !trimmed.starts_with("//")
        && !trimmed.starts_with("use ")
        && !TRIVIAL_LINES.contains(&trimmed)
}

/// Window text to its copy count, and to the earliest file and line it appears at.
struct Copies<'a> {
    count: HashMap<&'a str, usize>,
    first_at: HashMap<&'a str, (&'a Path, usize)>,
}

fn tally(windows: &[Window]) -> Copies<'_> {
    let mut copies = Copies {
        count: HashMap::new(),
        first_at: HashMap::new(),
    };
    for w in windows {
        *copies.count.entry(w.text.as_str()).or_default() += 1;
        let spot = (w.file.as_path(), w.start_line);
        copies
            .first_at
            .entry(w.text.as_str())
            .and_modify(|earliest| {
                if spot < *earliest {
                    *earliest = spot;
                }
            })
            .or_insert(spot);
    }
    copies
}

/// The windows that repeat enough, grouped per file. `repo_files` sorts paths, so all
/// windows of one file sit next to each other and in slot order.
fn copied_spots<'a>(
    windows: &'a [Window],
    counts: &HashMap<&str, usize>,
) -> Vec<(&'a Path, Vec<&'a Window>)> {
    let mut per_file: Vec<(&Path, Vec<&Window>)> = Vec::new();
    for w in windows
        .iter()
        .filter(|w| counts[w.text.as_str()] >= MIN_OCCURRENCES)
    {
        match per_file.last_mut() {
            Some((path, spots)) if *path == w.file.as_path() => spots.push(w),
            _ => per_file.push((w.file.as_path(), vec![w])),
        }
    }
    per_file
}

/// Windows that overlap or touch the previous one belong to one copied region.
fn merged_runs<'a>(spots: &[&'a Window]) -> Vec<Vec<&'a Window>> {
    let mut runs: Vec<Vec<&Window>> = Vec::new();
    for w in spots {
        match runs.last_mut() {
            Some(run)
                if run
                    .last()
                    .is_some_and(|last| w.slot <= last.slot + WINDOW_LINES) =>
            {
                run.push(w);
            }
            _ => runs.push(vec![*w]),
        }
    }
    runs
}

fn duplicate_findings(windows: &[Window]) -> Vec<Finding> {
    let copies = tally(windows);
    let mut found = Vec::new();
    for (file, spots) in copied_spots(windows, &copies.count) {
        for run in merged_runs(&spots) {
            let count = run
                .iter()
                .map(|w| copies.count[w.text.as_str()])
                .max()
                .unwrap_or_default();
            let first = copies.first_at[run[0].text.as_str()];
            let message = format!(
                "{count} copies of this block; first copy at {}:{}",
                first.0.display(),
                first.1
            );
            found.push(Finding::new(CHECK_NAME, file, run[0].start_line, message));
        }
    }
    found.sort_by_key(|f| (f.path.clone(), f.line));
    found
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    const EIGHT_LINE_BLOCK: &str = "\
fn process_item(item: Item) -> Result {
    let validated = validate(item)?;
    let transformed = transform(validated)?;
    let saved = save(transformed)?;
    let encoded = encode(saved)?;
    let outcome = finalize(encoded)?;
    Ok(outcome)
}
";

    fn write(dir: &Path, name: &str, body: &str) {
        let path = dir.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    fn findings_in(dir: &Path) -> Vec<Finding> {
        run(dir).unwrap()
    }

    #[test]
    fn three_copies_of_an_eight_line_block_report_each_copy() {
        let dir = tempdir().unwrap();
        for name in ["a.rs", "b.rs", "c.rs"] {
            write(dir.path(), name, EIGHT_LINE_BLOCK);
        }
        let found = findings_in(dir.path());
        assert_eq!(found.len(), 3, "findings: {found:?}");
        let wanted = "3 copies of this block; first copy at a.rs:1";
        assert!(
            found.iter().all(|f| f.message == wanted),
            "findings: {found:?}"
        );
    }

    #[test]
    fn two_copies_of_a_block_report_nothing() {
        let dir = tempdir().unwrap();
        write(dir.path(), "a.rs", EIGHT_LINE_BLOCK);
        write(dir.path(), "b.rs", EIGHT_LINE_BLOCK);
        let found = findings_in(dir.path());
        assert!(found.is_empty(), "findings: {found:?}");
    }

    #[test]
    fn six_short_lines_under_sixty_characters_report_nothing() {
        let dir = tempdir().unwrap();
        let block = "\
fn tiny() {
    let a=1;
    let b=2;
    let c=3;
    let d=4;
    let e=5;
}
";
        for name in ["a.rs", "b.rs", "c.rs"] {
            write(dir.path(), name, block);
        }
        let found = findings_in(dir.path());
        assert!(found.is_empty(), "findings: {found:?}");
    }

    #[test]
    fn overlapping_windows_of_one_region_report_one_finding_per_copy() {
        let dir = tempdir().unwrap();
        let block = "\
fn tally(rows: &[Row]) -> u64 {
    let first = rows.first();
    let second = rows.get(1);
    let third = rows.get(2);
    let count_a = first.len();
    let count_b = second.len();
    let count_c = third.len();
    count_a + count_b + count_c
}
";
        for name in ["a.rs", "b.rs", "c.rs"] {
            write(dir.path(), name, block);
        }
        let found = findings_in(dir.path());
        assert_eq!(found.len(), 3, "findings: {found:?}");
        assert!(found.iter().all(|f| f.line == 1), "findings: {found:?}");
    }

    #[test]
    fn comment_blank_and_brace_only_lines_do_not_break_a_match() {
        let dir = tempdir().unwrap();
        let clean = "\
fn scale(x: i32) -> i32 {
    let base = x * 2;
    let grown = base + 1;
    let fixed = grown * 3;
    let moved = fixed - 4;
    moved
}
";
        let noisy = "\
fn scale(x: i32) -> i32 {
    // double it first
    let base = x * 2;
});

    let grown = base + 1;
    let fixed = grown * 3;
    let moved = fixed - 4;
    moved
}
";
        write(dir.path(), "a.rs", clean);
        write(dir.path(), "b.rs", noisy);
        write(dir.path(), "c.rs", clean);
        let found = findings_in(dir.path());
        assert_eq!(found.len(), 3, "findings: {found:?}");
        let wanted = "3 copies of this block; first copy at a.rs:1";
        assert!(
            found.iter().all(|f| f.message == wanted),
            "findings: {found:?}"
        );
    }

    #[test]
    fn use_lines_inside_a_block_are_ignored() {
        let dir = tempdir().unwrap();
        let with_identity = "\
fn sum(values: &[i32]) -> i32 {
    let mut total = 0;
    use std::convert::identity;
    for v in values {
        total += *v;
    }
    let rounded = total;
    rounded
}
";
        let with_flip = "\
fn sum(values: &[i32]) -> i32 {
    let mut total = 0;
    use std::convert::absorb as flip;
    for v in values {
        total += *v;
    }
    let rounded = total;
    rounded
}
";
        write(dir.path(), "a.rs", with_identity);
        write(dir.path(), "b.rs", with_flip);
        write(dir.path(), "c.rs", with_identity);
        let found = findings_in(dir.path());
        assert_eq!(found.len(), 3, "findings: {found:?}");
    }

    #[test]
    fn fixtures_target_and_test_files_are_skipped() {
        let dir = tempdir().unwrap();
        write(dir.path(), "src/a.rs", EIGHT_LINE_BLOCK);
        write(dir.path(), "fixtures/b.rs", EIGHT_LINE_BLOCK);
        write(dir.path(), "src/c_test.rs", EIGHT_LINE_BLOCK);
        let found = findings_in(dir.path());
        assert!(found.is_empty(), "findings: {found:?}");
    }

    #[test]
    fn code_inside_a_cfg_test_module_is_skipped() {
        let dir = tempdir().unwrap();
        let unit = "\
#[cfg(test)]
mod tests {
    fn helper_block() -> i32 {
        let one = 1;
        let two = 2;
        let three = 3;
        let four = 4;
        let five = 5;
        one + two + three + four + five
    }
}
";
        for name in ["a.rs", "b.rs", "c.rs"] {
            write(dir.path(), name, unit);
        }
        let found = findings_in(dir.path());
        assert!(found.is_empty(), "findings: {found:?}");
    }
}
