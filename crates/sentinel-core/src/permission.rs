//! Permission modes, per-agent capabilities, and approval cards.

use serde::{Deserialize, Serialize};

use crate::ids::{ApprovalId, TaskId, ToolId};

/// How much agents may do without asking. Reserved files stay the hard boundary in every mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    PlanOnly,
    AskEachWave,
    #[default]
    AutoInsideReserved,
    Bypass,
}

impl PermissionMode {
    /// The exact name from `docs/product-scope.md`, F15.
    pub fn label(self) -> &'static str {
        match self {
            PermissionMode::PlanOnly => "Plan only",
            PermissionMode::AskEachWave => "Ask each wave",
            PermissionMode::AutoInsideReserved => "Auto inside reserved files",
            PermissionMode::Bypass => "Bypass permissions",
        }
    }

    /// A request for one more free file becomes an approval card in every mode except Bypass.
    pub fn asks_for_extension(self) -> bool {
        self != PermissionMode::Bypass
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Read,
    Write,
    /// Ask for more files.
    Ext,
    Tests,
    Shell,
    Net,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalKind {
    Extension,
    Wave,
    Plan,
    Decision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalState {
    Pending,
    Allowed,
    Denied,
    /// Nobody answered before the timeout.
    Defaulted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approval {
    pub id: ApprovalId,
    pub kind: ApprovalKind,
    pub task: Option<TaskId>,
    pub agent: Option<ToolId>,
    pub path: Option<String>,
    pub state: ApprovalState,
    /// A device name, or `default`.
    pub decided_by: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mode_is_auto_inside_reserved_files() {
        assert_eq!(
            PermissionMode::default(),
            PermissionMode::AutoInsideReserved
        );
    }

    #[test]
    fn only_bypass_skips_extension_cards() {
        assert!(PermissionMode::PlanOnly.asks_for_extension());
        assert!(PermissionMode::AutoInsideReserved.asks_for_extension());
        assert!(!PermissionMode::Bypass.asks_for_extension());
    }

    #[test]
    fn labels_match_the_docs() {
        assert_eq!(PermissionMode::Bypass.label(), "Bypass permissions");
        assert_eq!(
            PermissionMode::AutoInsideReserved.label(),
            "Auto inside reserved files"
        );
    }
}
