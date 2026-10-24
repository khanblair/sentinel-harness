//! The Telegram chat row. Sentinel keeps one private chat. The four level switches are four integer
//! columns, and the event list and quiet hours are JSON text that the daemon reads and writes.

use rusqlite::{OptionalExtension, params};
use time::OffsetDateTime;

use crate::{Result, Store, time_from_text, time_text};

/// Notify, Control, Code, Reports, in that order, as the four integer columns hold them.
pub type LevelSwitches = [bool; 4];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelegramChatRecord {
    pub id: String,
    pub telegram_user_id: i64,
    pub levels: LevelSwitches,
    pub detail: String,
    pub events_json: String,
    pub quiet_hours: Option<String>,
    pub connected_at: OffsetDateTime,
    pub revoked_at: Option<OffsetDateTime>,
}

type ChatRow = (
    String,
    i64,
    bool,
    bool,
    bool,
    bool,
    String,
    String,
    Option<String>,
    String,
    Option<String>,
);

impl Store {
    /// Writes the chat row. A row with the same id is replaced, so a chat can reconnect.
    pub fn upsert_telegram_chat(&self, chat: &TelegramChatRecord) -> Result<()> {
        let connection = self.connection()?;
        let [notify, control, code, reports] = chat.levels;
        connection.execute(
            "INSERT INTO telegram_chats
                (id, telegram_user_id, notify, control, code, reports, detail, events_json,
                 quiet_hours, connected_at, revoked_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
                telegram_user_id = excluded.telegram_user_id,
                notify = excluded.notify,
                control = excluded.control,
                code = excluded.code,
                reports = excluded.reports,
                detail = excluded.detail,
                events_json = excluded.events_json,
                quiet_hours = excluded.quiet_hours,
                connected_at = excluded.connected_at,
                revoked_at = excluded.revoked_at",
            params![
                chat.id,
                chat.telegram_user_id,
                notify,
                control,
                code,
                reports,
                chat.detail,
                chat.events_json,
                chat.quiet_hours,
                time_text(chat.connected_at)?,
                chat.revoked_at.map(time_text).transpose()?,
            ],
        )?;
        Ok(())
    }

    /// The one chat that is not revoked, if there is one.
    pub fn get_telegram_chat(&self) -> Result<Option<TelegramChatRecord>> {
        let connection = self.connection()?;
        let row: Option<ChatRow> = connection
            .query_row(
                "SELECT id, telegram_user_id, notify, control, code, reports, detail, events_json,
                        quiet_hours, connected_at, revoked_at
                 FROM telegram_chats
                 WHERE revoked_at IS NULL
                 ORDER BY connected_at DESC
                 LIMIT 1",
                [],
                chat_columns,
            )
            .optional()?;
        row.map(chat_from_row).transpose()
    }

    /// Marks the chat revoked. Returns `false` when no open row has that id.
    pub fn revoke_telegram_chat(&self, id: &str, at: OffsetDateTime) -> Result<bool> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE telegram_chats SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
            params![id, time_text(at)?],
        )?;
        Ok(changed > 0)
    }
}

fn chat_columns(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChatRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
    ))
}

fn chat_from_row(row: ChatRow) -> Result<TelegramChatRecord> {
    let (
        id,
        user_id,
        notify,
        control,
        code,
        reports,
        detail,
        events_json,
        quiet,
        connected,
        revoked,
    ) = row;
    Ok(TelegramChatRecord {
        id,
        telegram_user_id: user_id,
        levels: [notify, control, code, reports],
        detail,
        events_json,
        quiet_hours: quiet,
        connected_at: time_from_text(&connected)?,
        revoked_at: revoked.as_deref().map(time_from_text).transpose()?,
    })
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::TelegramChatRecord;
    use crate::Store;

    fn chat(id: &str) -> TelegramChatRecord {
        TelegramChatRecord {
            id: id.to_owned(),
            telegram_user_id: 42,
            levels: [true, false, true, false],
            detail: "detail".to_owned(),
            events_json: "[\"approval_needed\"]".to_owned(),
            quiet_hours: Some("{\"start_minute\":1320,\"end_minute\":420}".to_owned()),
            connected_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
            revoked_at: None,
        }
    }

    #[test]
    fn an_upserted_chat_reads_back_with_its_levels_and_json() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_telegram_chat(&chat("42")).unwrap();
        assert_eq!(store.get_telegram_chat().unwrap(), Some(chat("42")));
    }

    #[test]
    fn no_chat_is_read_before_one_is_written() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.get_telegram_chat().unwrap(), None);
    }

    #[test]
    fn a_second_upsert_with_the_same_id_replaces_the_levels() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_telegram_chat(&chat("42")).unwrap();
        let mut changed = chat("42");
        changed.levels[1] = true;
        store.upsert_telegram_chat(&changed).unwrap();
        let read = store.get_telegram_chat().unwrap().unwrap();
        assert!(read.levels[1], "the control switch should be on");
    }

    #[test]
    fn revoking_the_chat_hides_it_from_reads() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_telegram_chat(&chat("42")).unwrap();
        let at = OffsetDateTime::from_unix_timestamp(1_700_000_500).unwrap();
        assert!(store.revoke_telegram_chat("42", at).unwrap());
        assert_eq!(store.get_telegram_chat().unwrap(), None);
    }

    #[test]
    fn revoking_an_unknown_chat_changes_nothing() {
        let store = Store::open_in_memory().unwrap();
        let at = OffsetDateTime::from_unix_timestamp(1_700_000_500).unwrap();
        assert!(!store.revoke_telegram_chat("missing", at).unwrap());
    }

    #[test]
    fn upserting_a_revoked_chat_again_reconnects_it() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_telegram_chat(&chat("42")).unwrap();
        let at = OffsetDateTime::from_unix_timestamp(1_700_000_500).unwrap();
        store.revoke_telegram_chat("42", at).unwrap();
        store.upsert_telegram_chat(&chat("42")).unwrap();
        assert_eq!(store.get_telegram_chat().unwrap(), Some(chat("42")));
    }
}
