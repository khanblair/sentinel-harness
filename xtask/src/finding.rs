//! The result type every check shares.

use std::fmt;
use std::path::PathBuf;

/// One problem a check found. `line` is 1-based, or 0 when the problem is about a whole file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub check: &'static str,
    pub path: PathBuf,
    pub line: usize,
    pub message: String,
}

impl Finding {
    pub fn new(
        check: &'static str,
        path: impl Into<PathBuf>,
        line: usize,
        message: impl Into<String>,
    ) -> Self {
        Self {
            check,
            path: path.into(),
            line,
            message: message.into(),
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: [{}] {}",
            self.path.display(),
            self.line,
            self.check,
            self.message
        )
    }
}

/// Every check has the shape `fn run(root: &Path) -> CheckResult`. An empty list means pass.
pub type CheckResult = anyhow::Result<Vec<Finding>>;
