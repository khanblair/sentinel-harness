//! What a run starts from. A message in a chat can bring plain words, a backlog file, and
//! attachments. This module checks them before anything is stored, and turns them into the
//! things the run loop needs: the chat, the backlog (or the words), and the context text.

use sentinel_core::TaskId;
use sentinel_core::api::RunRequest;
use sentinel_core::chat::{NEW_CHAT_TITLE, title_from_message};
use sentinel_core::{Attachment, AttachmentKind, ChatId, Event};
use sentinel_plan::backlog::BacklogItem;

use crate::attach::{clean_relative, context_text};
use crate::controls::Caller;
use crate::engine::{Engine, EngineError, Result};

/// The longest task title made from plain words.
const TASK_TITLE_CHARS: usize = 80;

/// Everything a run needs to know about where it came from.
pub(crate) struct RunSource {
    pub chat: ChatId,
    /// A backlog file of the repository (or an uploaded one), when the run has one.
    pub backlog: Option<String>,
    /// Plain words that become one task when there is no backlog.
    pub words: Option<String>,
    /// Paths, small files, folder listings, and notes for every task.
    pub context: String,
    pub attachments: Vec<Attachment>,
    /// What the user said, for the stream.
    pub message: String,
}

impl RunSource {
    /// A source for a run that picks up where a stopped run left off. It has no new words.
    pub(crate) fn resumed(chat: ChatId) -> Self {
        Self {
            chat,
            backlog: None,
            words: None,
            context: String::new(),
            attachments: Vec::new(),
            message: String::new(),
        }
    }
}

impl Engine {
    /// Check a run request and work out its source. Nothing is stored here, so a refusal leaves
    /// no trace in the chat.
    pub(crate) fn resolve_source(&self, request: &RunRequest) -> Result<RunSource> {
        let chat = self.chat_for_run(request)?;
        let attachments = self.load_attachments(&chat, request)?;
        let backlog = request.backlog.clone().or_else(|| backlog_of(&attachments));
        let words = request
            .text
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty() && backlog.is_none())
            .map(str::to_owned);
        let context = context_text(self.repo_root(), &attachments);
        let message = self.message_for(request, backlog.as_deref());
        Ok(RunSource {
            chat,
            backlog,
            words,
            context,
            attachments,
            message,
        })
    }

    /// The chat of the request, or a new chat when the request names none (the command line).
    fn chat_for_run(&self, request: &RunRequest) -> Result<ChatId> {
        let Some(id) = &request.chat else {
            let title = request
                .backlog
                .as_deref()
                .or(request.text.as_deref())
                .map(title_from_message);
            let created = self.create_chat(title.as_deref(), &Caller::pc("You"))?;
            return Ok(created.id);
        };
        let record = self.chat_record(id)?;
        if record.archived_at.is_some() {
            return Err(EngineError::Refused(
                "This chat is archived. Unarchive it first.".to_owned(),
            ));
        }
        self.refuse_if_another_chat_is_live(id)?;
        Ok(id.clone())
    }

    /// One run is live per project, because there is one working tree.
    pub(crate) fn refuse_if_another_chat_is_live(&self, id: &ChatId) -> Result<()> {
        let Some(live) = self.live_chat().filter(|live| live != id) else {
            return Ok(());
        };
        let title = self
            .chat_record(&live)
            .map_or_else(|_| "another chat".to_owned(), |c| c.title);
        Err(EngineError::Refused(format!(
            "Another chat is working in this project: \"{title}\". Open it or stop it."
        )))
    }

    fn load_attachments(&self, chat: &ChatId, request: &RunRequest) -> Result<Vec<Attachment>> {
        let mut found = Vec::new();
        for id in &request.attachments {
            let item = self
                .store()
                .get_attachment(id)?
                .filter(|a| &a.chat == chat && !a.sent)
                .ok_or_else(|| EngineError::NotFound(format!("attachment {id}")))?;
            found.push(item);
        }
        Ok(found)
    }

    fn message_for(&self, request: &RunRequest, backlog: Option<&str>) -> String {
        if let Some(text) = request
            .text
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            return text.to_owned();
        }
        let name = backlog.map_or_else(|| self.config().backlog, str::to_owned);
        let short = name.rsplit('/').next().unwrap_or(&name).to_owned();
        format!("Use {short}")
    }

    /// Put the user's words and attachments in the stream. This runs once the run is registered,
    /// so the events belong to the chat.
    pub(crate) fn announce_request(&self, source: &RunSource, caller: &Caller) -> Result<()> {
        self.emit(&Event::Message {
            from: caller.name.clone(),
            to: None,
            text: source.message.clone(),
        })?;
        if !source.attachments.is_empty() {
            self.emit(&Event::AttachmentsSent {
                attachments: source
                    .attachments
                    .iter()
                    .map(Attachment::reference)
                    .collect(),
            })?;
        }
        let ids: Vec<_> = source.attachments.iter().map(|a| a.id.clone()).collect();
        self.store().mark_attachments_sent(&ids)?;
        self.name_chat_from(source)?;
        self.store().touch_chat(&source.chat, self.now())?;
        Ok(())
    }

    /// A chat nobody named takes the first line of its first message.
    fn name_chat_from(&self, source: &RunSource) -> Result<()> {
        let mut record = self.chat_record(&source.chat)?;
        if record.title_set || record.title != NEW_CHAT_TITLE {
            return Ok(());
        }
        record.title = title_from_message(&source.message);
        record.updated_at = self.now();
        self.store().update_chat(&record)?;
        self.emit(&Event::ChatChanged {
            chat: source.chat.clone(),
            change: sentinel_core::ChatChange::Renamed {
                title: record.title,
            },
        })?;
        Ok(())
    }
}

