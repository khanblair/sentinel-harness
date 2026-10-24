//! Smell check: size, nesting, names, and placeholder patterns in Rust source.

use std::path::{Component, Path};

use crate::finding::{CheckResult, Finding};
use crate::rust_scan::{RustFile, leading_ident};

const CHECK: &str = "smell-check";
const LONG_FUNCTION_LINES: usize = 40;
const BIG_FILE_LINES: usize = 400;
const DEEP_NESTING_LEVELS: usize = 3;
const BANNED_NAMES: [&str; 11] = [
    "data", "info", "handler", "manager", "util", "utils", "helper", "helpers", "misc", "common",
    "result2",
];
const PLACEHOLDERS: [&str; 2] = ["todo!(", "unimplemented!("];
const ASSERTIONS: [&str; 8] = [
    "assert",
    "assert_eq",
    "assert_ne",
    "assert!",
    "panic!",
    "unwrap_err",
    "expect_err",
    "matches!",
];
const SKIP_DIRS: [&str; 2] = ["fixtures", "target"];

type RuleFn = fn(&RustFile) -> Vec<Finding>;

const RULES: [RuleFn; 7] = [
    long_function,
    big_file,
    deep_nesting,
    bad_names,
    placeholder_code,
    bare_todo,
    test_without_assert,
];

/// Scans every `.rs` file in the repo, skipping fixtures and build output.
pub fn run(root: &Path) -> CheckResult {
    let mut findings = Vec::new();
    for rel in crate::files::repo_files(root)? {
        if rel.extension().and_then(|ext| ext.to_str()) != Some("rs") || has_skip_dir(&rel) {
            continue;
        }
        let Some(text) = crate::files::read_text(&root.join(&rel)) else {
            continue;
        };
        let file = RustFile::new(rel, &text);
        for rule in RULES {
            findings.extend(rule(&file));
        }
    }
    Ok(findings)
}

fn finding(file: &RustFile, line: usize, message: String) -> Finding {
    Finding::new(CHECK, file.path.clone(), line, message)
}

fn has_skip_dir(rel: &Path) -> bool {
    rel.components().any(|part| match part {
        Component::Normal(name) => SKIP_DIRS.iter().any(|skip| name == *skip),
        Component::Prefix(_) | Component::RootDir | Component::CurDir | Component::ParentDir => {
            false
        }
    })
}

fn long_function(file: &RustFile) -> Vec<Finding> {
    file.functions()
        .iter()
        .filter(|span| !span.is_test)
        .filter_map(|span| {
            let lines = span.end_line - span.start_line + 1;
            (lines > LONG_FUNCTION_LINES).then(|| {
                finding(
                    file,
                    span.start_line,
                    format!(
                        "long function: `{}` is {lines} lines, limit {LONG_FUNCTION_LINES}",
                        span.name
                    ),
                )
            })
        })
        .collect()
}

/// Test code does not count toward the file limit: a unit test module sits at the bottom of the
/// file on purpose (`docs/code-standards.md`), and a file under `tests/` is all tests.
fn big_file(file: &RustFile) -> Vec<Finding> {
    let tests = file.test_lines();
    let lines = tests.iter().filter(|is_test| !**is_test).count();
    if lines <= BIG_FILE_LINES {
        return Vec::new();
    }
    let message = format!("big file: {lines} lines, limit {BIG_FILE_LINES}");
    vec![finding(file, 1, message)]
}

fn deep_nesting(file: &RustFile) -> Vec<Finding> {
    file.functions()
        .iter()
        .filter(|span| !span.is_test)
        .filter_map(|span| {
            let depth = file.max_nesting(span);
            (depth > DEEP_NESTING_LEVELS).then(|| {
                finding(
                    file,
                    span.start_line,
                    format!(
                        "deep nesting: `{}` reaches {depth} levels, limit {DEEP_NESTING_LEVELS}",
                        span.name
                    ),
                )
            })
        })
        .collect()
}

