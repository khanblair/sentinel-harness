//! The switch of each skill. The text of a skill lives in its `SKILL.md`, never in the store.

use std::collections::BTreeMap;

use rusqlite::params;
use sentinel_core::skills::SkillMode;

use crate::{Result, Store, enum_from_text, enum_text};

impl Store {
    /// Every skill that has a switch row. A skill without one is Auto.
    pub fn get_skill_modes(&self) -> Result<BTreeMap<String, SkillMode>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT name, mode FROM skills")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut modes = BTreeMap::new();
        for row in rows {
            let (name, mode) = row?;
            modes.insert(name, enum_from_text(&mode)?);
        }
        Ok(modes)
    }

    pub fn set_skill_mode(&self, name: &str, mode: SkillMode) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO skills (name, mode) VALUES (?1, ?2)
             ON CONFLICT(name) DO UPDATE SET mode = excluded.mode",
            params![name, enum_text(&mode)?],
        )?;
        Ok(())
    }

    pub fn delete_skill_mode(&self, name: &str) -> Result<()> {
        let connection = self.connection()?;
        connection.execute("DELETE FROM skills WHERE name = ?1", params![name])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skill_switch_round_trips_and_a_deleted_one_is_gone() {
        let store = Store::open_in_memory().unwrap();
        store.set_skill_mode("review", SkillMode::Always).unwrap();
        store.set_skill_mode("review", SkillMode::Off).unwrap();
        assert_eq!(
            store.get_skill_modes().unwrap().get("review"),
            Some(&SkillMode::Off)
        );
        store.delete_skill_mode("review").unwrap();
        assert!(store.get_skill_modes().unwrap().is_empty());
    }
}
