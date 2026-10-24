//! Detected worker CLIs. A rescan rewrites what it found but never changes the
//! enabled flag the user set.

use rusqlite::{OptionalExtension, Row, params};
use sentinel_core::{ToolId, ToolInfo};

use crate::{Result, Store, StoreError, enum_from_text, enum_text};

const TOOL_COLUMNS: &str = "id, name, path, version, adapter, status, tier, enabled";

impl Store {
    /// Stores one scan result. The `enabled` flag survives a rescan of a tool
    /// the user already switched off or on by hand.
    pub fn upsert_tool(&self, tool: &ToolInfo) -> Result<()> {
        let connection = self.connection()?;
        let enabled = match current_enabled(&connection, tool)? {
            Some(kept) => kept,
            None => tool.enabled,
        };
        connection.execute(
            "INSERT INTO tools (id, name, path, version, adapter, status, tier, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                path = excluded.path,
                version = excluded.version,
                adapter = excluded.adapter,
                status = excluded.status,
                tier = excluded.tier,
                enabled = excluded.enabled",
            params![
                tool.id.as_str(),
                tool.name,
                tool.path,
                tool.version,
                tool.adapter,
                enum_text(&tool.status)?,
                enum_text(&tool.tier)?,
                enabled
            ],
        )?;
        Ok(())
    }

    /// Every tool, sorted by id.
    pub fn list_tools(&self) -> Result<Vec<ToolInfo>> {
        let connection = self.connection()?;
        let rows = {
            let mut statement =
                connection.prepare(&format!("SELECT {TOOL_COLUMNS} FROM tools ORDER BY id"))?;
            let mapped = statement.query_map([], read_tool_row)?;
            mapped.collect::<rusqlite::Result<Vec<ToolRow>>>()?
        };
        rows.into_iter().map(tool_from_row).collect()
    }

    pub fn set_tool_enabled(&self, id: &ToolId, enabled: bool) -> Result<()> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE tools SET enabled = ?1 WHERE id = ?2",
            params![enabled, id.as_str()],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound(format!("tool {}", id.as_str())));
        }
        Ok(())
    }
}

fn current_enabled(connection: &rusqlite::Connection, tool: &ToolInfo) -> Result<Option<bool>> {
    connection
        .query_row(
            "SELECT enabled FROM tools WHERE id = ?1",
            params![tool.id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(StoreError::Sqlite)
}

fn read_tool_row(row: &Row<'_>) -> rusqlite::Result<ToolRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
    ))
}

fn tool_from_row(row: ToolRow) -> Result<ToolInfo> {
    let (id, name, path, version, adapter, status, tier, enabled) = row;
    Ok(ToolInfo {
        id: ToolId::new(id),
        name,
        path,
        version,
        adapter,
        status: enum_from_text(&status)?,
        tier: enum_from_text(&tier)?,
        enabled,
    })
}

type ToolRow = (String, String, String, String, String, String, String, bool);