fn bad_names(file: &RustFile) -> Vec<Finding> {
    let mut hits = Vec::new();
    let stem = file.path.file_stem().and_then(|stem| stem.to_str());
    if let Some(stem) = stem
        && let Some(rank) = banned_rank(stem)
    {
        hits.push(finding(
            file,
            1,
            format!(
                "bad name: file stem `{stem}` matches banned name {rank} of {}",
                BANNED_NAMES.len()
            ),
        ));
    }
    for (idx, line) in file.code_lines().iter().enumerate() {
        for (kind, name) in named_items(line) {
            if let Some(rank) = banned_rank(name) {
                hits.push(finding(
                    file,
                    idx + 1,
                    format!(
                        "bad name: {kind} `{name}` matches banned name {rank} of {}",
                        BANNED_NAMES.len()
                    ),
                ));
            }
        }
    }
    hits
}

fn placeholder_code(file: &RustFile) -> Vec<Finding> {
    let code = file.code_lines();
    let test_lines = file.test_lines();
    let mut hits = Vec::new();
    for (idx, line) in code.iter().enumerate() {
        if test_lines[idx] {
            continue;
        }
        let count: usize = PLACEHOLDERS
            .iter()
            .map(|mark| line.matches(mark).count())
            .sum();
        if count > 0 {
            let unit = if count == 1 { "call" } else { "calls" };
            hits.push(finding(
                file,
                idx + 1,
                format!("placeholder code: {count} placeholder {unit} outside test code"),
            ));
        }
    }
    hits
}

fn bare_todo(file: &RustFile) -> Vec<Finding> {
    let mut hits = Vec::new();
    for (idx, comment) in file.line_comments().iter().enumerate() {
        let Some(comment) = comment else {
            continue;
        };
        let count = bare_markers(comment);
        if count > 0 {
            let unit = if count == 1 { "marker" } else { "markers" };
            hits.push(finding(
                file,
                idx + 1,
                format!("bare TODO: {count} unidentified {unit}"),
            ));
        }
    }
    hits
}

fn test_without_assert(file: &RustFile) -> Vec<Finding> {
    let code = file.code_lines();
    let mut hits = Vec::new();
    for span in file.functions() {
        if !marked_test(&code, span.start_line) {
            continue;
        }
        let body = &code[span.start_line - 1..span.end_line];
        if ASSERTIONS
            .iter()
            .any(|mark| body.iter().any(|line| line.contains(mark)))
        {
            continue;
        }
        let lines = span.end_line - span.start_line + 1;
        hits.push(finding(
            file,
            span.start_line,
            format!(
                "test with no assertion: `{}` has 0 assertions in {lines} lines",
                span.name
            ),
        ));
    }
    hits
}

fn marked_test(code: &[String], start_line: usize) -> bool {
    start_line >= 2 && code[start_line - 2].contains("#[test]")
}

fn named_items(line: &str) -> Vec<(&str, &str)> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let mut found = Vec::new();
    for (idx, keyword) in words.iter().enumerate() {
        if !matches!(*keyword, "fn" | "struct" | "enum" | "let") {
            continue;
        }
        let mut name_at = idx + 1;
        if words.get(name_at) == Some(&"mut") {
            name_at += 1;
        }
        if let Some(next) = words.get(name_at) {
            let name = leading_ident(next);
            if !name.is_empty() {
                found.push((*keyword, name));
            }
        }
    }
    found
}

fn banned_rank(name: &str) -> Option<usize> {
    let at = BANNED_NAMES.iter().position(|banned| *banned == name)?;
    Some(at + 1)
}

fn bare_markers(comment: &str) -> usize {
    ["TODO", "FIXME"]
        .iter()
        .map(|marker| {
            comment
                .match_indices(marker)
                .filter(|(at, _)| !has_task_id(&comment[at + marker.len()..]))
                .count()
        })
        .sum()
}

