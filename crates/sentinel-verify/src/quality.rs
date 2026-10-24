//! Quality notes on the lines an agent added: warnings about size, copy-paste, and filler. They
//! never change a verdict. The rules are language neutral and work on a unified diff, on added
//! lines only.

use std::collections::{BTreeSet, HashMap};

use sentinel_core::{QualityKind, QualityNote, TaskId};

/// Most added lines in a row before the block is called long.
const LONG_BLOCK_LINES: usize = 40;
/// A file longer than this after the change gets a note.
const LONG_FILE_LINES: usize = 400;
/// Lines in a copied block, and its smallest size in characters.
const DUPLICATE_LINES: usize = 6;
const DUPLICATE_CHARS: usize = 60;
/// Words that carry no meaning when a comment is compared with the next line of code.
const FILLER_WORDS: [&str; 8] = ["this", "the", "a", "an", "to", "is", "of", "and"];
/// Words that count as an assertion in a test.
const ASSERT_WORDS: [&str; 6] = [
    "assert",
    "expect(",
    "panic!",
    "should",
    "unwrap_err",
    "tobe(",
];

struct Added {
    line: usize,
    text: String,
}

struct Hunk {
    new_start: usize,
    new_count: usize,
    added: Vec<Added>,
}

struct FileDiff {
    path: String,
    hunks: Vec<Hunk>,
}

pub fn quality_notes(task: &TaskId, diff: &str) -> Vec<QualityNote> {
    let files = parse(diff);
    let mut notes = Vec::new();
    for file in &files {
        for hunk in &file.hunks {
            long_file(task, file, hunk, &mut notes);
            long_added_block(task, file, hunk, &mut notes);
            placeholders(task, file, hunk, &mut notes);
            filler_comments(task, file, hunk, &mut notes);
            tests_without_assertion(task, file, hunk, &mut notes);
        }
    }
    duplicate_blocks(task, &files, &mut notes);
    notes
}

/// The kind and the stable code of one rule.
type Rule = (QualityKind, &'static str);

fn note(task: &TaskId, (kind, code): Rule, path: &str, line: usize) -> QualityNote {
    QualityNote {
        task: task.clone(),
        kind,
        location: format!("{path}:{line}"),
        detail_code: code.to_owned(),
    }
}

/// Split a unified diff into files and hunks, keeping each added line with its new line number.
fn parse(diff: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut line_number = 0;
    for raw in diff.lines() {
        if let Some(path) = raw.strip_prefix("+++ ") {
            let path = path.strip_prefix("b/").unwrap_or(path);
            files.push(FileDiff {
                path: path.to_owned(),
                hunks: Vec::new(),
            });
        } else if raw.starts_with("@@") {
            let Some(file) = files.last_mut() else {
                continue;
            };
            let (start, count) = hunk_range(raw);
            line_number = start;
            file.hunks.push(Hunk {
                new_start: start,
                new_count: count,
                added: Vec::new(),
            });
        } else if let Some(hunk) = files.last_mut().and_then(|f| f.hunks.last_mut()) {
            line_number = read_body_line(raw, line_number, hunk);
        }
    }
    files
}

/// `@@ -a,b +c,d @@` gives `(c, d)`. A missing count means 1.
fn hunk_range(header: &str) -> (usize, usize) {
    let new = header
        .split_whitespace()
        .find(|part| part.starts_with('+'))
        .unwrap_or("+0");
    let mut parts = new.trim_start_matches('+').split(',');
    let start = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let count = parts.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    (start, count)
}

/// Record one body line of a hunk and return the next new-file line number.
fn read_body_line(raw: &str, line_number: usize, hunk: &mut Hunk) -> usize {
    match raw.chars().next() {
        Some('+') => {
            hunk.added.push(Added {
                line: line_number,
                text: raw[1..].to_owned(),
            });
            line_number + 1
        }
        Some('-' | '\\') => line_number,
        _ => line_number + 1,
    }
}

fn long_file(task: &TaskId, file: &FileDiff, hunk: &Hunk, notes: &mut Vec<QualityNote>) {
    let end = (hunk.new_start + hunk.new_count).saturating_sub(1);
    if end > LONG_FILE_LINES {
        notes.push(note(
            task,
            (QualityKind::Size, "long_file"),
            &file.path,
            end,
        ));
    }
}

fn long_added_block(task: &TaskId, file: &FileDiff, hunk: &Hunk, notes: &mut Vec<QualityNote>) {
    let mut run_start = None;
    let mut run_len = 0;
    let mut previous = 0;
    for added in &hunk.added {
        let continues = run_start.is_some() && added.line == previous + 1;
        if !continues {
            run_start = Some(added.line);
            run_len = 0;
        }
        previous = added.line;
        if !added.text.trim().is_empty() {
            run_len += 1;
        }
        if run_len == LONG_BLOCK_LINES + 1 {
            let line = run_start.unwrap_or(added.line);
            notes.push(note(
                task,
                (QualityKind::Size, "long_added_block"),
                &file.path,
                line,
            ));
        }
    }
}

fn placeholders(task: &TaskId, file: &FileDiff, hunk: &Hunk, notes: &mut Vec<QualityNote>) {
    for added in &hunk.added {
        if is_placeholder(&added.text) {
            notes.push(note(
                task,
                (QualityKind::Slop, "placeholder"),
                &file.path,
                added.line,
            ));
        }
    }
}

fn is_placeholder(text: &str) -> bool {
    text.contains("todo!(")
        || text.contains("unimplemented!(")
        || text.contains("FIXME")
        || bare_todo(text)
}

/// An open-work marker that has no task id in brackets after it.
fn bare_todo(text: &str) -> bool {
    let Some(index) = text.find("TODO") else {
        return false;
    };
    let after = &text[index + 4..];
    let id = after
        .strip_prefix('(')
        .and_then(|rest| rest.split_once(')'))
        .map(|(id, _)| id);
    !id.is_some_and(|id| id.contains('-') && id.chars().any(|c| c.is_ascii_digit()))
}

/// A comment that only repeats the next line: every meaningful word of the comment appears in the
/// code line.
fn filler_comments(task: &TaskId, file: &FileDiff, hunk: &Hunk, notes: &mut Vec<QualityNote>) {
    for pair in hunk.added.windows(2) {
        let (comment, code) = (&pair[0], &pair[1]);
        if code.line != comment.line + 1 {
            continue;
        }
        let Some(text) = comment_text(&comment.text) else {
            continue;
        };
        let said = meaningful_words(text);
        if !said.is_empty() && said.is_subset(&meaningful_words(&code.text)) {
            notes.push(note(
                task,
                (QualityKind::Slop, "filler_comment"),
                &file.path,
                comment.line,
            ));
        }
    }
}

fn comment_text(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    ["///", "//!", "//", "#"]
        .iter()
        .find_map(|marker| trimmed.strip_prefix(marker))
}

/// Lowercase words, split on anything that is not a letter or digit, without filler words, with
/// a trailing `s` removed so `adds` matches `add`.
fn meaningful_words(text: &str) -> BTreeSet<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty() && !FILLER_WORDS.contains(word))
        .map(|word| {
            word.strip_suffix('s')
                .filter(|w| w.len() > 2)
                .unwrap_or(word)
                .to_owned()
        })
        .collect()
}

