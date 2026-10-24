//! Paired device queries.

use rusqlite::{OptionalExtension, Row, params};
use sentinel_core::DeviceId;
use time::OffsetDateTime;

use crate::{Result, Store, StoreError, enum_from_text, enum_text, time_from_text, time_text};

pub use sentinel_core::DeviceKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: DeviceId,
    pub name: String,
    pub kind: DeviceKind,
    pub token_hash: String,
    pub created_at: OffsetDateTime,
    pub revoked_at: Option<OffsetDateTime>,
}

const DEVICE_COLUMNS: &str = "id, name, kind, token_hash, created_at, revoked_at";

type DeviceRow = (String, String, String, String, String, Option<String>);

impl Store {
    pub fn insert_device(&self, device: &Device) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO devices (id, name, kind, token_hash, created_at, revoked_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                device.id.as_str(),
                device.name,
                enum_text(&device.kind)?,
                device.token_hash,
                time_text(device.created_at)?,
                device.revoked_at.map(time_text).transpose()?,
            ],
        )?;
        Ok(())
    }

    pub fn list_devices(&self) -> Result<Vec<Device>> {
        let connection = self.connection()?;
        let rows = {
            let mut statement = connection.prepare(&format!(
                "SELECT {DEVICE_COLUMNS} FROM devices ORDER BY created_at, id"
            ))?;
            let mapped = statement.query_map([], read_device_row)?;
            mapped.collect::<rusqlite::Result<Vec<DeviceRow>>>()?
        };
        rows.into_iter().map(device_from_row).collect()
    }

    pub fn revoke_device(&self, id: &DeviceId, at: OffsetDateTime) -> Result<()> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE devices SET revoked_at = ?1 WHERE id = ?2",
            params![time_text(at)?, id.as_str()],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    /// A revoked device is never returned.
    pub fn device_by_token_hash(&self, token_hash: &str) -> Result<Option<Device>> {
        let connection = self.connection()?;
        let row = connection
            .query_row(
                &format!(
                    "SELECT {DEVICE_COLUMNS} FROM devices
                     WHERE token_hash = ?1 AND revoked_at IS NULL"
                ),
                params![token_hash],
                read_device_row,
            )
            .optional()?;
        row.map(device_from_row).transpose()
    }
}

fn read_device_row(row: &Row<'_>) -> rusqlite::Result<DeviceRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
    ))
}

fn device_from_row(row: DeviceRow) -> Result<Device> {
    let (id, name, kind, token_hash, created_at, revoked_at) = row;
    Ok(Device {
        id: DeviceId::new(id),
        name,
        kind: enum_from_text(&kind)?,
        token_hash,
        created_at: time_from_text(&created_at)?,
        revoked_at: revoked_at.as_deref().map(time_from_text).transpose()?,
    })
}
