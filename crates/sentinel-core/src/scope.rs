//! The set of paths a task may write, and the math that decides whether two tasks collide.
//!
//! An entry is a file (`src/a.rs`) or a folder (`src/limits/`, `src/limits/*`, `src/limits/**`).
//! A folder covers every path below it.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Scope {
    entries: BTreeSet<String>,
}

enum Entry<'a> {
    File(&'a str),
    /// Folder prefix that always ends with `/`.
    Folder(String),
}

impl Scope {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a scope from entries. Fails on the first invalid entry.
    pub fn from_paths<I, S>(paths: I) -> Result<Self, CoreError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut scope = Self::new();
        for path in paths {
            scope.insert(path.as_ref())?;
        }
        Ok(scope)
    }

    /// Add one entry. Rejects empty, absolute, and `..` paths so a scope cannot leave the repo.
    pub fn insert(&mut self, path: &str) -> Result<(), CoreError> {
        let clean = path.strip_prefix("./").unwrap_or(path);
        let invalid = |reason| CoreError::InvalidPath {
            path: path.to_owned(),
            reason,
        };
        if clean.is_empty() {
            return Err(invalid("empty"));
        }
        if clean.starts_with('/') {
            return Err(invalid("absolute"));
        }
        if clean.split('/').any(|part| part == "..") {
            return Err(invalid("parent folder"));
        }
        self.entries.insert(clean.to_owned());
        Ok(())
    }

    pub fn entries(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// True when some entry of this scope and some entry of `other` can touch the same file.
    pub fn overlaps(&self, other: &Scope) -> bool {
        self.entries()
            .any(|a| other.entries().any(|b| entries_overlap(a, b)))
    }

    /// True when `path` (a concrete file) is covered by this scope.
    pub fn covers(&self, path: &str) -> bool {
        self.entries().any(|entry| match parse(entry) {
            Entry::File(file) => file == path,
            Entry::Folder(prefix) => path.starts_with(&prefix),
        })
    }
}

fn parse(entry: &str) -> Entry<'_> {
    for suffix in ["/**", "/*", "/"] {
        if let Some(folder) = entry.strip_suffix(suffix) {
            return Entry::Folder(format!("{folder}/"));
        }
    }
    Entry::File(entry)
}

fn entries_overlap(a: &str, b: &str) -> bool {
    match (parse(a), parse(b)) {
        (Entry::File(x), Entry::File(y)) => x == y,
        (Entry::File(file), Entry::Folder(prefix)) | (Entry::Folder(prefix), Entry::File(file)) => {
            file.starts_with(&prefix)
        }
        (Entry::Folder(x), Entry::Folder(y)) => x.starts_with(&y) || y.starts_with(&x),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(paths: &[&str]) -> Scope {
        Scope::from_paths(paths).unwrap()
    }

    #[test]
    fn same_file_overlaps() {
        assert!(scope(&["src/a.rs"]).overlaps(&scope(&["src/a.rs", "src/b.rs"])));
    }

    #[test]
    fn different_files_do_not_overlap() {
        assert!(!scope(&["src/a.rs"]).overlaps(&scope(&["src/b.rs"])));
    }

    #[test]
    fn folder_overlaps_a_file_below_it() {
        assert!(scope(&["src/limits/*"]).overlaps(&scope(&["src/limits/daily.rs"])));
        assert!(scope(&["src/limits/daily.rs"]).overlaps(&scope(&["src/limits/"])));
    }

    #[test]
    fn nested_folders_overlap_but_siblings_do_not() {
        assert!(scope(&["src/"]).overlaps(&scope(&["src/api/**"])));
        assert!(!scope(&["src/api/"]).overlaps(&scope(&["src/apiv2/"])));
    }

    #[test]
    fn empty_scope_overlaps_nothing() {
        assert!(!Scope::new().overlaps(&scope(&["src/"])));
    }

    #[test]
    fn covers_matches_files_and_folders() {
        let s = scope(&["src/a.rs", "docs/*"]);
        assert!(s.covers("src/a.rs"));
        assert!(s.covers("docs/x/y.md"));
        assert!(!s.covers("src/b.rs"));
    }

    #[test]
    fn insert_rejects_paths_that_leave_the_repo() {
        let mut s = Scope::new();
        assert!(s.insert("").is_err());
        assert!(s.insert("/etc/passwd").is_err());
        assert!(s.insert("src/../../x").is_err());
        assert!(s.insert("./src/a.rs").is_ok());
        assert!(s.covers("src/a.rs"));
    }

    #[test]
    fn serializes_as_a_plain_list() {
        let json = serde_json::to_string(&scope(&["b.rs", "a.rs"])).unwrap();
        assert_eq!(json, "[\"a.rs\",\"b.rs\"]");
    }
}
