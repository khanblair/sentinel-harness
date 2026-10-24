//! In-memory approval cards. The caller persists them and emits the events.

use std::collections::HashMap;

use sentinel_core::{Approval, ApprovalId, ApprovalKind, ApprovalState, TaskId, ToolId};
use time::{Duration, OffsetDateTime};

/// An in-memory book of pending and answered approval cards.
#[derive(Debug, Default)]
pub struct ApprovalBook {
    cards: HashMap<String, CardEntry>,
    next_id: u64,
}

#[derive(Debug, Clone)]
struct CardEntry {
    approval: Approval,
    created_at: OffsetDateTime,
}

/// What a new card is about. Only the kind is set on every card.
struct NewCard {
    kind: ApprovalKind,
    task: Option<TaskId>,
    agent: Option<ToolId>,
    path: Option<String>,
}

impl NewCard {
    fn unscoped(kind: ApprovalKind) -> Self {
        Self {
            kind,
            task: None,
            agent: None,
            path: None,
        }
    }
}

impl ApprovalBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// Request an extension card. Returns the existing card for the same task and path.
    #[allow(clippy::too_many_arguments)]
    pub fn request_extension(
        &mut self,
        task: &TaskId,
        agent: &ToolId,
        path: &str,
        now: OffsetDateTime,
    ) -> Approval {
        let key = format!("ext:{}:{}", task.as_str(), path);
        let card = NewCard {
            kind: ApprovalKind::Extension,
            task: Some(task.clone()),
            agent: Some(agent.clone()),
            path: Some(path.to_owned()),
        };
        self.open_card(key, card, now)
    }

    /// Request a wave card.
    pub fn request_wave(&mut self, wave: u32, now: OffsetDateTime) -> Approval {
        let card = NewCard::unscoped(ApprovalKind::Wave);
        self.open_card(format!("wave:{wave}"), card, now)
    }

    /// Request a plan card.
    pub fn request_plan(&mut self, now: OffsetDateTime) -> Approval {
        let card = NewCard::unscoped(ApprovalKind::Plan);
        self.open_card("plan".to_owned(), card, now)
    }

    /// Return the card under `key`, or open a new pending one. The lookup comes
    /// first so a repeat request does not use up an id.
    fn open_card(&mut self, key: String, card: NewCard, now: OffsetDateTime) -> Approval {
        if let Some(entry) = self.cards.get(&key) {
            return entry.approval.clone();
        }
        let id = ApprovalId::new(format!("A-{}", self.next_id));
        self.next_id += 1;
        let approval = Approval {
            id,
            kind: card.kind,
            task: card.task,
            agent: card.agent,
            path: card.path,
            state: ApprovalState::Pending,
            decided_by: None,
        };
        self.cards.insert(
            key,
            CardEntry {
                approval: approval.clone(),
                created_at: now,
            },
        );
        approval
    }

    /// Answer a card. Returns None if unknown or already answered.
    pub fn answer(&mut self, id: &ApprovalId, allow: bool, decided_by: &str) -> Option<Approval> {
        let entry = self.cards.values_mut().find(|e| e.approval.id == *id)?;
        if entry.approval.state != ApprovalState::Pending {
            return None;
        }
        entry.approval.state = if allow {
            ApprovalState::Allowed
        } else {
            ApprovalState::Denied
        };
        entry.approval.decided_by = Some(decided_by.to_owned());
        Some(entry.approval.clone())
    }

    /// Default stale pending cards older than the limit.
    pub fn default_stale(
        &mut self,
        now: OffsetDateTime,
        older_than: Duration,
        kinds: &[ApprovalKind],
    ) -> Vec<Approval> {
        let mut defaulted = Vec::new();
        for entry in self.cards.values_mut() {
            if entry.approval.state != ApprovalState::Pending {
                continue;
            }
            if !kinds.is_empty() && !kinds.contains(&entry.approval.kind) {
                continue;
            }
            if now - entry.created_at >= older_than {
                entry.approval.state = ApprovalState::Defaulted;
                entry.approval.decided_by = Some("default".to_owned());
                defaulted.push(entry.approval.clone());
            }
        }
        defaulted
    }

    /// All pending cards.
    pub fn pending(&self) -> Vec<Approval> {
        self.cards
            .values()
            .filter(|e| e.approval.state == ApprovalState::Pending)
            .map(|e| e.approval.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{ApprovalKind, TaskId, ToolId};
    use time::OffsetDateTime;

    fn now() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    #[test]
    fn duplicate_extension_request_returns_same_card() {
        let mut book = ApprovalBook::new();
        let task = TaskId::new("T-01");
        let agent = ToolId::new("claude");
        let first = book.request_extension(&task, &agent, "src/main.rs", now());
        let second = book.request_extension(&task, &agent, "src/main.rs", now());
        assert_eq!(first.id, second.id);
    }

    #[test]
    fn answer_once_then_none() {
        let mut book = ApprovalBook::new();
        let task = TaskId::new("T-01");
        let agent = ToolId::new("claude");
        let card = book.request_extension(&task, &agent, "src/main.rs", now());
        let answered = book.answer(&card.id, true, "phone").unwrap();
        assert_eq!(answered.state, ApprovalState::Allowed);
        assert_eq!(book.answer(&card.id, false, "phone"), None);
    }

    #[test]
    fn default_stale_after_limit_not_before() {
        let mut book = ApprovalBook::new();
        let task = TaskId::new("T-01");
        let agent = ToolId::new("claude");
        let _card = book.request_extension(&task, &agent, "src/main.rs", now());
        let later = now() + time::Duration::minutes(11);
        let defaulted = book.default_stale(
            later,
            time::Duration::minutes(10),
            &[ApprovalKind::Extension],
        );
        assert_eq!(defaulted.len(), 1);
        assert_eq!(defaulted[0].state, ApprovalState::Defaulted);
        assert_eq!(defaulted[0].decided_by.as_deref(), Some("default"));
    }

    #[test]
    fn default_stale_respects_kind_filter() {
        let mut book = ApprovalBook::new();
        let task = TaskId::new("T-01");
        let agent = ToolId::new("claude");
        book.request_extension(&task, &agent, "src/main.rs", now());
        book.request_wave(1, now());
        let later = now() + time::Duration::minutes(11);
        let defaulted = book.default_stale(
            later,
            time::Duration::minutes(10),
            &[ApprovalKind::Extension],
        );
        assert_eq!(defaulted.len(), 1);
        assert_eq!(defaulted[0].kind, ApprovalKind::Extension);
    }
}
