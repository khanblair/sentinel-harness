//! Event append and read.

use rusqlite::{Row, params};
use sentinel_core::{ChatId, Event, EventRecord};
use time::OffsetDateTime;

use crate::{Result, Store, time_from_text, time_text};

impl Store {
    /// Store an event that belongs to no chat.
    pub fn append_event(&self, event: &Event, at: OffsetDateTime) -> Result<EventRecord> {
        self.append_event_in(None, event, at)
    }

    /// Store an event for a chat (or for none).
    pub fn append_event_in(
        &self,
        chat: Option<&ChatId>,
        event: &Event,
        at: OffsetDateTime,
    ) -> Result<EventRecord> {
        let connection = self.connection()?;
        let payload_json = serde_json::to_string(event)?;
        let task_id = event.task().map(|task| task.as_str().to_owned());
        connection.execute(
            "INSERT INTO events (ts, kind, task_id, payload_json, chat_id) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                time_text(at)?,
                event.kind(),
                task_id,
                payload_json,
                chat.map(ChatId::as_str)
            ],
        )?;
        Ok(EventRecord {
            id: connection.last_insert_rowid(),
            chat: chat.cloned(),
            at,
            event: event.clone(),
        })
    }

    /// Ids ascend. `after_id` is exclusive so a caller can page from the last seen id.
    pub fn events_after(&self, after_id: i64, limit: u32) -> Result<Vec<EventRecord>> {
        self.query_events(
            "SELECT id, ts, payload_json, chat_id FROM events WHERE id > ?1 ORDER BY id LIMIT ?2",
            params![after_id, i64::from(limit)],
        )
    }

    /// The events of one chat after `after_id`, oldest first.
    pub fn chat_events_after(
        &self,
        chat: &ChatId,
        after_id: i64,
        limit: u32,
    ) -> Result<Vec<EventRecord>> {
        self.query_events(
            "SELECT id, ts, payload_json, chat_id FROM events \
             WHERE chat_id = ?1 AND id > ?2 ORDER BY id LIMIT ?3",
            params![chat.as_str(), after_id, i64::from(limit)],
        )
    }

    /// The newest event id, or 0 when there are no events.
    pub fn last_event_id(&self) -> Result<i64> {
        let connection = self.connection()?;
        Ok(
            connection.query_row("SELECT COALESCE(MAX(id), 0) FROM events", [], |row| {
                row.get(0)
            })?,
        )
    }

    /// The newest `limit` events that are not agent output, oldest first. The stream and the
    /// snapshot use this so a chatty agent cannot push real events out of view.
    pub fn recent_stream_events(&self, limit: u32) -> Result<Vec<EventRecord>> {
        let mut records = self.query_events(
            "SELECT id, ts, payload_json, chat_id FROM events WHERE kind != 'AgentOutput' \
             ORDER BY id DESC LIMIT ?1",
            params![i64::from(limit)],
        )?;
        records.reverse();
        Ok(records)
    }

    /// Like `recent_stream_events`, for the events of one chat.
    pub fn recent_chat_stream_events(&self, chat: &ChatId, limit: u32) -> Result<Vec<EventRecord>> {
        let mut records = self.query_events(
            "SELECT id, ts, payload_json, chat_id FROM events \
             WHERE chat_id = ?1 AND kind != 'AgentOutput' ORDER BY id DESC LIMIT ?2",
            params![chat.as_str(), i64::from(limit)],
        )?;
        records.reverse();
        Ok(records)
    }

    fn query_events(&self, sql: &str, args: impl rusqlite::Params) -> Result<Vec<EventRecord>> {
        let connection = self.connection()?;
        let rows = {
            let mut statement = connection.prepare(sql)?;
            let mapped = statement.query_map(args, read_event_row)?;
            mapped.collect::<rusqlite::Result<Vec<EventRow>>>()?
        };
        rows.into_iter().map(event_from_row).collect()
    }
}

type EventRow = (i64, String, String, Option<String>);

fn read_event_row(row: &Row<'_>) -> rusqlite::Result<EventRow> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
}

fn event_from_row(row: EventRow) -> Result<EventRecord> {
    let (id, ts, payload_json, chat) = row;
    Ok(EventRecord {
        id,
        chat: chat.map(ChatId::new),
        at: time_from_text(&ts)?,
        event: serde_json::from_str(&payload_json)?,
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::params;
    use sentinel_core::{TaskId, TaskStatus};

    use super::*;

    #[test]
    fn stored_kind_and_task_id_match_the_event() {
        let store = Store::open_in_memory().unwrap();
        let event = Event::TaskStatus {
            task: TaskId::new("T-01"),
            status: TaskStatus::Ready,
        };
        let at = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let record = store.append_event(&event, at).unwrap();

        let connection = store.connection().unwrap();
        let (kind, task_id): (String, Option<String>) = connection
            .query_row(
                "SELECT kind, task_id FROM events WHERE id = ?1",
                params![record.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(kind, "TaskStatus");
        assert_eq!(task_id.as_deref(), Some("T-01"));
    }

    #[test]
    fn an_event_without_a_task_stores_a_null_task_id() {
        let store = Store::open_in_memory().unwrap();
        let at = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let record = store.append_event(&Event::HostHeartbeat, at).unwrap();

        let connection = store.connection().unwrap();
        let (kind, task_id): (String, Option<String>) = connection
            .query_row(
                "SELECT kind, task_id FROM events WHERE id = ?1",
                params![record.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(kind, "HostHeartbeat");
        assert_eq!(task_id, None);
    }
}
