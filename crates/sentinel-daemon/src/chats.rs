//! Chats: create, list, rename, pin, archive, delete. A chat is one conversation in the project.
//! The store keeps the rows, the events carry the chat id, and this module adds what only the
//! engine knows: which chat owns the live run, and what a chat's icon should show.

use std::fs;

use sentinel_core::api::UpdateChat;
use sentinel_core::chat::NEW_CHAT_TITLE;
use sentinel_core::{ChatChange, ChatId, ChatInfo, ChatRunState, Event, EventRecord, HarnessState};
use sentinel_store::ChatRecord;

use crate::controls::Caller;
use crate::engine::{Engine, EngineError, Result, lock};

/// The longest title a user may give a chat.
const MAX_TITLE_CHARS: usize = 120;

/// Events read per page when a chat is checked for anything but its own changes.
const EVENT_PAGE: u32 = 100;

impl Engine {
    /// The name of the project: the repository folder.
    pub(crate) fn project_name(&self) -> String {
        self.repo_path().file_name().map_or_else(
            || "project".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        )
    }

    /// The chat that owns the live run, when a run is live.
    pub(crate) fn live_chat(&self) -> Option<ChatId> {
        lock(&self.inner.live)
            .run
            .as_ref()
            .map(|run| run.chat.clone())
    }

    /// Every chat of the project with the state its sidebar row shows. Pinned chats come first,
    /// archived ones last.
    pub fn list_chats(&self) -> Result<Vec<ChatInfo>> {
        let live = self.live_chat();
        let mut chats = self
            .store()
            .list_chats()?
            .into_iter()
            .map(|record| self.chat_info(record, live.as_ref()))
            .collect::<Result<Vec<_>>>()?;
        chats.sort_by_key(|chat| chat.archived);
        Ok(chats)
    }

    pub fn get_chat(&self, id: &ChatId) -> Result<ChatInfo> {
        let record = self.chat_record(id)?;
        self.chat_info(record, self.live_chat().as_ref())
    }

    pub(crate) fn chat_record(&self, id: &ChatId) -> Result<ChatRecord> {
        self.store()
            .get_chat(id)?
            .ok_or_else(|| EngineError::NotFound(format!("chat {id}")))
    }

    fn chat_info(&self, record: ChatRecord, live: Option<&ChatId>) -> Result<ChatInfo> {
        let is_live = live == Some(&record.id);
        let (run, verified, total) = if is_live {
            self.live_chat_state()?
        } else {
            self.finished_chat_state(&record.id)?
        };
        let needs_you = is_live && self.has_open_cards()?;
        Ok(ChatInfo {
            id: record.id,
            project: record.project,
            title: record.title,
            pinned: record.pinned,
            archived: record.archived_at.is_some(),
            created_at: record.created_at,
            updated_at: record.updated_at,
            run,
            verified,
            total,
            needs_you,
        })
    }

    /// The icon state and the counts of the chat that owns the live run.
    fn live_chat_state(&self) -> Result<(ChatRunState, u32, u32)> {
        let tasks = self.store().list_tasks()?;
        let verified = tasks
            .iter()
            .filter(|t| t.status == sentinel_core::TaskStatus::Verified)
            .count();
        let run = match self.harness_state() {
            HarnessState::Paused => ChatRunState::Paused,
            HarnessState::Stopping => ChatRunState::Stopped,
            HarnessState::Idle
            | HarnessState::Onboarding
            | HarnessState::Ready
            | HarnessState::Running
            | HarnessState::Done
            | HarnessState::Crashed
            | HarnessState::Resuming => ChatRunState::Running,
        };
        Ok((
            run,
            u32::try_from(verified).unwrap_or(0),
            u32::try_from(tasks.len()).unwrap_or(0),
        ))
    }

    /// The icon state and the counts of a chat whose runs are over.
    fn finished_chat_state(&self, id: &ChatId) -> Result<(ChatRunState, u32, u32)> {
        match self.store().last_finish_counts(id)? {
            Some((verified, total)) if verified == total => {
                Ok((ChatRunState::Done, verified, total))
            }
            Some((verified, total)) => Ok((ChatRunState::Blocked, verified, total)),
            None => Ok((ChatRunState::Idle, 0, 0)),
        }
    }

    fn has_open_cards(&self) -> Result<bool> {
        let approvals = self
            .store()
            .list_approvals(&[sentinel_core::ApprovalState::Pending])?;
        let decisions = self
            .store()
            .list_decisions(&[sentinel_core::DecisionState::Pending])?;
        Ok(!approvals.is_empty() || !decisions.is_empty())
    }

