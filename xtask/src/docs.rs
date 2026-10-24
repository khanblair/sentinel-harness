//! Doc check: docs/project-structure.md must match the files in the repo.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::Context;

use crate::finding::{CheckResult, Finding};

const CHECK: &str = "doc-check";
const STRUCTURE_DOC: &str = "docs/project-structure.md";
const PROGRESS_DOC: &str = "docs/progress-tracker.md";
const README_DOC: &str = "docs/README.md";

const ALLOWED_FILES: [&str; 5] = [
    "Cargo.lock",
    ".gitignore",
    ".cargo/config.toml",
    "crates/.gitkeep",
    "LICENSE",
];

/// Root prefixes whose contents the structure doc never lists, by intent.
const ALLOWED_PREFIXES: [&str; 5] = [".github/", "site/", "target/", "dist/", "fixtures/"];

pub fn run(root: &Path) -> CheckResult {
    let parsed = parse_structure(&read_required(root, STRUCTURE_DOC)?);
    let planned = planned_prefixes(&read_optional(root, PROGRESS_DOC));
    let mut findings = duplicate_findings(&parsed);
    findings.extend(missing_findings(&parsed, &planned, root));
    let rules = ExtraRules {
        listed: parsed
            .listed
            .iter()
            .map(|entry| entry.path.clone())
            .collect(),
        docs_map: doc_map_docs(&read_optional(root, README_DOC)),
        crate_folders: parsed.crate_folders.clone(),
    };
    for file in crate::files::repo_files(root)? {
        if !rules.allows(&file) {
            findings.push(Finding::new(
                CHECK,
                file,
                0,
                "on disk but not listed in project-structure.md",
            ));
        }
    }
    Ok(findings)
}

fn read_required(root: &Path, rel: &str) -> anyhow::Result<String> {
    fs::read_to_string(root.join(rel)).with_context(|| format!("read {rel}"))
}

fn read_optional(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap_or_default()
}

struct Listed {
    path: String,
    line: usize,
}

struct Structure {
    listed: Vec<Listed>,
    crate_folders: HashSet<String>,
}

/// Walks the fenced blocks of project-structure.md and collects file paths.
#[derive(Default)]
struct Parser {
    in_code: bool,
    context: String,
    listed: Vec<Listed>,
    crate_folders: HashSet<String>,
}

impl Parser {
    fn accept(&mut self, raw: &str, line: usize) {
        let text = raw.trim_end();
        if text.trim() == "```" {
            self.in_code = !self.in_code;
            return;
        }
        if !self.in_code {
            return;
        }
        let Some(token) = text.split_whitespace().next() else {
            return;
        };
        if text.starts_with(' ') || text.starts_with('\t') {
            self.accept_entry(token, line);
        } else if token.ends_with('/') {
            self.start_context(token);
        } else {
            // An unindented file line, such as the site block workflow entry,
            // is relative to the repo root, never to the current context.
            self.listed.push(Listed {
                path: token.to_string(),
                line,
            });
        }
    }

    fn accept_entry(&mut self, token: &str, line: usize) {
        if token.ends_with('/') || token.ends_with('*') {
            return;
        }
        self.add_file(token, line);
    }

    fn start_context(&mut self, token: &str) {
        self.context = if token == "sentinel/" {
            String::new()
        } else {
            token.trim_end_matches('/').to_string()
        };
        self.note_crate_folder();
    }

    /// The extra-file check runs only for crates the structure doc describes.
    fn note_crate_folder(&mut self) {
        let Some((first, second)) = self.context.split_once('/') else {
            return;
        };
        if first == "crates" && !second.is_empty() {
            self.crate_folders.insert(format!("crates/{second}/"));
        }
    }

    fn add_file(&mut self, token: &str, line: usize) {
        let path = if self.context.is_empty() {
            token.to_string()
        } else {
            format!("{}/{token}", self.context)
        };
        self.listed.push(Listed { path, line });
    }
}

