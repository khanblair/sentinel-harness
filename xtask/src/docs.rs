//! Doc check: docs/project-structure.md matches the files in the repo.

use std::path::Path;

use crate::finding::CheckResult;

pub fn run(_root: &Path) -> CheckResult {
    // TODO(P0-04): implement this check.
    Ok(Vec::new())
}
