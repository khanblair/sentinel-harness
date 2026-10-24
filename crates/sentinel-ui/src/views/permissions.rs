//! The Permissions panel: the four modes and the per-agent capability matrix
//! (`docs/ui-rules.md`, Permissions and Approvals).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::PermissionsUpdate;
use sentinel_core::{AgentCapabilities, Capability, PermissionMode, ToolId, ToolInfo};

use crate::api;
use crate::components::icon::Icon;
use crate::components::permission_matrix::PermissionMatrix;
use crate::components::permission_mode_list::PermissionModeList;
use crate::components::top_bar::settle;
use crate::state::{Modal, Ui, use_ui};

/// The modes, least to most permissive.
pub const MODES: [PermissionMode; 4] = [
    PermissionMode::PlanOnly,
    PermissionMode::AskEachWave,
    PermissionMode::AutoInsideReserved,
    PermissionMode::Bypass,
];

/// The capability columns: the enum value, the full label, and the short header.
pub const CAPABILITY_COLUMNS: [(Capability, &str, &str); 6] = [
    (Capability::Read, "Read repository", "Read"),
    (Capability::Write, "Write reserved files", "Write"),
    (Capability::Ext, "Ask for more files", "Ask"),
    (Capability::Tests, "Run tests", "Tests"),
    (Capability::Shell, "Run shell commands", "Shell"),
    (Capability::Net, "Network access", "Net"),
];

/// One agent's row in the matrix. `allowed` lines up with `CAPABILITY_COLUMNS`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixRow {
    pub agent: ToolId,
    pub allowed: Vec<bool>,
}

/// The icon and the sentence under a mode card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeCopy {
    pub icon: &'static str,
    pub description: &'static str,
}

pub fn mode_copy(mode: PermissionMode) -> ModeCopy {
    match mode {
        PermissionMode::PlanOnly => ModeCopy {
            icon: "eye",
            description: "Agents read and plan. Nothing is written until you approve the plan.",
        },
        PermissionMode::AskEachWave => ModeCopy {
            icon: "shield",
            description: "You approve every wave and every scope extension before agents write.",
        },
        PermissionMode::AutoInsideReserved => ModeCopy {
            icon: "lock",
            description: "Writes to files a task has reserved run without asking. Asking for more files needs your OK.",
        },
        PermissionMode::Bypass => ModeCopy {
            icon: "alert",
            description: "No approval cards. Free files are granted on request. Locks and the test-edit guard still apply.",
        },
    }
}

/// Whether an agent may use one capability. Missing entries mean no.
pub fn is_allowed(caps: &[AgentCapabilities], agent: &ToolId, capability: Capability) -> bool {
    caps.iter()
        .find(|entry| &entry.agent == agent)
        .is_some_and(|entry| entry.allowed.contains(&capability))
}

/// The capability list with one switch flipped for one agent.
pub fn toggled(
    caps: &[AgentCapabilities],
    agent: &ToolId,
    capability: Capability,
) -> Vec<AgentCapabilities> {
    let mut next = caps.to_vec();
    match next.iter_mut().find(|entry| &entry.agent == agent) {
        Some(entry) if entry.allowed.contains(&capability) => {
            entry.allowed.retain(|held| *held != capability);
        }
        Some(entry) => entry.allowed.push(capability),
        None => next.push(AgentCapabilities {
            agent: agent.clone(),
            allowed: vec![capability],
        }),
    }
    next
}

/// One row per enabled tool, with a switch for each capability column.
pub fn matrix_rows(tools: &[ToolInfo], caps: &[AgentCapabilities]) -> Vec<MatrixRow> {
    tools
        .iter()
        .filter(|tool| tool.enabled)
        .map(|tool| MatrixRow {
            agent: tool.id.clone(),
            allowed: CAPABILITY_COLUMNS
                .iter()
                .map(|(capability, _, _)| is_allowed(caps, &tool.id, *capability))
                .collect(),
        })
        .collect()
}

fn send_update(ui: Ui, update: PermissionsUpdate) {
    spawn_local(async move {
        let result = api::send::<_, serde_json::Value>("PUT", "/api/permissions", &update).await;
        settle(ui, result);
    });
}

fn pick_mode(ui: Ui, mode: PermissionMode) {
    if mode == PermissionMode::Bypass {
        ui.modal.set(Some(Modal::BypassConfirm));
        return;
    }
    send_update(
        ui,
        PermissionsUpdate {
            mode: Some(mode),
            ..PermissionsUpdate::default()
        },
    );
}

fn pick_capability(ui: Ui, agent: &ToolId, capability: Capability) {
    let caps = ui
        .snapshot
        .with_untracked(|s| toggled(&s.capabilities, agent, capability));
    send_update(
        ui,
        PermissionsUpdate {
            capabilities: Some(caps),
            ..PermissionsUpdate::default()
        },
    );
}

#[component]
pub fn PermissionsPanel() -> impl IntoView {
    let ui = use_ui();
    let mode = Signal::derive(move || ui.snapshot.with(|s| s.permission_mode));
    let rows = Memo::new(move |_| ui.snapshot.with(|s| matrix_rows(&s.tools, &s.capabilities)));
    view! {
        <PermissionModeList selected=mode on_pick=Callback::new(move |picked| pick_mode(ui, picked)) />
        <div class="note" style="margin-bottom:.8em">
            <Icon name="lock" size=14 />
            <span>"Agents can only write files their task has reserved. No setting turns this off."</span>
        </div>
        <PermissionMatrix rows=rows on_toggle=Callback::new(move |(agent, capability)| pick_capability(ui, &agent, capability)) />
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(agent: &str, allowed: &[Capability]) -> AgentCapabilities {
        AgentCapabilities {
            agent: ToolId::new(agent),
            allowed: allowed.to_vec(),
        }
    }

    #[test]
    fn toggle_adds_then_removes_one_capability() {
        let start = vec![caps("claude", &[Capability::Read])];
        let added = toggled(&start, &ToolId::new("claude"), Capability::Tests);
        assert!(is_allowed(
            &added,
            &ToolId::new("claude"),
            Capability::Tests
        ));
        let removed = toggled(&added, &ToolId::new("claude"), Capability::Read);
        assert!(!is_allowed(
            &removed,
            &ToolId::new("claude"),
            Capability::Read
        ));
    }

    #[test]
    fn toggle_creates_a_row_for_an_agent_with_no_entry() {
        let next = toggled(&[], &ToolId::new("codex"), Capability::Net);
        assert_eq!(next, vec![caps("codex", &[Capability::Net])]);
    }

    #[test]
    fn matrix_has_one_row_per_enabled_tool() {
        let tool = |id: &str, enabled: bool| ToolInfo {
            id: ToolId::new(id),
            name: id.to_owned(),
            path: String::new(),
            version: String::new(),
            adapter: "generic".to_owned(),
            status: sentinel_core::ToolStatus::Ready,
            tier: sentinel_core::Tier::Cheap,
            enabled,
        };
        let tools = vec![tool("claude", true), tool("codex", false)];
        let rows = matrix_rows(&tools, &[caps("claude", &[Capability::Read])]);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].allowed,
            vec![true, false, false, false, false, false]
        );
    }

    #[test]
    fn bypass_has_a_copy_and_all_modes_have_icons() {
        for mode in MODES {
            assert_ne!(mode_copy(mode).description, "");
            assert_ne!(mode_copy(mode).icon, "");
        }
    }
}
