//! The Context panel's numbers: what the next task would put in the window of the agent routed for
//! the `mid` tier. Each part is built from the text Sentinel really sends, so the panel shows what
//! a run loads. Every count is an estimate: one token is about four characters.

use std::fs;
use std::slice;

use sentinel_brain::mode::may_read;
use sentinel_brain::select::{SelectRequest, select_notes};
use sentinel_core::context::{ContextEntry, ContextItem, ContextUsage};
use sentinel_core::skills::SkillMode;
use sentinel_core::{Attachment, AttachmentKind, ChatId, Event, EventRecord, Task, TaskId};
use sentinel_workers::context::ContextLimits;

use crate::attach::context_text;
use crate::engine::{Engine, Result, lock};
use crate::prompt::task_prompt;

const CHARS_PER_TOKEN: usize = 4;
/// The window of an adapter that the table does not name.
const DEFAULT_WINDOW: u32 = 128_000;

/// The window in tokens of an agent, by adapter id. The CLI may use a bigger window than these;
/// the panel shows the size Sentinel plans around.
pub(crate) fn window_for_adapter(adapter: &str) -> u32 {
    match adapter {
        "claude" | "codex" => 200_000,
        "gemini" => 1_000_000,
        _ => DEFAULT_WINDOW,
    }
}

/// The estimate for a text: one token per four characters, rounded up.
pub fn estimate_tokens(text: &str) -> u32 {
    u32::try_from(text.chars().count().div_ceil(CHARS_PER_TOKEN)).unwrap_or(u32::MAX)
}

/// The agent whose window the panel measures.
struct MidAgent {
    name: String,
    window: u32,
}

impl MidAgent {
    fn none() -> Self {
        Self {
            name: "none".to_owned(),
            window: DEFAULT_WINDOW,
        }
    }
}

/// The parts of the window, collected before the sums are taken.
#[derive(Default)]
struct Sections {
    items: Vec<ContextItem>,
    notes: Vec<ContextEntry>,
    skills: Vec<ContextEntry>,
    attachments: Vec<ContextEntry>,
}

impl Sections {
    fn item(&mut self, kind: &str, label: &str, tokens: u32) {
        self.items.push(ContextItem {
            kind: kind.to_owned(),
            label: label.to_owned(),
            tokens,
        });
    }

    /// The free space is what the items leave of the window, never less than zero.
    fn into_usage(mut self, agent: MidAgent) -> ContextUsage {
        let used = self
            .items
            .iter()
            .fold(0_u32, |sum, item| sum.saturating_add(item.tokens));
        let free = agent.window.saturating_sub(used);
        self.item("free", "Free space", free);
        ContextUsage {
            agent: agent.name,
            window_tokens: agent.window,
            used_tokens: used,
            items: self.items,
            notes: self.notes,
            skills: self.skills,
            attachments: self.attachments,
        }
    }
}

fn total(entries: &[ContextEntry]) -> u32 {
    entries
        .iter()
        .fold(0_u32, |sum, entry| sum.saturating_add(entry.tokens))
}

/// The text of a chat message, or nothing for any other event.
fn message_text(event: &Event) -> Option<&str> {
    let Event::Message { text, .. } = event else {
        return None;
    };
    Some(text)
}

/// The newest message a person sent. Sentinel's own lines carry the sender `sentinel`.
fn last_user_text(records: &[EventRecord]) -> String {
    records
        .iter()
        .rev()
        .find_map(|record| {
            let Event::Message { from, text, .. } = &record.event else {
                return None;
            };
            (from != "sentinel").then(|| text.clone())
        })
        .unwrap_or_default()
}

impl Engine {
    /// What the next task would put in the window of the `mid` agent. The chat decides the
    /// messages and the attachments that count.
    pub fn context_usage(&self, chat: Option<&ChatId>) -> Result<ContextUsage> {
        let agent = self.mid_agent()?;
        let chat = self.context_chat(chat)?;
        let records = self.chat_records(chat.as_ref())?;
        let pending = self.chat_pending(chat.as_ref())?;
        let mut sections = Sections::default();
        sections.item("messages", "Messages", messages_tokens(&records));
        self.add_instructions(&mut sections, &pending);
        self.add_brain(&mut sections, &last_user_text(&records))?;
        self.add_skills(&mut sections)?;
        self.add_attachments(&mut sections, &pending);
        self.add_backlog(&mut sections);
        Ok(sections.into_usage(agent))
    }

