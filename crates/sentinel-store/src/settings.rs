//! Key/value settings.

use rusqlite::{OptionalExtension, params};

use crate::{Result, Store};

impl Store {
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let connection = self.connection()?;
        let value = connection
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}