    /// Make an empty chat in this project. A request with no title reuses the newest empty chat
    /// instead, so the New chat button never adds a second blank row.
    pub fn create_chat(&self, title: Option<&str>, caller: &Caller) -> Result<ChatInfo> {
        let named = title.map(str::trim).filter(|t| !t.is_empty());
        if named.is_none()
            && let Some(empty) = self.empty_chats()?.pop()
        {
            return self.get_chat(&empty.id);
        }
        let now = self.now();
        let record = ChatRecord {
            id: ChatId::generate(),
            project: self.project_name(),
            title: clean_title(named.unwrap_or(NEW_CHAT_TITLE))?,
            title_set: named.is_some(),
            pinned: false,
            archived_at: None,
            created_at: now,
            updated_at: now,
        };
        self.store().insert_chat(&record)?;
        self.emit_chat_change(&record.id, ChatChange::Created)?;
        self.note_device_action(caller, &format!("opened chat {}", record.title));
        self.get_chat(&record.id)
    }

    /// Rename, pin, or archive a chat. Only the fields that are present change.
    pub fn update_chat(
        &self,
        id: &ChatId,
        update: &UpdateChat,
        caller: &Caller,
    ) -> Result<ChatInfo> {
        let mut record = self.chat_record(id)?;
        if let Some(title) = &update.title {
            record.title = clean_title(title)?;
            record.title_set = true;
        }
        if let Some(pinned) = update.pinned {
            record.pinned = pinned;
        }
        if let Some(archived) = update.archived {
            self.check_not_live(id, "archived")?;
            record.archived_at = archived.then(|| self.now());
        }
        record.updated_at = self.now();
        self.store().update_chat(&record)?;
        for change in changes_of(update, &record) {
            self.emit_chat_change(id, change)?;
        }
        self.note_device_action(caller, &format!("changed chat {}", record.title));
        self.get_chat(id)
    }

    /// Delete a chat, its events, and its uploads. The repository is not touched.
    pub fn delete_chat(&self, id: &ChatId, caller: &Caller) -> Result<()> {
        let record = self.chat_record(id)?;
        self.check_not_live(id, "deleted")?;
        self.emit_chat_change(id, ChatChange::Deleted)?;
        self.store().delete_chat(id)?;
        self.remove_uploads(id);
        self.note_device_action(caller, &format!("deleted chat {}", record.title));
        self.ensure_a_chat()
    }

    /// Remove the upload folder of a chat. A missing folder only means the chat never had one.
    fn remove_uploads(&self, id: &ChatId) {
        let uploads = self.repo_root().join(".sentinel/chats").join(id.as_str());
        let _ = fs::remove_dir_all(uploads);
    }

    /// The chats with nothing in them, oldest first.
    fn empty_chats(&self) -> Result<Vec<ChatRecord>> {
        let mut empties = Vec::new();
        for record in self.store().list_chats()? {
            if self.is_empty_chat(&record)? {
                empties.push(record);
            }
        }
        empties.sort_by_key(|record| record.created_at);
        Ok(empties)
    }

    /// A chat is empty when nobody named or pinned it, nothing is attached, it is not archived, and
    /// its only events are its own `ChatChanged` events. A pin is a choice, so it keeps the chat.
    fn is_empty_chat(&self, record: &ChatRecord) -> Result<bool> {
        let named = record.title_set || record.title != NEW_CHAT_TITLE;
        if named || record.pinned || record.archived_at.is_some() {
            return Ok(false);
        }
        if !self.store().list_attachments(&record.id, false)?.is_empty() {
            return Ok(false);
        }
        self.has_only_chat_changes(&record.id)
    }

    /// True when the chat has no stored event other than `ChatChanged`. Reads a page at a time and
    /// stops at the first other event.
    fn has_only_chat_changes(&self, id: &ChatId) -> Result<bool> {
        let mut after = 0;
        loop {
            let page = self.store().chat_events_after(id, after, EVENT_PAGE)?;
            if page
                .iter()
                .any(|record| !matches!(record.event, Event::ChatChanged { .. }))
            {
                return Ok(false);
            }
            match page.last() {
                Some(last) => after = last.id,
                None => return Ok(true),
            }
        }
    }

    /// Delete every empty chat except the newest one. Runs once when the engine opens.
    pub(crate) fn prune_empty_chats(&self) -> Result<()> {
        let mut empties = self.empty_chats()?;
        empties.pop();
        for record in empties {
            self.store().delete_chat(&record.id)?;
            self.remove_uploads(&record.id);
        }
        Ok(())
    }

    /// The stored events of one chat, oldest first.
    pub fn chat_events(&self, id: &ChatId, after: i64, limit: u32) -> Result<Vec<EventRecord>> {
        self.chat_record(id)?;
        Ok(self.store().chat_events_after(id, after, limit)?)
    }

    /// A chat with a live run cannot be archived or deleted until the run stops.
    fn check_not_live(&self, id: &ChatId, what: &str) -> Result<()> {
        if self.is_run_live() && self.live_chat().as_ref() == Some(id) {
            return Err(EngineError::Refused(format!(
                "This chat has a live run. Stop the run before it is {what}."
            )));
        }
        Ok(())
    }

