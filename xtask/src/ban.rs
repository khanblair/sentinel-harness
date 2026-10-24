//! Ban check: no git worktree use and no banned crates.

use std::path::Path;

use crate::finding::CheckResult;

pub fn run(_root: &Path) -> CheckResult {
    // TODO(P0-04): implement this check.
    Ok(Vec::new())
}
