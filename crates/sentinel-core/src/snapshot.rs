//! The full state the daemon sends to a device on connect (`GET /api/state`). The UI builds every
//! view from this plus the live events that follow on the WebSocket. One definition for both
//! sides means no schema drift.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::cost::Cost;
use crate::event::EventRecord;
use crate::ids::{DeviceId, RunId, ToolId};
use crate::note::BrainMode;
use crate::permission::{Approval, Capability, PermissionMode};
use crate::reservation::Reservation;
use crate::state::{Decision, HarnessState, TailscaleState};
use crate::task::{Dependency, Task, Tier};
use crate::tool::ToolInfo;

/// Which agent serves each tier. Set in Settings, never in the setup list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Routing {
    pub cheap: Option<ToolId>,
    pub mid: Option<ToolId>,
    pub smart: Option<ToolId>,
}

impl Routing {
    pub fn for_tier(&self, tier: Tier) -> Option<&ToolId> {
        match tier {
            Tier::Cheap => self.cheap.as_ref(),
            Tier::Mid => self.mid.as_ref(),
            Tier::Smart => self.smart.as_ref(),
        }
    }
}

/// What one agent may do. A capability that is off wins over the permission mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCapabilities {
    pub agent: ToolId,
    pub allowed: Vec<Capability>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostSnapshot {
    pub total: Cost,
    pub cap: Cost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCost {
    pub agent: ToolId,
    pub cost: Cost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Phone,
    Desktop,
    Telegram,
}

/// A paired device. The token hash never leaves the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: DeviceId,
    pub name: String,
    pub kind: DeviceKind,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub revoked: bool,
}

/// The repository this daemon works on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoInfo {
    pub path: String,
    pub name: String,
    pub is_git: bool,
    pub has_backlog: bool,
    pub clean: bool,
    pub backlog_tasks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunInfo {
    pub id: RunId,
    pub backlog: String,
    pub wave: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
}

/// Progress through install check and the five onboarding steps.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnboardingState {
    /// 0 is the install check, 1 to 5 are the steps.
    pub step: u8,
    pub complete: bool,
    pub skipped: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub harness: HarnessState,
    pub repo: Option<RepoInfo>,
    pub run: Option<RunInfo>,
    pub tasks: Vec<Task>,
    pub dependencies: Vec<Dependency>,
    /// Granted and waiting reservations only.
    pub reservations: Vec<Reservation>,
    pub tools: Vec<ToolInfo>,
    pub routing: Routing,
    pub permission_mode: PermissionMode,
    pub capabilities: Vec<AgentCapabilities>,
    pub brain_mode: BrainMode,
    pub cost: CostSnapshot,
    pub per_agent_cost: Vec<AgentCost>,
    /// Pending approvals first, then recent answered ones.
    pub approvals: Vec<Approval>,
    pub decisions: Vec<Decision>,
    pub devices: Vec<DeviceInfo>,
    pub tailscale: TailscaleState,
    pub onboarding: OnboardingState,
    /// True when the live-run privacy rules apply: Brain read-only, Telegram status only.
    pub hackathon_mode: bool,
    /// The most recent events, oldest first. The stream is built from these.
    pub events: Vec<EventRecord>,
    pub last_event_id: i64,
    /// Free-form settings the UI shows, such as `test_command` and `notes_token_cap`.
    pub settings: BTreeMap<String, String>,
}

impl Snapshot {
    /// An empty snapshot, used before the daemon has answered and in tests.
    pub fn empty() -> Self {
        Self {
            harness: HarnessState::Idle,
            repo: None,
            run: None,
            tasks: Vec::new(),
            dependencies: Vec::new(),
            reservations: Vec::new(),
            tools: Vec::new(),
            routing: Routing::default(),
            permission_mode: PermissionMode::default(),
            capabilities: Vec::new(),
            brain_mode: BrainMode::ReadOnly,
            cost: CostSnapshot::default(),
            per_agent_cost: Vec::new(),
            approvals: Vec::new(),
            decisions: Vec::new(),
            devices: Vec::new(),
            tailscale: TailscaleState::NotInstalled,
            onboarding: OnboardingState::default(),
            hackathon_mode: false,
            events: Vec::new(),
            last_event_id: 0,
            settings: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_snapshot_round_trips_through_json() {
        let snapshot = Snapshot::empty();
        let json = serde_json::to_string(&snapshot).unwrap();
        assert_eq!(serde_json::from_str::<Snapshot>(&json).unwrap(), snapshot);
    }

    #[test]
    fn routing_returns_the_tool_for_each_tier() {
        let routing = Routing {
            cheap: Some(ToolId::new("opencode")),
            mid: None,
            smart: Some(ToolId::new("claude")),
        };
        assert_eq!(
            routing.for_tier(Tier::Cheap),
            Some(&ToolId::new("opencode"))
        );
        assert_eq!(routing.for_tier(Tier::Mid), None);
        assert_eq!(routing.for_tier(Tier::Smart), Some(&ToolId::new("claude")));
    }
}