    /// The agent routed for the `mid` tier. Without one, the window is the default.
    fn mid_agent(&self) -> Result<MidAgent> {
        let Some(routed) = self.config().routing.mid else {
            return Ok(MidAgent::none());
        };
        let tool = self
            .store()
            .list_tools()?
            .into_iter()
            .find(|tool| tool.id == routed);
        Ok(tool.map_or_else(MidAgent::none, |tool| MidAgent {
            name: tool.name,
            window: window_for_adapter(&tool.adapter),
        }))
    }

    /// The chat asked for, else the live run's chat, else the first chat that is not archived.
    fn context_chat(&self, asked: Option<&ChatId>) -> Result<Option<ChatId>> {
        if let Some(id) = asked {
            self.chat_record(id)?;
            return Ok(Some(id.clone()));
        }
        if let Some(live) = self.live_chat() {
            return Ok(Some(live));
        }
        let first = self
            .store()
            .list_chats()?
            .into_iter()
            .find(|chat| chat.archived_at.is_none());
        Ok(first.map(|chat| chat.id))
    }

    fn chat_records(&self, chat: Option<&ChatId>) -> Result<Vec<EventRecord>> {
        match chat {
            Some(id) => Ok(self.store().chat_events_after(id, 0, u32::MAX)?),
            None => Ok(Vec::new()),
        }
    }

    fn chat_pending(&self, chat: Option<&ChatId>) -> Result<Vec<Attachment>> {
        match chat {
            Some(id) => self.pending_attachments(id),
            None => Ok(Vec::new()),
        }
    }

    /// The fixed prompt text every task gets, and the notes the owner added to the chat.
    fn add_instructions(&self, sections: &mut Sections, pending: &[Attachment]) {
        let empty = Task::new(TaskId::new(""), "", "");
        let prompt = task_prompt(&empty, None, "", &ContextLimits::default());
        let notes: Vec<Attachment> = pending
            .iter()
            .filter(|attachment| attachment.kind == AttachmentKind::Note)
            .cloned()
            .collect();
        let notes_text = context_text(&self.repo_path(), &notes);
        let tokens = estimate_tokens(&prompt).saturating_add(estimate_tokens(&notes_text));
        sections.item("instructions", "Instructions", tokens);
    }

    /// The notes a run would load now. A Brain that is off loads none.
    fn add_brain(&self, sections: &mut Sections, query: &str) -> Result<()> {
        let notes = if may_read(self.config().brain_mode) {
            self.context_brain_notes(query)?
        } else {
            Vec::new()
        };
        sections.item("brain", "Brain notes", total(&notes));
        sections.notes = notes;
        Ok(())
    }

    fn context_brain_notes(&self, query: &str) -> Result<Vec<ContextEntry>> {
        let state = self.brain()?;
        let request = SelectRequest {
            query: query.to_owned(),
            ..SelectRequest::default()
        };
        let selection = {
            let index = lock(&state.index);
            select_notes(&index, &state.vault, &request)?
        };
        Ok(selection
            .notes
            .iter()
            .map(|selected| {
                let body = state.vault.read(&selected.id).ok().flatten();
                let tokens = body.map_or(0, |note| {
                    estimate_tokens(&note.title).saturating_add(estimate_tokens(&note.body))
                });
                ContextEntry {
                    name: selected.title.clone(),
                    tokens,
                }
            })
            .collect())
    }

    /// The enabled skills. A skill that is off never reaches a prompt.
    fn add_skills(&self, sections: &mut Sections) -> Result<()> {
        let skills: Vec<ContextEntry> = self
            .load_skills()?
            .into_iter()
            .filter(|skill| skill.info.mode != SkillMode::Off)
            .map(|skill| ContextEntry {
                name: skill.info.name,
                tokens: estimate_tokens(&skill.body),
            })
            .collect();
        sections.item("skills", "Skills", total(&skills));
        sections.skills = skills;
        Ok(())
    }

    /// Files, folders, and uploads waiting for the next message. Each is counted as the text a
    /// run would get for it.
    fn add_attachments(&self, sections: &mut Sections, pending: &[Attachment]) {
        let repo = self.repo_path();
        let entries: Vec<ContextEntry> = pending
            .iter()
            .filter(|attachment| {
                matches!(
                    attachment.kind,
                    AttachmentKind::File | AttachmentKind::Folder | AttachmentKind::Upload
                )
            })
            .map(|attachment| ContextEntry {
                name: attachment.name.clone(),
                tokens: estimate_tokens(&context_text(&repo, slice::from_ref(attachment))),
            })
            .collect();
        sections.item("attachments", "Attachments", total(&entries));
        sections.attachments = entries;
    }