/// A test marker line starts a test. Its body runs to the next marker or the end of the hunk. A
/// test whose lines hold no assertion word gets a note.
fn tests_without_assertion(
    task: &TaskId,
    file: &FileDiff,
    hunk: &Hunk,
    notes: &mut Vec<QualityNote>,
) {
    let starts: Vec<usize> = (0..hunk.added.len())
        .filter(|&i| is_test_marker(&hunk.added[i].text))
        .collect();
    for (n, &start) in starts.iter().enumerate() {
        let end = starts.get(n + 1).copied().unwrap_or(hunk.added.len());
        let has_assertion = hunk.added[start..end].iter().any(|added| {
            let lower = added.text.to_lowercase();
            ASSERT_WORDS.iter().any(|word| lower.contains(word))
        });
        if !has_assertion {
            let line = hunk.added[start].line;
            notes.push(note(
                task,
                (QualityKind::Slop, "test_without_assertion"),
                &file.path,
                line,
            ));
        }
    }
}

fn is_test_marker(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("#[test]")
        || trimmed.starts_with("#[tokio::test")
        || trimmed.starts_with("def test_")
        || trimmed.starts_with("test(")
        || trimmed.starts_with("it(")
}

/// Windows of six non-blank added lines that appear twice or more in the diff. One note per copied
/// region, not one per sliding window.
fn duplicate_blocks(task: &TaskId, files: &[FileDiff], notes: &mut Vec<QualityNote>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut last_reported: HashMap<&str, usize> = HashMap::new();
    let windows = files.iter().flat_map(|file| {
        file.hunks
            .iter()
            .flat_map(move |hunk| block_windows(hunk).into_iter().map(move |w| (file, w)))
    });
    for (file, (line, joined)) in windows {
        let count = seen.entry(joined).or_insert(0);
        *count += 1;
        let recent = last_reported
            .get(file.path.as_str())
            .is_some_and(|last| line <= last + DUPLICATE_LINES);
        if *count == 2 && !recent {
            let rule = (QualityKind::Duplication, "duplicate_block");
            notes.push(note(task, rule, &file.path, line));
            last_reported.insert(file.path.as_str(), line);
        }
    }
}

/// Every window of six non-blank added lines in a hunk that is long enough to matter, with the
/// new line number of its first line.
fn block_windows(hunk: &Hunk) -> Vec<(usize, String)> {
    let lines: Vec<&Added> = hunk
        .added
        .iter()
        .filter(|a| !a.text.trim().is_empty())
        .collect();
    lines
        .windows(DUPLICATE_LINES)
        .map(|window| {
            let joined: Vec<&str> = window.iter().map(|a| a.text.trim()).collect();
            (window[0].line, joined.join("\n"))
        })
        .filter(|(_, joined)| joined.len() >= DUPLICATE_CHARS)
        .collect()
}
