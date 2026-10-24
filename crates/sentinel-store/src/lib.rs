//! `SQLite` store for Sentinel state: tasks, reservations, runs, events, devices, settings.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::Connection;
use rusqlite::types::Type;
use rusqlite_migration::{M, Migrations};
use sentinel_core::CoreError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

pub mod approvals;
pub mod barriers;
pub mod capabilities;
pub mod checkpoints;
pub mod decisions;
pub mod devices;
pub mod events;
pub mod quality;
pub mod reservations;
pub mod reset;
pub mod runs;
pub mod settings;
pub mod tasks;
pub mod tools;
pub mod verifications;

pub use barriers::BarrierRecord;
pub use checkpoints::CheckpointRecord;
pub use devices::{Device, DeviceKind};
pub use runs::RunRecord;
pub use verifications::VerificationRecord;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("migration error: {0}")]
    Migration(#[from] rusqlite_migration::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("core error: {0}")]
    Core(#[from] CoreError),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("store lock is poisoned")]
    Poisoned,
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// A cheap clone that shares one connection behind a lock.
#[derive(Clone)]
pub struct Store {
    connection: Arc<Mutex<Connection>>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        Store::from_connection(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Store> {
        Store::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut connection: Connection) -> Result<Store> {
        configure(&connection)?;
        migrations().to_latest(&mut connection)?;
        Ok(Store {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub(crate) fn connection(&self) -> Result<MutexGuard<'_, Connection>> {
        self.connection.lock().map_err(|_| StoreError::Poisoned)
    }
}

fn configure(connection: &Connection) -> Result<()> {
    // WAL is not available for in-memory databases, so a failure here is fine.
    let _ = connection.execute_batch("PRAGMA journal_mode=WAL;");
    connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;")?;
    Ok(())
}

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(include_str!("migrations/0001_init.sql")),
        M::up(include_str!("migrations/0002_extras.sql")),
    ])
}

/// The serde `snake_case` name of an enum, stored as text.
pub(crate) fn enum_text<T: Serialize>(value: &T) -> Result<String> {
    let json = serde_json::to_value(value)?;
    serde_json::from_value(json).map_err(StoreError::Json)
}

/// Parse an enum back from its serde `snake_case` name.
pub(crate) fn enum_from_text<T: DeserializeOwned>(text: &str) -> Result<T> {
    serde_json::from_value(serde_json::Value::String(text.to_owned())).map_err(StoreError::Json)
}

pub(crate) fn time_text(value: OffsetDateTime) -> Result<String> {
    value
        .to_offset(UtcOffset::UTC)
        .format(&Rfc3339)
        .map_err(|error| {
            StoreError::Sqlite(rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
        })
}

pub(crate) fn time_from_text(text: &str) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(text, &Rfc3339).map_err(|error| {
        StoreError::Sqlite(rusqlite::Error::FromSqlConversionFailure(
            0,
            Type::Text,
            Box::new(error),
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_text_stores_utc_rfc3339() {
        let local = OffsetDateTime::from_unix_timestamp(1_700_000_000)
            .unwrap()
            .to_offset(UtcOffset::from_hms(2, 0, 0).unwrap());
        assert_eq!(time_text(local).unwrap(), "2023-11-14T22:13:20Z");
    }
}
