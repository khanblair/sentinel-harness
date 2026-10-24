//! Ban check: no banned source texts and no banned crates.

use std::fs;
use std::path::Path;

use anyhow::Context;

use crate::finding::{CheckResult, Finding};

const CHECK: &str = "ban-check";
/// Texts banned in Rust sources. This const line is exempt from its own scan.
const BANNED_TEXTS: [&str; 2] = ["git worktree", "git2"];
/// Root folders whose Rust sources the ban texts apply to.
const SOURCE_ROOTS: [&str; 2] = ["crates", "xtask"];
/// Manifest sections that name dependencies.
const DEP_SECTIONS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

pub fn run(root: &Path) -> CheckResult {
    let banned = banned_crates(root)?;
    let mut findings = source_findings(root)?;
    findings.extend(manifest_findings(root, &banned)?);
    Ok(findings)
}

fn source_findings(root: &Path) -> anyhow::Result<Vec<Finding>> {
    let files = crate::files::repo_files(root)?;
    let mut findings = Vec::new();
    for file in files.into_iter().filter(|file| is_source(file)) {
        let Some(text) = crate::files::read_text(&root.join(&file)) else {
            continue;
        };
        findings.extend(banned_lines(&file, &text));
    }
    Ok(findings)
}

fn is_source(file: &Path) -> bool {
    let rel = file.to_string_lossy();
    let under_source = SOURCE_ROOTS
        .iter()
        .any(|leaf| rel.starts_with(format!("{leaf}/").as_str()));
    under_source && rel.ends_with(".rs")
}

fn banned_lines(file: &Path, text: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if !line_has_banned_text(file, line) {
            continue;
        }
        let matched = BANNED_TEXTS.iter().find(|text| line.contains(**text));
        findings.push(Finding::new(
            CHECK,
            file,
            index + 1,
            format!("banned text {matched:?} in Rust source"),
        ));
    }
    findings
}

fn line_has_banned_text(file: &Path, line: &str) -> bool {
    if file == Path::new("xtask/src/ban.rs") && line.contains("const") {
        return false;
    }
    BANNED_TEXTS.iter().any(|text| line.contains(*text))
}

fn manifest_findings(root: &Path, banned: &[String]) -> anyhow::Result<Vec<Finding>> {
    let files = crate::files::repo_files(root)?;
    let mut findings = Vec::new();
    for manifest in files.into_iter().filter(|file| is_manifest(file)) {
        let path = root.join(&manifest);
        let text =
            fs::read_to_string(&path).with_context(|| format!("read {}", manifest.display()))?;
        findings.extend(dep_findings(&manifest, &text, banned));
    }
    Ok(findings)
}

fn is_manifest(file: &Path) -> bool {
    let rel = file.to_string_lossy();
    if rel == "Cargo.toml" {
        return true;
    }
    let under_source = SOURCE_ROOTS
        .iter()
        .any(|leaf| rel.starts_with(format!("{leaf}/").as_str()));
    under_source && rel.ends_with("/Cargo.toml")
}

fn dep_findings(rel: &Path, text: &str, banned: &[String]) -> Vec<Finding> {
    let Ok(table) = toml::from_str::<toml::Table>(text) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    for name in dep_names(&table) {
        if !banned.contains(&name) {
            continue;
        }
        let message = format!("{name} is in the deny.toml ban list");
        findings.push(Finding::new(CHECK, rel, dep_line(text, &name), message));
    }
    findings
}

fn section_keys(table: &toml::Table, section: &str) -> Vec<String> {
    table
        .get(section)
        .and_then(toml::Value::as_table)
        .map_or(Vec::new(), |deps| deps.keys().cloned().collect())
}

fn dep_names(table: &toml::Table) -> Vec<String> {
    let mut names = Vec::new();
    for section in DEP_SECTIONS {
        names.extend(section_keys(table, section));
    }
    let workspace = table.get("workspace").and_then(toml::Value::as_table);
    let workspace_deps = workspace
        .and_then(|w| w.get("dependencies"))
        .and_then(toml::Value::as_table);
    if let Some(deps) = workspace_deps {
        names.extend(deps.keys().cloned());
    }
    names
}