    /// The project always has at least one chat, so the UI always has one to open.
    pub(crate) fn ensure_a_chat(&self) -> Result<()> {
        if self.store().list_chats()?.is_empty() {
            self.create_chat(None, &Caller::pc("Sentinel"))?;
        }
        Ok(())
    }

    /// A store from before chats has events with no chat. They move into one chat.
    pub(crate) fn adopt_old_events(&self) -> Result<()> {
        if !self.store().list_chats()?.is_empty() || self.store().last_event_id()? == 0 {
            return Ok(());
        }
        let now = self.now();
        let record = ChatRecord {
            id: ChatId::generate(),
            project: self.project_name(),
            title: "Earlier runs".to_owned(),
            title_set: true,
            pinned: false,
            archived_at: None,
            created_at: now,
            updated_at: now,
        };
        self.store().insert_chat(&record)?;
        self.store().adopt_chatless_events(&record.id)?;
        Ok(())
    }

    fn emit_chat_change(&self, chat: &ChatId, change: ChatChange) -> Result<()> {
        // A deleted chat takes its events with it, so its own deletion is kept as an event of no chat.
        let owner = (!matches!(change, ChatChange::Deleted)).then_some(chat);
        let event = Event::ChatChanged {
            chat: chat.clone(),
            change,
        };
        self.emit_in(owner, &event)?;
        Ok(())
    }

    fn note_device_action(&self, caller: &Caller, what: &str) {
        self.emit_quiet(&Event::DeviceAction {
            device: sentinel_core::DeviceId::new(&caller.name),
            action: what.to_owned(),
        });
    }
}

/// The trimmed title, or why it is not allowed.
fn clean_title(text: &str) -> Result<String> {
    let title = text.trim();
    if title.is_empty() {
        return Err(EngineError::Invalid("A chat needs a name.".to_owned()));
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(EngineError::Invalid(format!(
            "A chat name has at most {MAX_TITLE_CHARS} characters."
        )));
    }
    Ok(title.to_owned())
}

/// One event per field that the update carried.
fn changes_of(update: &UpdateChat, record: &ChatRecord) -> Vec<ChatChange> {
    let mut changes = Vec::new();
    if update.title.is_some() {
        changes.push(ChatChange::Renamed {
            title: record.title.clone(),
        });
    }
    if let Some(pinned) = update.pinned {
        changes.push(ChatChange::Pinned { pinned });
    }
    if let Some(archived) = update.archived {
        changes.push(ChatChange::Archived { archived });
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::SystemRunner;
    use crate::engine::EngineOptions;
    use crate::secrets::MemoryStore;
    use std::sync::Arc;
    use time::OffsetDateTime;

    #[test]
    fn an_empty_title_is_refused() {
        assert!(clean_title("   ").is_err());
    }

    #[test]
    fn a_title_is_trimmed() {
        assert_eq!(clean_title("  Rate limit ").unwrap(), "Rate limit");
    }

    #[test]
    fn a_very_long_title_is_refused() {
        assert!(clean_title(&"a".repeat(MAX_TITLE_CHARS + 1)).is_err());
    }

    fn open_in_memory(repo: &std::path::Path) -> Engine {
        Engine::open(EngineOptions {
            repo_root: repo.to_path_buf(),
            home: repo.to_path_buf(),
            runner: Arc::new(SystemRunner),
            secrets: Arc::new(MemoryStore::new()),
            fake_agents: None,
            in_memory: true,
        })
        .unwrap()
    }

    /// A chat row with no events, as an older build could leave behind.
    fn insert_blank_chat(engine: &Engine, id: &str, unix_seconds: i64) {
        let moment = OffsetDateTime::from_unix_timestamp(unix_seconds).unwrap();
        let record = ChatRecord {
            id: ChatId::new(id),
            project: "repo".to_owned(),
            title: NEW_CHAT_TITLE.to_owned(),
            title_set: false,
            pinned: false,
            archived_at: None,
            created_at: moment,
            updated_at: moment,
        };
        engine.store().insert_chat(&record).unwrap();
    }

    #[test]
    fn prune_keeps_the_newest_empty_chat_and_every_named_chat() {
        let dir = tempfile::tempdir().unwrap();
        let engine = open_in_memory(dir.path());
        let newest_blank = engine.list_chats().unwrap().remove(0).id;
        insert_blank_chat(&engine, "older-blank", 1_000);
        insert_blank_chat(&engine, "middle-blank", 2_000);
        let named = engine
            .create_chat(Some("Rate limit"), &Caller::pc("You"))
            .unwrap();

        engine.prune_empty_chats().unwrap();

        let mut kept: Vec<String> = engine
            .list_chats()
            .unwrap()
            .into_iter()
            .map(|chat| chat.id.as_str().to_owned())
            .collect();
        kept.sort();
        let mut expected = vec![
            newest_blank.as_str().to_owned(),
            named.id.as_str().to_owned(),
        ];
        expected.sort();
        assert_eq!(kept, expected);
    }
}
