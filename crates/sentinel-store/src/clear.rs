//! Clear the rows of one data scope. Each function runs in one transaction, so a failure leaves the
//! tables as they were. The engine decides which scope to clear and checks for a live run first.

use rusqlite::{Connection, params};

use crate::{Result, Store};

/// The tables that hold one run's history. `runs` holds the cost rows.
const HISTORY_TABLES: [&str; 10] = [
    "quality_notes",
    "verifications",
    "checkpoints",
    "barriers",
    "approvals",
    "decisions",
    "reservations",
    "task_deps",
    "tasks",
    "runs",
];

impl Store {
    /// Remove the tasks, reservations, checkpoints, approvals, decisions, and cost rows of every
    /// run. Chats and their events stay.
    pub fn clear_history(&self) -> Result<()> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        for table in HISTORY_TABLES {
            transaction.execute(&format!("DELETE FROM {table}"), [])?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Remove the Telegram chat rows and the outbox, then the paired flag and the bot name. The
    /// keys are the ones the daemon's Telegram module writes.
    pub fn clear_telegram(&self) -> Result<()> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute("DELETE FROM telegram_outbox", [])?;
        transaction.execute("DELETE FROM telegram_chats", [])?;
        delete_setting_rows(&transaction, &["telegram.paired", "telegram.bot_username"])?;
        transaction.commit()?;
        Ok(())
    }

    /// Remove every routine and every brief they built.
    pub fn clear_routines(&self) -> Result<()> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute("DELETE FROM briefs", [])?;
        transaction.execute("DELETE FROM routines", [])?;
        transaction.commit()?;
        Ok(())
    }

    /// Remove the settings rows with these keys. A key that is not stored is not an error.
    pub fn delete_settings(&self, keys: &[&str]) -> Result<()> {
        let connection = self.connection()?;
        delete_setting_rows(&connection, keys)
    }
}

fn delete_setting_rows(connection: &Connection, keys: &[&str]) -> Result<()> {
    for key in keys {
        connection.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use sentinel_core::ChatId;
    use time::OffsetDateTime;

    use crate::Store;
    use crate::chats::ChatRecord;
    use crate::telegram::TelegramChatRecord;

    fn count(store: &Store, table: &str) -> i64 {
        store
            .connection()
            .unwrap()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    fn run_sql(store: &Store, sql: &str) {
        store.connection().unwrap().execute_batch(sql).unwrap();
    }

    fn a_chat(store: &Store, id: &str) {
        let moment = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store
            .insert_chat(&ChatRecord {
                id: ChatId::new(id),
                project: "repo".to_owned(),
                title: "Rate limit".to_owned(),
                title_set: true,
                pinned: false,
                archived_at: None,
                created_at: moment,
                updated_at: moment,
                agent: None,
            })
            .unwrap();
    }

    fn a_telegram_chat(store: &Store) {
        let moment = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store
            .upsert_telegram_chat(&TelegramChatRecord {
                id: "chat-1".to_owned(),
                telegram_user_id: 42,
                levels: [true, false, false, false],
                detail: "summary".to_owned(),
                events_json: "[]".to_owned(),
                quiet_hours: None,
                connected_at: moment,
                revoked_at: None,
            })
            .unwrap();
    }

    #[test]
    fn clear_history_removes_the_task_and_cost_rows() {
        let store = Store::open_in_memory().unwrap();
        run_sql(
            &store,
            "INSERT INTO tasks (id, title, body, status, tier, scope_json, flags)
               VALUES ('t1', 'Task', 'Body', 'verified', 'mid', '[]', '[]');
             INSERT INTO runs (id, task_id, tool_id, model, started_at, cost_micro_usd, cost_estimated)
               VALUES ('r1', 't1', 'claude', 'm', '2023-11-14T22:13:20Z', 5, 0);",
        );
        store.clear_history().unwrap();
        assert_eq!(count(&store, "tasks"), 0);
        assert_eq!(count(&store, "runs"), 0);
    }

    #[test]
    fn clear_history_keeps_the_chats_and_their_rows() {
        let store = Store::open_in_memory().unwrap();
        a_chat(&store, "chat-1");
        store.clear_history().unwrap();
        assert_eq!(store.list_chats().unwrap().len(), 1);
    }

    #[test]
    fn clear_telegram_removes_the_chat_outbox_and_paired_flag() {
        let store = Store::open_in_memory().unwrap();
        a_telegram_chat(&store);
        store.set_setting("telegram.paired", "true").unwrap();
        store.set_setting("unrelated", "keep").unwrap();
        run_sql(
            &store,
            "INSERT INTO telegram_outbox (chat_id, kind, payload_hash, state, created_at)
               VALUES ('chat-1', 'brief', 'h', 'sent', '2023-11-14T22:13:20Z');",
        );
        store.clear_telegram().unwrap();
        assert!(store.get_telegram_chat().unwrap().is_none());
        assert_eq!(count(&store, "telegram_outbox"), 0);
        assert_eq!(store.get_setting("telegram.paired").unwrap(), None);
        assert_eq!(
            store.get_setting("unrelated").unwrap().as_deref(),
            Some("keep")
        );
    }

    #[test]
    fn clear_routines_removes_routines_and_briefs() {
        let store = Store::open_in_memory().unwrap();
        run_sql(
            &store,
            "INSERT INTO routines (id, name, kind, minute_of_day, days, created_at)
               VALUES ('r1', 'Standup', 'reminder', 540, 31, '2023-11-14T22:13:20Z');
             INSERT INTO briefs (routine_id, kind, title, body, created_at)
               VALUES ('r1', 'morning', 'Title', 'Body', '2023-11-14T22:13:20Z');",
        );
        store.clear_routines().unwrap();
        assert_eq!(count(&store, "routines"), 0);
        assert_eq!(count(&store, "briefs"), 0);
    }

    #[test]
    fn delete_settings_removes_only_the_named_keys() {
        let store = Store::open_in_memory().unwrap();
        store.set_setting("tools.defaults_applied", "1").unwrap();
        store.set_setting("onboarding.step", "3").unwrap();
        store
            .delete_settings(&["tools.defaults_applied", "missing"])
            .unwrap();
        assert_eq!(store.get_setting("tools.defaults_applied").unwrap(), None);
        assert_eq!(
            store.get_setting("onboarding.step").unwrap().as_deref(),
            Some("3")
        );
    }
}