fn dep_line(text: &str, name: &str) -> usize {
    let needle = format!("{name} ");
    text.lines()
        .position(|line| line.trim_start().starts_with(&needle))
        .map_or(0, |index| index + 1)
}

/// Crate names from the `[bans]` deny list. An absent deny.toml is no ban at all.
fn banned_crates(root: &Path) -> anyhow::Result<Vec<String>> {
    let Ok(text) = fs::read_to_string(root.join("deny.toml")) else {
        return Ok(Vec::new());
    };
    let table: toml::Table = toml::from_str(&text).with_context(|| "read deny.toml".to_string())?;
    let deny = table
        .get("bans")
        .and_then(toml::Value::as_table)
        .and_then(|bans| bans.get("deny"))
        .and_then(toml::Value::as_array);
    let Some(entries) = deny else {
        return Ok(Vec::new());
    };
    Ok(entries
        .iter()
        .filter_map(|entry| {
            entry
                .get("crate")
                .and_then(toml::Value::as_str)
                .map(str::to_string)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    use tempfile::tempdir;

    /// The banned source text, built here so no test line matches its own scan.
    fn worktree_text() -> String {
        ["git", "worktree"].join(" ")
    }

    /// A banned crate name, built here so no test line matches its own scan.
    fn denied_crate_name() -> String {
        format!("git{}", 2)
    }

    fn touched(root: &Path, rel: &str, body: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    #[test]
    fn source_code_and_comment_with_banned_text_fail() {
        let dir = tempdir().unwrap();
        let banned = worktree_text();
        touched(
            dir.path(),
            "crates/demo/src/lib.rs",
            &format!(
                "fn f() {{ let cmd = \"{banned}\"; }}\nlet unrelated = 1;\n// see the {banned} manual\n"
            ),
        );
        let findings = run(dir.path()).unwrap();
        assert_eq!(findings.len(), 2, "code and comment lines both count");
        assert_eq!(findings[0].line, 1);
        assert_eq!(findings[1].line, 3);
        assert_eq!(findings[0].path, Path::new("crates/demo/src/lib.rs"));
    }

    #[test]
    fn pattern_const_line_is_exempt_but_other_lines_are_not() {
        let dir = tempdir().unwrap();
        let banned = worktree_text();
        touched(
            dir.path(),
            "xtask/src/ban.rs",
            &format!("const BANNED: &str = \"{banned}\";\nlet banned = \"{banned}\";\n"),
        );
        let findings = run(dir.path()).unwrap();
        assert_eq!(findings.len(), 1, "only the non-const line is found");
        assert_eq!(findings[0].line, 2);
    }

    #[test]
    fn manifest_depending_on_denied_crate_fails() {
        let dir = tempdir().unwrap();
        let crate_name = denied_crate_name();
        touched(
            dir.path(),
            "deny.toml",
            &format!("[bans]\ndeny = [{{ crate = \"{crate_name}\" }}]\n"),
        );
        touched(
            dir.path(),
            "crates/demo/Cargo.toml",
            &format!("[dependencies]\n{crate_name} = \"0.18\"\n"),
        );
        touched(
            dir.path(),
            "crates/demo/src/lib.rs",
            "pub fn a() -> u32 { 1 }\n",
        );
        let findings = run(dir.path()).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, Path::new("crates/demo/Cargo.toml"));
        assert_eq!(findings[0].line, 2);
        assert!(findings[0].message.contains(&crate_name));
    }

    #[test]
    fn clean_crate_passes() {
        let dir = tempdir().unwrap();
        touched(
            dir.path(),
            "crates/demo/Cargo.toml",
            "[dependencies]\ntoml = \"1\"\n",
        );
        touched(
            dir.path(),
            "crates/demo/src/lib.rs",
            "pub fn a() -> u32 { 1 }\n",
        );
        let findings = run(dir.path()).unwrap();
        assert_eq!(findings, Vec::new());
    }
}
