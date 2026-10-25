//! What fills an agent's context window, as the Context panel shows it. Every count is an
//! estimate: one token is about four characters.

use serde::{Deserialize, Serialize};

/// The window of the agent that runs the next task, and the parts that fill it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextUsage {
    /// The agent routed for the `mid` tier, or `none` when no agent is routed.
    pub agent: String,
    pub window_tokens: u32,
    /// Every item except `free`.
    pub used_tokens: u32,
    /// The parts of the window in panel order. The last one is `free`.
    pub items: Vec<ContextItem>,
    /// The Brain notes a run would load now.
    pub notes: Vec<ContextEntry>,
    /// The enabled skills.
    pub skills: Vec<ContextEntry>,
    /// The files, folders, and uploads waiting for the next message.
    pub attachments: Vec<ContextEntry>,
}

/// One part of the window, such as `messages` or `free`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextItem {
    pub kind: String,
    pub label: String,
    pub tokens: u32,
}

/// A named piece of context, such as one note or one skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextEntry {
    pub name: String,
    pub tokens: u32,
}
