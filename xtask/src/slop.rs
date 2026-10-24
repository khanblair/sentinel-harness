//! Slop check: banned words and patterns in docs, UI copy, and comments.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::files::{read_text, repo_files};
use crate::finding::{CheckResult, Finding};

const CHECK: &str = "slop-check";
const BANNED_PATH: &str = "xtask/banned-words.txt";
const OVERVIEW_PATH: &str = "docs/project-overview.md";
const WRITING_HEADING: &str = "## Writing Style Rules";
const UI_SRC: &str = "crates/sentinel-ui/src";
const EM_DASH: char = '\u{2014}';

/// One entry from the banned list. A word matches whole words; a phrase matches anywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    Word(String),
    Phrase(String),
}

impl Entry {
    fn parse(raw: &str) -> Option<Self> {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let lower = line.to_lowercase();
        if lower.contains(' ') || lower.contains('\'') {
            Some(Self::Phrase(lower))
        } else {
            Some(Self::Word(lower))
        }
    }

    fn hits(&self, lower: &str) -> usize {
        match self {
            Self::Word(word) => word_hits(lower, word),
            Self::Phrase(phrase) => lower.matches(phrase).count(),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Word(_) => "word",
            Self::Phrase(_) => "phrase",
        }
    }

    fn text(&self) -> &str {
        match self {
            Self::Word(value) | Self::Phrase(value) => value,
        }
    }
}

/// A run of ASCII letters, digits, and hyphens.
fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-'
}

fn word_hits(lower: &str, word: &str) -> usize {
    lower
        .match_indices(word)
        .filter(|(start, _)| {
            let start = *start;
            let before = start
                .checked_sub(1)
                .and_then(|i| lower.as_bytes().get(i))
                .copied();
            let after = lower.as_bytes().get(start + word.len()).copied();
            !before.is_some_and(is_word_byte) && !after.is_some_and(is_word_byte)
        })
        .count()
}

fn banned_entries(root: &Path) -> Result<Vec<Entry>> {
    let local = root.join(BANNED_PATH);
    // A temp fixture may hold only the files under test, so use the shipped list when it has none.
    let path = if local.is_file() {
        local
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("banned-words.txt")
    };
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    Ok(text.lines().filter_map(Entry::parse).collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Markdown,
    RustComments,
    RustWithStrings,
}

fn mode_for(rel: &Path) -> Option<Mode> {
    match rel.extension().and_then(|ext| ext.to_str()) {
        Some("md") => Some(Mode::Markdown),
        Some("rs") if rel.starts_with(Path::new(UI_SRC)) => Some(Mode::RustWithStrings),
        Some("rs") => Some(Mode::RustComments),
        _ => None,
    }
}

pub fn run(root: &Path) -> CheckResult {
    let entries = banned_entries(root)?;
    let mut findings = Vec::new();
    for rel in repo_files(root)? {
        if rel == Path::new(BANNED_PATH) {
            continue;
        }
        let Some(mode) = mode_for(&rel) else {
            continue;
        };
        let Some(text) = read_text(&root.join(&rel)) else {
            continue;
        };
        let mut scan = Scan {
            entries: &entries,
            path: &rel,
            skip_writing_section: rel == Path::new(OVERVIEW_PATH),
            findings: Vec::new(),
        };
        match mode {
            Mode::Markdown => scan.markdown(&text),
            Mode::RustComments => scan.rust(&text, false),
            Mode::RustWithStrings => scan.rust(&text, true),
        }
        findings.extend(scan.findings);
    }
    Ok(findings)
}

struct Scan<'a> {
    entries: &'a [Entry],
    path: &'a Path,
    skip_writing_section: bool,
    findings: Vec<Finding>,
}

impl Scan<'_> {
    fn push(&mut self, line: usize, message: String) {
        self.findings
            .push(Finding::new(CHECK, self.path, line, message));
    }

    fn text_line(&mut self, line: usize, text: &str) {
        let lower = text.to_lowercase();
        for entry in self.entries {
            let hits = entry.hits(&lower);
            let message = format!("banned {} `{}`", entry.label(), entry.text());
            for _ in 0..hits {
                self.push(line, message.clone());
            }
        }
        if has_not_just_but(&lower) {
            self.push(line, "pattern `not just ... but`".to_owned());
        }
        for _ in text.matches(EM_DASH) {
            self.push(line, "em dash is banned".to_owned());
        }
    }

    fn heading_line(&mut self, line: usize, text: &str) {
        for emoji in text.chars().filter(|c| is_emoji(*c)) {
            self.push(line, format!("emoji `{emoji}` in markdown heading"));
        }
    }

    fn markdown(&mut self, text: &str) {
        let mut fenced = false;
        let mut skip = false;
        for (index, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }
            if self.skip_writing_section && writing_section_skipped(&mut skip, trimmed) {
                continue;
            }
            let number = index + 1;
            self.text_line(number, line);
            if trimmed.starts_with('#') {
                self.heading_line(number, trimmed);
            }
        }
    }

    fn rust(&mut self, text: &str, strings: bool) {
        for (index, line) in text.lines().enumerate() {
            let number = index + 1;
            if let Some(start) = comment_start(line) {
                self.text_line(number, &line[start..]);
            }
            if strings {
                for literal in rust_strings(line) {
                    self.text_line(number, literal);
                }
            }
        }
    }
}

fn writing_section_skipped(skip: &mut bool, trimmed: &str) -> bool {
    if trimmed.starts_with("## ") {
        *skip = trimmed.trim_end() == WRITING_HEADING;
    }
    *skip
}