    /// The backlog file named in the config. A missing file counts as nothing.
    fn add_backlog(&self, sections: &mut Sections) {
        let path = self.repo_path().join(self.config().backlog);
        let tokens = fs::read_to_string(path).map_or(0, |text| estimate_tokens(&text));
        sections.item("backlog", "Backlog", tokens);
    }
}

fn messages_tokens(records: &[EventRecord]) -> u32 {
    records
        .iter()
        .filter_map(|record| message_text(&record.event))
        .fold(0_u32, |sum, text| sum.saturating_add(estimate_tokens(text)))
}

#[cfg(test)]
mod tests {
    use sentinel_brain::note::{Author, Note};
    use sentinel_core::{NoteId, NoteType};

    use super::*;
    use crate::brain::write_note;
    use crate::brain_seed::fixtures::{engine_for, sample_repo};
    use crate::controls::Caller;

    fn tokens_of(usage: &ContextUsage, kind: &str) -> u32 {
        usage
            .items
            .iter()
            .find(|item| item.kind == kind)
            .map_or(0, |item| item.tokens)
    }

    #[test]
    fn empty_text_is_zero_tokens() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn four_characters_are_one_token() {
        assert_eq!(estimate_tokens("abcd"), 1);
    }

    #[test]
    fn five_characters_round_up_to_two_tokens() {
        assert_eq!(estimate_tokens("abcde"), 2);
    }

    #[test]
    fn the_window_follows_the_adapter_and_defaults_for_the_rest() {
        assert_eq!(window_for_adapter("claude"), 200_000);
        assert_eq!(window_for_adapter("gemini"), 1_000_000);
        assert_eq!(window_for_adapter("pi"), 128_000);
        assert_eq!(window_for_adapter("unknown-cli"), 128_000);
    }

    #[test]
    fn free_space_is_never_negative_when_the_items_pass_the_window() {
        let mut sections = Sections::default();
        sections.item("messages", "Messages", 200_000);
        let usage = sections.into_usage(MidAgent {
            name: "claude".to_owned(),
            window: 128_000,
        });
        assert_eq!(tokens_of(&usage, "free"), 0);
        assert_eq!(usage.used_tokens, 200_000);
    }

    #[test]
    fn a_chat_with_two_messages_and_a_note_counts_messages_and_brain() {
        let (_dir, repo) = sample_repo();
        let engine = engine_for(&repo);
        let chat = engine
            .create_chat(Some("Ledger"), &Caller::pc("You"))
            .unwrap();
        for text in ["Fix ledger amounts", "Amounts are micro-dollars"] {
            let message = Event::Message {
                from: "You".to_owned(),
                to: None,
                text: text.to_owned(),
            };
            engine.emit_in(Some(&chat.id), &message).unwrap();
        }
        let note = Note {
            id: NoteId::new("ledger-amounts"),
            note_type: NoteType::Convention,
            title: "Ledger amounts".to_owned(),
            tags: Vec::new(),
            by: Author::Run,
            strength: 0.5,
            confirmed: None,
            pinned: false,
            lesson: None,
            body: "Ledger amounts are integer micro-dollars.".to_owned(),
            extra: Vec::new(),
        };
        let state = engine.brain().unwrap();
        write_note(&state, &note, None).unwrap();

        let usage = engine.context_usage(Some(&chat.id)).unwrap();
        assert!(tokens_of(&usage, "messages") > 0, "{usage:?}");
        assert!(tokens_of(&usage, "brain") > 0, "{usage:?}");
        assert!(usage.notes.iter().any(|note| note.name == "Ledger amounts"));
    }

    #[test]
    fn a_fresh_engine_answers_with_an_unrouted_agent_and_a_free_item_last() {
        let (_dir, repo) = sample_repo();
        let engine = engine_for(&repo);
        let usage = engine.context_usage(None).unwrap();
        assert_eq!(usage.agent, "none");
        assert_eq!(
            usage.items.last().map(|item| item.kind.as_str()),
            Some("free")
        );
        assert_eq!(
            usage.used_tokens + tokens_of(&usage, "free"),
            usage.window_tokens
        );
    }
}