/// The backlog an attachment names, if there is one.
fn backlog_of(attachments: &[Attachment]) -> Option<String> {
    attachments
        .iter()
        .find(|a| a.kind == AttachmentKind::Backlog)
        .and_then(|a| a.path.clone())
}

/// One task from plain words: the first line is the title, the whole message is the body.
pub(crate) fn words_item(words: &str) -> BacklogItem {
    let title: String = title_from_message(words)
        .chars()
        .take(TASK_TITLE_CHARS)
        .collect();
    BacklogItem {
        id: TaskId::new("T-01"),
        title,
        body: words.trim().to_owned(),
        claims_done: false,
    }
}

/// Add the attached context to the body of every item.
pub(crate) fn with_context(mut items: Vec<BacklogItem>, context: &str) -> Vec<BacklogItem> {
    if context.is_empty() {
        return items;
    }
    for item in &mut items {
        item.body = format!("{}\n\n{context}", item.body.trim_end());
    }
    items
}

/// A path that is safe to read as a backlog. A chat upload keeps its stored path.
pub(crate) fn backlog_path(name: &str) -> Result<String> {
    if name.starts_with(".sentinel/chats/") {
        return Ok(name.to_owned());
    }
    if std::path::Path::new(name).is_absolute() {
        return Ok(name.to_owned());
    }
    clean_relative(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_words_become_one_task_titled_by_the_first_line() {
        let item = words_item("Add a rate limit\nRefuse 30 transfers a minute.");
        assert_eq!(item.id.as_str(), "T-01");
        assert_eq!(item.title, "Add a rate limit");
        assert!(item.body.contains("Refuse 30"));
    }

    #[test]
    fn context_is_added_to_every_body() {
        let items = vec![words_item("One"), words_item("Two")];
        let with = with_context(items, "Attached by the user:\n- Note: x");
        assert!(with.iter().all(|i| i.body.ends_with("- Note: x")));
    }

    #[test]
    fn no_context_leaves_the_items_alone() {
        let items = vec![words_item("One")];
        assert_eq!(with_context(items.clone(), "")[0].body, items[0].body);
    }

    #[test]
    fn an_upload_path_is_kept_and_a_parent_path_is_refused() {
        assert_eq!(
            backlog_path(".sentinel/chats/c/attachments/a-b.md").unwrap(),
            ".sentinel/chats/c/attachments/a-b.md"
        );
        assert!(backlog_path("../x.md").is_err());
        assert_eq!(backlog_path("docs/roadmap.md").unwrap(), "docs/roadmap.md");
    }
}