fn has_not_just_but(lower: &str) -> bool {
    let Some(start) = lower.find("not just") else {
        return false;
    };
    lower[start + "not just".len()..].contains("but")
}

fn is_emoji(c: char) -> bool {
    matches!(c, '\u{1F300}'..='\u{1FAFF}' | '\u{2600}'..='\u{27BF}')
}

/// Index of the quote that ends the string literal opened at `open`, or the length of `bytes`.
fn string_end(bytes: &[u8], open: usize) -> usize {
    let mut index = open + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return index,
            _ => index += 1,
        }
    }
    bytes.len()
}

/// Byte offset of `//` outside a string literal.
fn comment_start(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = string_end(bytes, index) + 1,
            b'/' if bytes.get(index + 1) == Some(&b'/') => return Some(index),
            _ => index += 1,
        }
    }
    None
}

/// Contents of the string literals on one line, ignoring any literal inside a comment.
fn rust_strings(line: &str) -> Vec<&str> {
    let code = comment_start(line).map_or(line, |start| &line[..start]);
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            let end = string_end(bytes, index).min(code.len());
            found.push(&code[index + 1..end]);
            index = end + 1;
        } else {
            index += 1;
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::*;

    const LIST: &str = "delve\nleverage\nunlock\nit's worth noting\n";

    fn write(dir: &TempDir, rel: &str, body: &str) {
        let path = dir.path().join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }

    fn scan_files(files: &[(&str, &str)]) -> Vec<Finding> {
        let dir = tempfile::tempdir().unwrap();
        write(&dir, BANNED_PATH, LIST);
        for (rel, body) in files {
            write(&dir, rel, body);
        }
        run(dir.path()).unwrap()
    }
    #[test]
    fn slop_banned_word_matches_whole_words_only() {
        let hits = scan_files(&[("docs/a.md", "Unlock the door.\n")]);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].message.contains("unlock"));
        assert_eq!(
            scan_files(&[("docs/a.md", "The door unlocked.\n")]),
            Vec::new()
        );
    }
    #[test]
    fn slop_banned_phrase_matches_substring_only() {
        let hits = scan_files(&[("docs/a.md", "It's worth noting that.\n")]);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].message.contains("worth noting"));
        assert_eq!(
            scan_files(&[("docs/a.md", "It is worth noting that.\n")]),
            Vec::new()
        );
    }
    #[test]
    fn slop_not_just_but_pattern_is_flagged() {
        let hits = scan_files(&[("docs/a.md", "This is not just fast, but cheap.\n")]);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].message.contains("not just"));
        assert_eq!(
            scan_files(&[("docs/a.md", "This is fast and cheap.\n")]),
            Vec::new()
        );
    }
    #[test]
    fn slop_em_dash_is_flagged() {
        let hits = scan_files(&[("docs/a.md", "A line \u{2014} here.\n")]);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].message.contains("em dash"));
        assert_eq!(scan_files(&[("docs/a.md", "A line - here.\n")]), Vec::new());
    }
    #[test]
    fn slop_emoji_in_heading_is_flagged() {
        let hits = scan_files(&[("docs/a.md", "# Launch \u{1F680}\n")]);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].message.contains("emoji"));
        assert_eq!(
            scan_files(&[("docs/a.md", "# Launch\n\nGo \u{1F680}.\n")]),
            Vec::new()
        );
    }
    #[test]
    fn slop_rust_scans_comments_not_code() {
        let hits = scan_files(&[("src/a.rs", "let unlock = 1; // leverage here\n")]);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].message.contains("leverage"));
        assert_eq!(scan_files(&[("src/a.rs", "let unlock = 1;\n")]), Vec::new());
    }
    #[test]
    fn slop_rust_comment_after_a_string_still_counts() {
        let hits = scan_files(&[("src/a.rs", "let url = \"https://x\"; // leverage\n")]);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].message.contains("leverage"));
    }
    #[test]
    fn slop_ui_strings_are_scanned() {
        let hits = scan_files(&[(
            "crates/sentinel-ui/src/view.rs",
            "let label = \"Unlock now\";\n",
        )]);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].message.contains("unlock"));
        assert_eq!(
            scan_files(&[("crates/core/src/lib.rs", "let label = \"Unlock now\";\n")]),
            Vec::new()
        );
    }
    #[test]
    fn slop_banned_words_file_is_never_scanned() {
        assert_eq!(scan_files(&[]), Vec::new());
    }
    #[test]
    fn slop_writing_style_section_is_exempt() {
        let body =
            "Intro.\n\n## Writing Style Rules\n\nUse leverage and unlock.\n\n## Next\n\nPlain.\n";
        assert_eq!(
            scan_files(&[("docs/project-overview.md", body)]),
            Vec::new()
        );
    }
    #[test]
    fn slop_text_outside_writing_style_section_is_scanned() {
        let body = "## Writing Style Rules\n\nClean.\n\n## Next\n\nUse leverage here.\n";
        let hits = scan_files(&[("docs/project-overview.md", body)]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 7);
    }
    #[test]
    fn slop_fenced_code_is_skipped() {
        assert_eq!(
            scan_files(&[("docs/a.md", "```\nleverage\n```\n")]),
            Vec::new()
        );
        let hits = scan_files(&[("docs/a.md", "```\ncode\n```\nleverage\n")]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 4);
    }
    #[test]
    fn slop_leverage_in_markdown_reports_one_finding() {
        let hits = scan_files(&[("docs/note.md", "We leverage the tool.\n")]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].check, "slop-check");
        assert_eq!(hits[0].path, PathBuf::from("docs/note.md"));
        assert!(hits[0].message.contains("leverage"));
    }
}
