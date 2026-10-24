//! What a permission mode asks for, straight from F15 and the disposition
//! table in `docs/architecture.md`.

use sentinel_core::{AgentCapabilities, Capability, PermissionMode, ToolId};

/// How an extension request plays out. Bypass grants and logs; every other
/// mode raises an approval card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionAction {
    AskCard,
    GrantAndLog,
}

/// Whether a wave start needs an approval card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveGate {
    None,
    Card,
}

/// Bypass is the only mode that grants a free path. This matches
/// `PermissionMode::asks_for_extension`.
pub fn extension_action(mode: PermissionMode) -> ExtensionAction {
    if mode == PermissionMode::Bypass {
        ExtensionAction::GrantAndLog
    } else {
        ExtensionAction::AskCard
    }
}

/// Ask each wave raises one card per wave; other modes never do.
pub fn wave_gate(mode: PermissionMode) -> WaveGate {
    match mode {
        PermissionMode::AskEachWave => WaveGate::Card,
        PermissionMode::PlanOnly | PermissionMode::AutoInsideReserved | PermissionMode::Bypass => {
            WaveGate::None
        }
    }
}

/// Plan only holds every run behind one card after the plan, before writing.
pub fn plan_gate(mode: PermissionMode) -> bool {
    mode == PermissionMode::PlanOnly
}

/// After the plan is approved a Plan only run continues under Auto inside
/// reserved files; other modes stay what they are.
pub fn next_mode_after_plan_approval(mode: PermissionMode) -> PermissionMode {
    match mode {
        PermissionMode::PlanOnly => PermissionMode::AutoInsideReserved,
        PermissionMode::AskEachWave
        | PermissionMode::AutoInsideReserved
        | PermissionMode::Bypass => mode,
    }
}

/// Whether one agent may use one capability. An agent row with the capability
/// absent means off, and off wins over even Bypass. An agent with no row is
/// treated as having the default set.
pub fn capability_allows(caps: &[AgentCapabilities], agent: &ToolId, need: Capability) -> bool {
    match caps.iter().find(|row| &row.agent == agent) {
        Some(row) => row.allowed.contains(&need),
        None => default_capabilities(agent).allowed.contains(&need),
    }
}

/// Read, write, ask for more files, and run tests are on; shell and network
/// are off until the user opens them.
pub fn default_capabilities(agent: &ToolId) -> AgentCapabilities {
    AgentCapabilities {
        agent: agent.clone(),
        allowed: vec![
            Capability::Read,
            Capability::Write,
            Capability::Ext,
            Capability::Tests,
        ],
    }
}

/// A write to an unreserved file is never an approval. It is blocked and
/// restored in every mode; this exists so a test can pin the rule.
pub const fn write_is_never_an_approval() -> bool {
    true
}