/// True when the text right after a marker is `(P<digits>-<digits>)`.
fn has_task_id(after: &str) -> bool {
    let Some(tail) = after.strip_prefix("(P") else {
        return false;
    };
    let digits = tail
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(tail.len());
    let Some(rest) = tail[digits..].strip_prefix('-') else {
        return false;
    };
    let more = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    digits > 0 && more > 0 && rest[more..].starts_with(')')
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;
    use std::fs;

    use super::*;

    fn rule_hits(name: &str, text: &str, rule: &str) -> Vec<Finding> {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(name), text).unwrap();
        run(dir.path())
            .unwrap()
            .into_iter()
            .filter(|hit| hit.message.starts_with(rule))
            .collect()
    }

    fn assert_no_hits(name: &str, text: &str, rule: &str) {
        assert_eq!(rule_hits(name, text, rule).len(), 0);
    }

    fn fn_with_body(body_lines: usize) -> String {
        let mut text = String::from("fn worker() {\n");
        for step in 0..body_lines {
            writeln!(text, "    let step_{step} = {step};").unwrap();
        }
        text.push_str("}\n");
        text
    }

    fn fn_with_ifs(ifs: usize) -> String {
        let mut text = String::from("fn guard() {\n");
        for depth in 0..ifs {
            writeln!(text, "{}if flag_{depth} {{", "    ".repeat(depth + 1)).unwrap();
        }
        for depth in (0..ifs).rev() {
            writeln!(text, "{}}}", "    ".repeat(depth + 1)).unwrap();
        }
        text.push_str("}\n");
        text
    }

    #[test]
    fn size_rules_flag_past_the_limit_and_allow_the_limit() {
        let long = rule_hits("over.rs", &fn_with_body(39), "long function");
        assert_eq!((long.len(), long[0].line), (1, 1));
        assert!(long[0].message.contains("41 lines"));
        assert_no_hits("under.rs", &fn_with_body(38), "long function");
        let big = rule_hits("pad.rs", &"// filler\n".repeat(401), "big file");
        assert_eq!(big.len(), 1);
        assert_eq!(big[0].line, 1);
        assert!(big[0].message.contains("401 lines"));
        assert_no_hits("pad.rs", &"// filler\n".repeat(400), "big file");
        let deep = rule_hits("nest.rs", &fn_with_ifs(4), "deep nesting");
        assert!(deep.len() == 1 && deep[0].message.contains("4 levels"));
        assert_no_hits("nest.rs", &fn_with_ifs(3), "deep nesting");
    }

    #[test]
    fn bad_names_flag_banned_items_and_allow_clean_names() {
        let text = "struct data;\nenum info {}\nfn handler() {\n    let mut manager = 1;\n    let helper = manager;\n}\n";
        let hits = rule_hits("decls.rs", text, "bad name");
        assert_eq!(hits.len(), 5);
        assert!(
            hits.iter()
                .any(|hit| hit.line == 4 && hit.message.contains("let `manager`"))
        );
        let clean = "struct packet;\nenum mode {}\nfn build() {\n    let item = 1;\n    let mut total = item;\n}\n";
        assert_no_hits("decls.rs", clean, "bad name");
        let stem = rule_hits("utils.rs", "fn build() {}\n", "bad name");
        assert_eq!(stem.len(), 1);
        assert_eq!(stem[0].line, 1);
        assert!(stem[0].message.contains("file stem `utils`"));
    }

    #[test]
    fn placeholder_todo_and_assertion_rules_flags_bad_code_and_allow_clean_code() {
        let flagged = "fn build() -> u8 {\n    todo!()\n}\nfn rest() -> u8 {\n    unimplemented!(\n    )\n}\n";
        let hits = rule_hits("build.rs", flagged, "placeholder code");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].line, 2);
        assert_eq!(hits[1].line, 5);
        let quiet =
            "#[cfg(test)]\nmod tests {\n    fn probe() -> u8 {\n        todo!()\n    }\n}\n";
        assert_no_hits("probe.rs", quiet, "placeholder code");
        let bad = "// TODO: fix\nfn first() {}\n// FIXME later\nfn second() {}\n";
        let hits = rule_hits("note.rs", bad, "bare TODO");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].line, 1);
        assert_eq!(hits[1].line, 3);
        let good = "// TODO(P3-05): fix\nfn first() {}\n// FIXME(P3-06): later\nfn second() {}\n";
        assert_no_hits("note.rs", good, "bare TODO");
        let bad =
            "#[test]\nfn reads_config() {\n    let parsed = parse();\n    parsed.unwrap();\n}\n";
        let hits = rule_hits("reads.rs", bad, "test with no assertion");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 2);
        let good = "#[test]\nfn reads_config() {\n    assert!(parse());\n}\n";
        assert_no_hits("reads.rs", good, "test with no assertion");
    }

    #[test]
    fn run_keeps_src_and_skips_fixtures_target_and_non_rust() {
        let dir = tempfile::tempdir().unwrap();
        for rel in [
            "src/keep.rs",
            "fixtures/skip.rs",
            "target/skip.rs",
            "notes.md",
        ] {
            let path = dir.path().join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "// TODO: fix\n").unwrap();
        }
        let hits = run(dir.path()).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, std::path::PathBuf::from("src/keep.rs"));
    }
}
