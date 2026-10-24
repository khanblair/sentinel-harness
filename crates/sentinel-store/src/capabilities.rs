//! The per-agent capability matrix. Rows are stored even when a capability is
//! off, so an explicit off survives the default set and wins over the mode.

use rusqlite::{Row, params};
use sentinel_core::{AgentCapabilities, Capability, ToolId};

use crate::{Result, Store, enum_from_text, enum_text};

/// The display order for capabilities inside one agent row.
const CAPABILITY_ORDER: [Capability; 6] = [
    Capability::Read,
    Capability::Write,
    Capability::Ext,
    Capability::Tests,
    Capability::Shell,
    Capability::Net,
];

impl Store {
    pub fn set_capability(
        &self,
        agent: &ToolId,
        capability: Capability,
        allowed: bool,
    ) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT OR REPLACE INTO permissions (agent_id, capability, allowed)
             VALUES (?1, ?2, ?3)",
            params![agent.as_str(), enum_text(&capability)?, allowed],
        )?;
        Ok(())
    }

    /// Every agent that has a row, with its allowed capabilities. Agents sort
    /// by id, capabilities follow the Read, Write, Ext, Tests, Shell, Net order.
    pub fn list_capabilities(&self) -> Result<Vec<AgentCapabilities>> {
        let rows = read_allowed_rows(self)?;
        let mut groups = group_by_agent(rows)?;
        groups.sort_by(|left, right| left.0.cmp(&right.0));
        groups
            .into_iter()
            .map(|(agent, allowed)| {
                Ok(AgentCapabilities {
                    agent: ToolId::new(agent),
                    allowed,
                })
            })
            .collect()
    }
}

type PermissionRow = (String, String);
type AgentGroup = (String, Vec<Capability>);

fn read_allowed_rows(store: &Store) -> Result<Vec<PermissionRow>> {
    let connection = store.connection()?;
    let mut statement = connection.prepare(
        "SELECT agent_id, capability FROM permissions WHERE allowed = 1 ORDER BY agent_id",
    )?;
    let mapped = statement.query_map([], read_permission_row)?;
    mapped
        .collect::<rusqlite::Result<Vec<PermissionRow>>>()
        .map_err(Into::into)
}

fn group_by_agent(rows: Vec<PermissionRow>) -> Result<Vec<AgentGroup>> {
    let mut groups: Vec<AgentGroup> = Vec::new();
    for (agent, stored) in rows {
        let capability: Capability = enum_from_text(&stored)?;
        push_capability(&mut groups, agent, capability);
    }
    sort_capabilities(&mut groups);
    Ok(groups)
}

fn push_capability(groups: &mut Vec<AgentGroup>, agent: String, capability: Capability) {
    if let Some(existing) = groups.iter_mut().find(|(known, _)| known == &agent) {
        existing.1.push(capability);
        return;
    }
    groups.push((agent, vec![capability]));
}

fn sort_capabilities(groups: &mut [AgentGroup]) {
    for (_, allowed) in groups {
        allowed.sort_by_key(|c| capability_rank(*c));
    }
}

fn read_permission_row(row: &Row<'_>) -> rusqlite::Result<PermissionRow> {
    Ok((row.get(0)?, row.get(1)?))
}

/// Position in the display order, or the end for a stored text with no rank.
fn capability_rank(capability: Capability) -> usize {
    CAPABILITY_ORDER
        .iter()
        .position(|known| known == &capability)
        .unwrap_or(usize::MAX)
}
