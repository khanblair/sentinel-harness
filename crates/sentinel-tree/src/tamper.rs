//! Detect edits to existing test files between two commits. Added tests are allowed.

use std::collections::BTreeSet;

use crate::Result;
use crate::git::Git;

pub fn edited_tests(git: &Git, base: &str, head: &str) -> Result<Vec<String>> {
    let raw = git.run_bytes(&[
        "diff-tree",
        "-r",
        "-z",
        "--name-status",
        "--no-renames",
        base,
        head,
    ])?;
    Ok(changed_tests(&raw))
}

fn changed_tests(raw: &[u8]) -> Vec<String> {
    let mut tests = BTreeSet::new();
    let mut records = raw.split(|byte| *byte == 0);
    while let Some(status) = records.next() {
        let Some(path) = records.next() else {
            break;
        };
        if status.is_empty() || status.first() == Some(&b'A') {
            continue;
        }
        let path = String::from_utf8_lossy(path);
        if looks_like_test(&path) {
            tests.insert(path.into_owned());
        }
    }
    tests.into_iter().collect()
}

/// True when the path sits in a test folder or its name follows a test naming pattern.
pub fn looks_like_test(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    let Some((name, folders)) = parts.split_last() else {
        return false;
    };
    let in_test_folder = folders
        .iter()
        .any(|folder| matches!(*folder, "tests" | "test" | "__tests__" | "spec"));
    in_test_folder || test_name(name)
}

fn test_name(name: &str) -> bool {
    name.ends_with("Test.java")
        || name.ends_with("Tests.cs")
        || name.ends_with("Test.kt")
        || name.contains("_test.")
        || name.contains(".test.")
        || name.contains(".spec.")
        || (name.starts_with("test_") && name.contains('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_like_test_matches_folders_and_test_names() {
        let flagged = [
            "tests/a.rs",
            "src/test/java/Foo.java",
            "app/__tests__/x.js",
            "spec/a_spec.rb",
            "src/foo_test.rs",
            "src/foo.test.ts",
            "src/foo.spec.ts",
            "src/test_helpers.rs",
            "src/FooTest.java",
            "src/FooTests.cs",
            "src/FooTest.kt",
        ];
        for path in flagged {
            assert!(looks_like_test(path), "{path} should count as a test");
        }
    }

    #[test]
    fn looks_like_test_ignores_plain_source_files() {
        let plain = [
            "src/lib.rs",
            "src/contest.rs",
            "src/attestation.rs",
            "docs/testing.md",
            "tests",
        ];
        for path in plain {
            assert!(!looks_like_test(path), "{path} should not count as a test");
        }
    }
}
