//! Protected files: paths no agent may change. The practice repo pins each one in
//! `tests.lock.json` with its SHA-256. The config adds more, by path or by folder.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use sentinel_core::TaskStatus;
use sha2::{Digest, Sha256};

use crate::verdict::Verdict;

const LOCK_FILE: &str = "tests.lock.json";

/// Protected paths. A `None` value protects the path only. A `Some` value also pins its SHA-256.
/// A key that ends in `/` protects every path under that folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Protected {
    entries: BTreeMap<String, Option<String>>,
}

impl Protected {
    /// Read `root/tests.lock.json` and add the extra paths. A missing or broken lock file adds
    /// nothing and is not an error.
    pub fn load(root: &Path, extra: &[String]) -> Self {
        let mut entries = BTreeMap::new();
        if let Some(locked) = read_lock(root) {
            entries.insert(LOCK_FILE.to_owned(), None);
            entries.extend(locked.into_iter().map(|(path, hash)| (path, Some(hash))));
        }
        for path in extra.iter().filter(|path| !path.is_empty()) {
            entries.entry(path.clone()).or_insert(None);
        }
        Self { entries }
    }

    /// True when `path` is protected itself, or sits under a protected folder.
    pub fn covers(&self, path: &str) -> bool {
        self.entries.contains_key(path)
            || self
                .entries
                .keys()
                .any(|key| key.ends_with('/') && path.starts_with(key.as_str()))
    }

    /// Protected paths with a pinned hash whose file is missing or has another SHA-256.
    pub fn changed(&self, root: &Path) -> Vec<String> {
        self.entries
            .iter()
            .filter_map(|(path, hash)| {
                let expected = hash.as_deref()?;
                let intact = fs::read(root.join(path))
                    .is_ok_and(|bytes| sha256_hex(&bytes).eq_ignore_ascii_case(expected));
                (!intact).then_some(path.clone())
            })
            .collect()
    }

    /// Protected paths that a unified diff touches. Reads the `diff --git a/PATH b/PATH` headers,
    /// so the diff must come from `git show --no-renames`.
    pub fn touched_by_diff(&self, diff: &str) -> Vec<String> {
        let touched: BTreeSet<&str> = diff
            .lines()
            .filter_map(|line| line.strip_prefix("diff --git a/"))
            .filter_map(|rest| rest.split_once(" b/").map(|(path, _)| path))
            .filter(|path| self.covers(path))
            .collect();
        touched.into_iter().map(str::to_owned).collect()
    }
}

/// The verdict for a task whose checkpoint touched a protected path. `None` when none did.
pub fn tamper_verdict(touched: &[String]) -> Option<Verdict> {
    if touched.is_empty() {
        return None;
    }
    Some(Verdict {
        status: TaskStatus::Blocked,
        reason: format!("tampered with a protected file: {}", touched.join(", ")),
    })
}

fn read_lock(root: &Path) -> Option<BTreeMap<String, String>> {
    let text = fs::read_to_string(root.join(LOCK_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_lock(root: &Path, json: &str) {
        fs::write(root.join(LOCK_FILE), json).unwrap();
    }

    #[test]
    fn load_protects_each_locked_path_and_the_lock_file_itself() {
        let root = tempfile::tempdir().unwrap();
        write_lock(root.path(), r#"{"scripts/grade.js":"abc"}"#);
        let protected = Protected::load(root.path(), &[]);
        assert!(protected.covers("scripts/grade.js"));
        assert!(protected.covers(LOCK_FILE));
    }

    #[test]
    fn load_without_a_lock_file_protects_only_the_extra_paths() {
        let root = tempfile::tempdir().unwrap();
        let protected = Protected::load(root.path(), &["backlog/".to_owned()]);
        assert!(!protected.covers("scripts/grade.js"));
        assert!(!protected.covers(LOCK_FILE));
        assert!(protected.covers("backlog/backlog.json"));
    }

    #[test]
    fn load_with_a_broken_lock_file_adds_nothing() {
        let root = tempfile::tempdir().unwrap();
        write_lock(root.path(), r#"{"scripts/grade.js": 5}"#);
        assert_eq!(Protected::load(root.path(), &[]), Protected::default());
    }

    #[test]
    fn covers_matches_an_exact_path_and_a_folder_prefix_only() {
        let root = tempfile::tempdir().unwrap();
        let extra = ["scripts/".to_owned(), "README.md".to_owned()];
        let protected = Protected::load(root.path(), &extra);
        assert!(protected.covers("README.md"));
        assert!(protected.covers("scripts/grade.js"));
        assert!(!protected.covers("src/scripts.rs"));
        assert!(!protected.covers("docs/README.md"));
    }

    #[test]
    fn changed_reports_a_file_that_differs_or_is_missing_but_not_an_intact_one() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("scripts")).unwrap();
        fs::write(root.path().join("scripts/grade.js"), b"ok").unwrap();
        let hash = sha256_hex(b"ok");
        write_lock(root.path(), &format!(r#"{{"scripts/grade.js":"{hash}"}}"#));
        let protected = Protected::load(root.path(), &[]);
        assert!(protected.changed(root.path()).is_empty());

        fs::write(root.path().join("scripts/grade.js"), b"edited").unwrap();
        let expected = vec!["scripts/grade.js".to_owned()];
        assert_eq!(protected.changed(root.path()), expected);

        fs::remove_file(root.path().join("scripts/grade.js")).unwrap();
        assert_eq!(protected.changed(root.path()), expected);
    }

    #[test]
    fn a_diff_touching_a_protected_file_is_blocked_as_tamper() {
        let root = tempfile::tempdir().unwrap();
        write_lock(root.path(), r#"{"scripts/grade.js":"abc"}"#);
        let protected = Protected::load(root.path(), &[]);
        let diff = "diff --git a/scripts/grade.js b/scripts/grade.js\n\
                    --- a/scripts/grade.js\n\
                    +++ b/scripts/grade.js\n\
                    @@ -1 +1 @@\n-old\n+new\n";
        let verdict = tamper_verdict(&protected.touched_by_diff(diff)).unwrap();
        assert_eq!(verdict.status, TaskStatus::Blocked);
        assert!(verdict.reason.contains("tamper"), "{}", verdict.reason);
    }

    #[test]
    fn an_ordinary_source_change_is_not_tamper() {
        let root = tempfile::tempdir().unwrap();
        write_lock(root.path(), r#"{"scripts/grade.js":"abc"}"#);
        let protected = Protected::load(root.path(), &[]);
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n\
                    --- a/src/lib.rs\n\
                    +++ b/src/lib.rs\n\
                    @@ -1 +1 @@\n-old\n+new\n";
        assert!(protected.touched_by_diff(diff).is_empty());
        assert!(tamper_verdict(&protected.touched_by_diff(diff)).is_none());
    }
}