fn parse_structure(text: &str) -> Structure {
    let mut parser = Parser::default();
    for (index, line) in text.lines().enumerate() {
        parser.accept(line, index + 1);
    }
    Structure {
        listed: parser.listed,
        crate_folders: parser.crate_folders,
    }
}

fn duplicate_findings(structure: &Structure) -> Vec<Finding> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut findings = Vec::new();
    for listed in &structure.listed {
        if !seen.insert(listed.path.clone()) {
            findings.push(Finding::new(
                CHECK,
                &listed.path,
                listed.line,
                "listed again in project-structure.md",
            ));
        }
    }
    findings
}

fn missing_findings(structure: &Structure, planned: &[String], root: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    for listed in &structure.listed {
        if root.join(&listed.path).is_file() {
            continue;
        }
        if planned.iter().any(|prefix| listed.path.starts_with(prefix)) {
            continue;
        }
        findings.push(Finding::new(
            CHECK,
            &listed.path,
            listed.line,
            "listed in project-structure.md but missing",
        ));
    }
    findings
}

/// Paths in the `## Planned Files` section that may still be missing.
fn planned_prefixes(text: &str) -> Vec<String> {
    let mut prefixes = Vec::new();
    let mut in_section = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") {
            if in_section {
                break;
            }
            in_section = trimmed == "## Planned Files";
        } else if in_section {
            prefixes.extend(bullet_token(trimmed));
        }
    }
    prefixes
}

fn bullet_token(line: &str) -> Option<String> {
    let rest = line.strip_prefix("- `")?;
    let end = rest.find('`')?;
    Some(rest[..end].to_string())
}

/// Doc paths from the doc map table in docs/README.md.
fn doc_map_docs(text: &str) -> HashSet<String> {
    let mut docs = HashSet::new();
    for line in text.lines() {
        if line.trim_start().starts_with("| [") {
            docs.extend(link_target(line).map(|target| format!("docs/{target}")));
        }
    }
    docs
}

fn link_target(line: &str) -> Option<String> {
    let rest = line.split("](").nth(1)?;
    let end = rest.find(')')?;
    Some(rest[..end].to_string())
}

/// The allow rules for a file that exists but is not listed.
struct ExtraRules {
    listed: HashSet<String>,
    docs_map: HashSet<String>,
    crate_folders: HashSet<String>,
}

impl ExtraRules {
    fn allows(&self, file: &Path) -> bool {
        let rel = file.to_string_lossy();
        if self.listed.contains(rel.as_ref()) || ALLOWED_FILES.contains(&rel.as_ref()) {
            return true;
        }
        if ALLOWED_PREFIXES
            .iter()
            .any(|prefix| rel.starts_with(*prefix))
        {
            return true;
        }
        if self.docs_map.contains(rel.as_ref()) {
            return true;
        }
        if rel.starts_with("crates/") {
            return self.crate_folder_listed(&rel);
        }
        false
    }

    /// A crate folder the doc never describes is not checked at all.
    fn crate_folder_listed(&self, rel: &str) -> bool {
        let crate_name = rel.split('/').nth(1).unwrap_or("");
        self.crate_folders
            .contains(&format!("crates/{crate_name}/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use tempfile::{TempDir, tempdir};

    fn touched(root: &Path, rel: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "x").unwrap();
    }

    /// A temp repo root with a structure doc and the given files on disk.
    fn fresh(text: &str, files: &[&str]) -> TempDir {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("docs")).unwrap();
        fs::write(dir.path().join(STRUCTURE_DOC), text).unwrap();
        for rel in files {
            touched(dir.path(), rel);
        }
        dir
    }

    fn top_block(entries: &str) -> String {
        format!(
            "\n## Top Level\n\n```\nsentinel/\n{entries}  docs/project-structure.md   this file\n```\n"
        )
    }

    const TOP_ENTRIES: &str = "  Cargo.toml   workspace\n  README.md   intro\n";
    const SITE_ENTRIES: &str =
        "  Cargo.toml   workspace\n  README.md   intro\n  docs/README.md   map\n";
    const README_MAP: &str =
        "# Docs\n\n## Doc Map\n\n| Doc | Q |\n|---|---|\n| [extra.md](extra.md) | ? |\n";

    fn planned_dir(plan: Option<&str>) -> TempDir {
        let text = "\n## site\n\n```\nsite/\n  book.toml   settings\n  src/help.md   help\ndocs/project-structure.md   this file\ndocs/progress-tracker.md   tracker\n```\n";
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("docs")).unwrap();
        fs::write(dir.path().join(STRUCTURE_DOC), text).unwrap();
        touched(dir.path(), "site/book.toml");
        touched(dir.path(), PROGRESS_DOC);
        if let Some(plan) = plan {
            fs::write(dir.path().join(PROGRESS_DOC), plan).unwrap();
        }
        dir
    }

