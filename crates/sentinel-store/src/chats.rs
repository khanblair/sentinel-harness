//! Chats: the conversations of a project. The engine adds the run state to each row.

use rusqlite::{OptionalExtension, Row, params};
use sentinel_core::ChatId;
use time::OffsetDateTime;

use crate::{Result, Store, time_from_text, time_text};

/// One stored chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRecord {
    pub id: ChatId,
    pub project: String,
    pub title: String,
    /// True once the user named the chat, so a first message does not rename it.
    pub title_set: bool,
    pub pinned: bool,
    pub archived_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

const COLUMNS: &str = "id, project, title, title_set, pinned, archived_at, created_at, updated_at";

type ChatRow = (
    String,
    String,
    String,
    bool,
    bool,
    Option<String>,
    String,
    String,
);

impl Store {
    pub fn insert_chat(&self, chat: &ChatRecord) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            &format!("INSERT INTO chats ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"),
            params![
                chat.id.as_str(),
                chat.project,
                chat.title,
                chat.title_set,
                chat.pinned,
                chat.archived_at.map(time_text).transpose()?,
                time_text(chat.created_at)?,
                time_text(chat.updated_at)?,
            ],
        )?;
        Ok(())
    }

    pub fn get_chat(&self, id: &ChatId) -> Result<Option<ChatRecord>> {
        let connection = self.connection()?;
        let row = connection
            .query_row(
                &format!("SELECT {COLUMNS} FROM chats WHERE id = ?1"),
                params![id.as_str()],
                read_row,
            )
            .optional()?;
        row.map(chat_from_row).transpose()
    }

    /// Pinned chats first, then the most recently changed first.
    pub fn list_chats(&self) -> Result<Vec<ChatRecord>> {
        let connection = self.connection()?;
        let rows = {
            let mut statement = connection.prepare(&format!(
                "SELECT {COLUMNS} FROM chats ORDER BY pinned DESC, updated_at DESC, id"
            ))?;
            let mapped = statement.query_map([], read_row)?;
            mapped.collect::<rusqlite::Result<Vec<ChatRow>>>()?
        };
        rows.into_iter().map(chat_from_row).collect()
    }

    /// Replace the title, the flags, and the archive time of a chat.
    pub fn update_chat(&self, chat: &ChatRecord) -> Result<()> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE chats SET title = ?2, title_set = ?3, pinned = ?4, archived_at = ?5, \
             updated_at = ?6 WHERE id = ?1",
            params![
                chat.id.as_str(),
                chat.title,
                chat.title_set,
                chat.pinned,
                chat.archived_at.map(time_text).transpose()?,
                time_text(chat.updated_at)?,
            ],
        )?;
        if changed == 0 {
            return Err(crate::StoreError::NotFound(format!("chat {}", chat.id)));
        }
        Ok(())
    }

    /// Mark a chat as changed now, so it sorts first.
    pub fn touch_chat(&self, id: &ChatId, at: OffsetDateTime) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            "UPDATE chats SET updated_at = ?2 WHERE id = ?1",
            params![id.as_str(), time_text(at)?],
        )?;
        Ok(())
    }

    /// Remove a chat with its events and its attachment rows.
    pub fn delete_chat(&self, id: &ChatId) -> Result<()> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM events WHERE chat_id = ?1",
            params![id.as_str()],
        )?;
        transaction.execute(
            "DELETE FROM attachments WHERE chat_id = ?1",
            params![id.as_str()],
        )?;
        let removed =
            transaction.execute("DELETE FROM chats WHERE id = ?1", params![id.as_str()])?;
        if removed == 0 {
            return Err(crate::StoreError::NotFound(format!("chat {id}")));
        }
        transaction.commit()?;
        Ok(())
    }

    /// Give every event that has no chat to this chat. Used once, for a store from before chats.
    pub fn adopt_chatless_events(&self, id: &ChatId) -> Result<usize> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE events SET chat_id = ?1 WHERE chat_id IS NULL AND kind != 'AgentOutput' \
             AND kind NOT IN ('ToolDetected', 'TailscaleChanged', 'ConnectionChanged')",
            params![id.as_str()],
        )?;
        Ok(changed)
    }

    /// The verified and total counts of the last `RunFinished` of a chat.
    pub fn last_finish_counts(&self, id: &ChatId) -> Result<Option<(u32, u32)>> {
        let connection = self.connection()?;
        let counts = connection
            .query_row(
                "SELECT json_extract(payload_json, '$.verified'), json_extract(payload_json, '$.total') \
                 FROM events WHERE chat_id = ?1 AND kind = 'RunFinished' ORDER BY id DESC LIMIT 1",
                params![id.as_str()],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, u32>(1)?)),
            )
            .optional()?;
        Ok(counts)
    }

    /// True when the chat has a `RunStarted` event.
    pub fn chat_has_run(&self, id: &ChatId) -> Result<bool> {
        let connection = self.connection()?;
        let found: Option<i64> = connection
            .query_row(
                "SELECT 1 FROM events WHERE chat_id = ?1 AND kind = 'RunStarted' LIMIT 1",
                params![id.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        Ok(found.is_some())
    }
}

fn read_row(row: &Row<'_>) -> rusqlite::Result<ChatRow> {
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

fn chat_from_row(row: ChatRow) -> Result<ChatRecord> {
    let (id, project, title, title_set, pinned, archived_at, created_at, updated_at) = row;
    Ok(ChatRecord {
        id: ChatId::new(id),
        project,
        title,
        title_set,
        pinned,
        archived_at: archived_at.as_deref().map(time_from_text).transpose()?,
        created_at: time_from_text(&created_at)?,
        updated_at: time_from_text(&updated_at)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(id: &str, pinned: bool, at: i64) -> ChatRecord {
        let moment = OffsetDateTime::from_unix_timestamp(at).unwrap();
        ChatRecord {
            id: ChatId::new(id),
            project: "fee-ledger".to_owned(),
            title: format!("Chat {id}"),
            title_set: false,
            pinned,
            archived_at: None,
            created_at: moment,
            updated_at: moment,
        }
    }

    #[test]
    fn a_chat_round_trips_through_the_store() {
        let store = Store::open_in_memory().unwrap();
        let record = chat("c1", true, 1_700_000_000);
        store.insert_chat(&record).unwrap();
        assert_eq!(store.get_chat(&record.id).unwrap(), Some(record));
    }

    #[test]
    fn pinned_chats_come_first_then_the_newest() {
        let store = Store::open_in_memory().unwrap();
        store
            .insert_chat(&chat("old", false, 1_700_000_000))
            .unwrap();
        store
            .insert_chat(&chat("new", false, 1_700_000_900))
            .unwrap();
        store
            .insert_chat(&chat("pin", true, 1_600_000_000))
            .unwrap();
        let order: Vec<String> = store
            .list_chats()
            .unwrap()
            .into_iter()
            .map(|c| c.id.to_string())
            .collect();
        assert_eq!(order, ["pin", "new", "old"]);
    }

    #[test]
    fn update_changes_the_title_and_the_archive_time() {
        let store = Store::open_in_memory().unwrap();
        let mut record = chat("c1", false, 1_700_000_000);
        store.insert_chat(&record).unwrap();
        record.title = "Rate limit".to_owned();
        record.title_set = true;
        record.archived_at = Some(OffsetDateTime::from_unix_timestamp(1_700_000_500).unwrap());
        store.update_chat(&record).unwrap();
        assert_eq!(store.get_chat(&record.id).unwrap(), Some(record));
    }

    #[test]
    fn updating_a_missing_chat_is_not_found() {
        let store = Store::open_in_memory().unwrap();
        let error = store.update_chat(&chat("none", false, 1)).unwrap_err();
        assert!(matches!(error, crate::StoreError::NotFound(_)));
    }

    #[test]
    fn delete_removes_the_chat_and_its_events() {
        let store = Store::open_in_memory().unwrap();
        let record = chat("c1", false, 1_700_000_000);
        store.insert_chat(&record).unwrap();
        let at = record.created_at;
        store
            .append_event_in(Some(&record.id), &sentinel_core::Event::ReportReady, at)
            .unwrap();
        store.delete_chat(&record.id).unwrap();
        assert_eq!(store.get_chat(&record.id).unwrap(), None);
        assert_eq!(
            store.chat_events_after(&record.id, 0, 10).unwrap(),
            Vec::new()
        );
    }

    #[test]
    fn the_last_finish_counts_come_from_the_newest_run_finished() {
        let store = Store::open_in_memory().unwrap();
        let record = chat("c1", false, 1_700_000_000);
        store.insert_chat(&record).unwrap();
        let at = record.created_at;
        for (verified, total) in [(1, 3), (3, 3)] {
            let event = sentinel_core::Event::RunFinished { verified, total };
            store.append_event_in(Some(&record.id), &event, at).unwrap();
        }
        assert_eq!(store.last_finish_counts(&record.id).unwrap(), Some((3, 3)));
    }
}
