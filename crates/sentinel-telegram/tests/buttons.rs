#![allow(clippy::unwrap_used, reason = "tests fail loudly on setup errors")]

use sentinel_core::{ApprovalId, TaskId};
use sentinel_telegram::buttons::{ButtonAction, ButtonError, ButtonTokens};
use sentinel_telegram::levels::Action;

const CHAT: i64 = 99;
const OTHER_CHAT: i64 = 100;
const ISSUED_AT: i64 = 1_000;

fn approve() -> ButtonAction {
    ButtonAction::Approval {
        approval: ApprovalId::new("a1"),
        allow: true,
    }
}

#[test]
fn a_token_redeems_at_exactly_600_seconds() {
    let mut tokens = ButtonTokens::new();
    let token = tokens.issue(CHAT, &approve(), ISSUED_AT);

    assert_eq!(tokens.redeem(&token, CHAT, ISSUED_AT + 600), Ok(approve()));
}

#[test]
fn a_token_is_expired_at_601_seconds() {
    let mut tokens = ButtonTokens::new();
    let token = tokens.issue(CHAT, &approve(), ISSUED_AT);

    assert_eq!(
        tokens.redeem(&token, CHAT, ISSUED_AT + 601),
        Err(ButtonError::Expired)
    );
}

#[test]
fn a_token_redeems_once() {
    let mut tokens = ButtonTokens::new();
    let token = tokens.issue(CHAT, &approve(), ISSUED_AT);
    assert!(tokens.redeem(&token, CHAT, ISSUED_AT + 1).is_ok());

    assert_eq!(
        tokens.redeem(&token, CHAT, ISSUED_AT + 2),
        Err(ButtonError::AlreadyUsed)
    );
}

#[test]
fn a_token_from_another_chat_fails_and_does_not_use_up_the_token() {
    let mut tokens = ButtonTokens::new();
    let token = tokens.issue(CHAT, &approve(), ISSUED_AT);

    assert_eq!(
        tokens.redeem(&token, OTHER_CHAT, ISSUED_AT + 1),
        Err(ButtonError::WrongChat)
    );
    assert_eq!(tokens.redeem(&token, CHAT, ISSUED_AT + 2), Ok(approve()));
}

#[test]
fn an_unknown_token_is_unknown() {
    let mut tokens = ButtonTokens::new();

    assert_eq!(
        tokens.redeem("not-a-token", CHAT, ISSUED_AT),
        Err(ButtonError::Unknown)
    );
}

#[test]
fn a_redeemed_action_keeps_its_task_and_answer() {
    let mut tokens = ButtonTokens::new();
    let action = ButtonAction::DiffSummary {
        task: Some(TaskId::new("T-04")),
    };
    let token = tokens.issue(CHAT, &action, ISSUED_AT);

    assert_eq!(tokens.redeem(&token, CHAT, ISSUED_AT), Ok(action));
}

#[test]
fn each_button_action_names_its_route() {
    assert_eq!(approve().action(), Action::Approve);
    assert_eq!(ButtonAction::Pause.action(), Action::Pause);
    assert_eq!(
        ButtonAction::StartRun { task: None }.action(),
        Action::StartRun
    );
    assert_eq!(
        ButtonAction::AddTask {
            text: "x".to_owned()
        }
        .action(),
        Action::AddTask
    );
}

#[test]
fn a_token_is_sixteen_characters_and_fits_telegram_callback_data() {
    let mut tokens = ButtonTokens::new();
    let token = tokens.issue(CHAT, &ButtonAction::Stop, ISSUED_AT);

    assert_eq!(token.len(), 16);
    assert!(token.len() <= 64);
}
