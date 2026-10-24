//! Slop check: banned words and patterns in docs, UI copy, and comments.

use std::path::Path;

use crate::finding::CheckResult;

pub fn run(_root: &Path) -> CheckResult {
    // TODO(P0-03): implement this check.
    Ok(Vec::new())
}
