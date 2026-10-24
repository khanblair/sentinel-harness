//! A worker CLI found on this computer.

use serde::{Deserialize, Serialize};

use crate::ids::ToolId;
use crate::task::Tier;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Ready,
    NeedsLogin,
    /// Unknown tool: the user must confirm a command template.
    NeedsCommand,
    Missing,
    Failed,
}

impl ToolStatus {
    /// Only a ready tool can take work.
    pub fn is_ready(self) -> bool {
        self == ToolStatus::Ready
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInfo {
    pub id: ToolId,
    pub name: String,
    pub path: String,
    pub version: String,
    /// Name of the adapter profile, such as `claude`, or `generic`.
    pub adapter: String,
    pub status: ToolStatus,
    pub tier: Tier,
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_ready_tools_take_work() {
        assert!(ToolStatus::Ready.is_ready());
        assert!(!ToolStatus::NeedsLogin.is_ready());
        assert!(!ToolStatus::Missing.is_ready());
    }
}
