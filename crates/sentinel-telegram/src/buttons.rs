//! One-time button tokens. A button carries only a token. The token maps to one action, one chat,
//! and a 10 minute life. A token redeems once.

use std::collections::BTreeMap;
use std::hash::{BuildHasher, Hasher, RandomState};

use sentinel_core::ids::{ApprovalId, DecisionId, TaskId, ToolId};
use thiserror::Error;

use crate::levels::Action;

/// A token stops working this many seconds after it was issued.
pub const TOKEN_LIFETIME_SECONDS: i64 = 600;
const TOKEN_LEN: usize = 16;
const ALPHABET: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ButtonAction {
    Approval {
        approval: ApprovalId,
        allow: bool,
    },
    /// `answer` is a task id, or `skip_both`.
    Decision {
        decision: DecisionId,
        answer: String,
    },
    Pause,
    Resume,
    Stop,
    Report,
    DiffSummary {
        task: Option<TaskId>,
    },
    StartRun {
        task: Option<TaskId>,
    },
    AddTask {
        text: String,
    },
    Kill {
        agent: ToolId,
    },
}

impl ButtonAction {
    /// The route the button calls. The router checks it against the chat's levels.
    pub fn action(&self) -> Action {
        match self {
            ButtonAction::Approval { .. } => Action::Approve,
            ButtonAction::Decision { .. } => Action::Decide,
            ButtonAction::Pause => Action::Pause,
            ButtonAction::Resume => Action::Resume,
            ButtonAction::Stop => Action::Stop,
            ButtonAction::Report => Action::Report,
            ButtonAction::DiffSummary { .. } => Action::DiffSummary,
            ButtonAction::StartRun { .. } => Action::StartRun,
            ButtonAction::AddTask { .. } => Action::AddTask,
            ButtonAction::Kill { .. } => Action::Kill,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ButtonError {
    #[error("unknown button")]
    Unknown,
    #[error("button expired")]
    Expired,
    #[error("button belongs to another chat")]
    WrongChat,
    #[error("button was already used")]
    AlreadyUsed,
}

#[derive(Debug)]
struct TokenEntry {
    chat_id: i64,
    action: ButtonAction,
    issued_at: i64,
    used: bool,
}

#[derive(Debug, Default)]
pub struct ButtonTokens {
    issued: BTreeMap<String, TokenEntry>,
    counter: u64,
}

impl ButtonTokens {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a 16 character token. The token is the whole callback data.
    pub fn issue(&mut self, chat_id: i64, action: &ButtonAction, now: i64) -> String {
        self.forget_old(now);
        let token = self.fresh_token(now);
        self.issued.insert(
            token.clone(),
            TokenEntry {
                chat_id,
                action: action.clone(),
                issued_at: now,
                used: false,
            },
        );
        token
    }

    /// A wrong chat does not use up the token. The right chat can still redeem it.
    pub fn redeem(
        &mut self,
        token: &str,
        chat_id: i64,
        now: i64,
    ) -> Result<ButtonAction, ButtonError> {
        let issued = self.issued.get_mut(token).ok_or(ButtonError::Unknown)?;
        if issued.chat_id != chat_id {
            return Err(ButtonError::WrongChat);
        }
        if issued.used {
            return Err(ButtonError::AlreadyUsed);
        }
        if now - issued.issued_at > TOKEN_LIFETIME_SECONDS {
            return Err(ButtonError::Expired);
        }
        issued.used = true;
        Ok(issued.action.clone())
    }

    /// Entries stay twice the life, so an old token answers `Expired`, not `Unknown`.
    fn forget_old(&mut self, now: i64) {
        self.issued
            .retain(|_, issued| now - issued.issued_at <= 2 * TOKEN_LIFETIME_SECONDS);
    }

    fn fresh_token(&mut self, now: i64) -> String {
        loop {
            self.counter += 1;
            let token = random_token(self.counter, now);
            if !self.issued.contains_key(&token) {
                return token;
            }
        }
    }
}

fn random_token(counter: u64, now: i64) -> String {
    let high = u128::from(random_word(counter));
    let low = u128::from(random_word(now.unsigned_abs()));
    let mut value = (high << 64) | low;
    (0..TOKEN_LEN)
        .map(|_| {
            let index = usize::try_from(value % 62).unwrap_or_default();
            value /= 62;
            char::from(ALPHABET[index])
        })
        .collect()
}

/// `RandomState` gets a fresh key from the OS, so the output cannot be guessed from the counter.
fn random_word(salt: u64) -> u64 {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(salt);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::{ButtonAction, ButtonTokens, TOKEN_LIFETIME_SECONDS};

    #[test]
    fn a_token_is_sixteen_letters_and_digits() {
        let token = ButtonTokens::new().issue(1, &ButtonAction::Pause, 0);
        assert_eq!(token.len(), 16);
        assert!(token.chars().all(|c| c.is_ascii_alphanumeric()), "{token}");
    }

    #[test]
    fn two_tokens_differ() {
        let mut tokens = ButtonTokens::new();
        let first = tokens.issue(1, &ButtonAction::Pause, 0);
        let second = tokens.issue(1, &ButtonAction::Pause, 0);
        assert_ne!(first, second);
    }

    #[test]
    fn the_lifetime_is_ten_minutes() {
        assert_eq!(TOKEN_LIFETIME_SECONDS, 600);
    }
}