    #[test]
    fn clean_structure_doc_with_matching_files_passes() {
        let dir = fresh(&top_block(TOP_ENTRIES), &["Cargo.toml", "README.md"]);
        assert_eq!(run(dir.path()).unwrap(), Vec::new());
    }

    #[test]
    fn listed_missing_file_fails() {
        let entries = TOP_ENTRIES.replace("README.md", "NOTICE.md");
        let dir = fresh(&top_block(&entries), &["Cargo.toml"]);
        let findings = run(dir.path()).unwrap();
        assert_eq!(findings[0].path, Path::new("NOTICE.md"), "{findings:?}");
    }

    #[test]
    fn planned_file_prefix_allows_missing_file() {
        let plan = "# Progress\n\n## Planned Files\n\n- `site/`\n";
        let with_plan = planned_dir(Some(plan));
        assert_eq!(run(with_plan.path()).unwrap(), Vec::new());
        let without_plan = planned_dir(None);
        assert_eq!(run(without_plan.path()).unwrap().len(), 1);
    }

    #[test]
    fn unlisted_extra_file_fails() {
        let dir = fresh(
            &top_block(TOP_ENTRIES),
            &["Cargo.toml", "README.md", "extra.rs"],
        );
        let findings = run(dir.path()).unwrap();
        assert_eq!(findings[0].path, Path::new("extra.rs"), "{findings:?}");
    }

    #[test]
    fn allowed_root_extra_file_passes() {
        let dir = fresh(
            &top_block(TOP_ENTRIES),
            &["Cargo.toml", "README.md", "Cargo.lock"],
        );
        assert_eq!(run(dir.path()).unwrap(), Vec::new());
    }

    #[test]
    fn doc_map_doc_with_no_map_row_fails() {
        let dir = fresh(
            &top_block(TOP_ENTRIES),
            &["Cargo.toml", "README.md", "docs/extra.md"],
        );
        assert_eq!(
            run(dir.path()).unwrap().len(),
            1,
            "a docs file with no map row fails"
        );
    }

    #[test]
    fn doc_map_doc_passes_when_unlisted() {
        let dir = fresh(
            &top_block(SITE_ENTRIES),
            &["Cargo.toml", "README.md", "docs/extra.md"],
        );
        touched(dir.path(), README_DOC);
        fs::write(dir.path().join(README_DOC), README_MAP).unwrap();
        assert_eq!(run(dir.path()).unwrap(), Vec::new());
    }

    #[test]
    fn duplicate_listing_fails() {
        let entries = "  Cargo.toml   workspace\n  README.md   intro\n  README.md   intro again\n";
        let dir = fresh(&top_block(entries), &["Cargo.toml", "README.md"]);
        let findings = run(dir.path()).unwrap();
        let duplicate = findings.last().unwrap();
        assert_eq!(duplicate.path, Path::new("README.md"), "{findings:?}");
        assert_eq!(duplicate.message, "listed again in project-structure.md");
        assert_eq!(duplicate.line, 8);
    }
}
