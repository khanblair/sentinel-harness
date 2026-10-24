#![allow(clippy::unwrap_used, reason = "tests fail loudly on setup errors")]

use std::collections::BTreeSet;

use sentinel_core::Cost;
use sentinel_core::ids::{TaskId, ToolId};
use sentinel_telegram::command::{Command, ParseProblem, help_text, is_allowed, parse};
use sentinel_telegram::levels::{Action, Level};

#[test]
fn each_plain_command_parses_to_its_variant() {
    let cases = [
        ("/start", Command::Start),
        ("/help", Command::Help),
        ("/status", Command::Status),
        ("/pause", Command::Pause),
        ("/resume", Command::Resume),
        ("/stop", Command::Stop),
        ("/report", Command::Report),
        ("/daily", Command::Daily),
    ];
    for (text, expected) in cases {
        assert_eq!(parse(text), Ok(expected), "{text}");
    }
}

#[test]
fn cap_parses_a_dollar_amount_in_micro_dollars() {
    let expected = Cost::from_micro_usd(1_500_000, false);
    assert_eq!(parse("/cap 1.50"), Ok(Command::Cap(expected)));
}

#[test]
fn a_bad_cap_amount_is_a_bad_cap_problem() {
    assert_eq!(
        parse("/cap lots"),
        Err(ParseProblem::BadCap("lots".to_owned()))
    );
    assert_eq!(parse("/cap -2"), Err(ParseProblem::BadCap("-2".to_owned())));
}

#[test]
fn cap_without_an_amount_is_missing_its_argument() {
    assert_eq!(
        parse("/cap"),
        Err(ParseProblem::Missing("Usage: /cap 1.50"))
    );
}

#[test]
fn kill_takes_the_agent_name() {
    assert_eq!(
        parse("/kill codex"),
        Ok(Command::Kill(ToolId::new("codex")))
    );
    assert!(matches!(parse("/kill"), Err(ParseProblem::Missing(_))));
}

#[test]
fn agent_takes_a_name_and_the_rest_as_text() {
    let expected = Command::Agent {
        tool: ToolId::new("claude"),
        text: "use the new helper".to_owned(),
    };
    assert_eq!(parse("/agent claude use the new helper"), Ok(expected));
}

#[test]
fn agent_without_text_is_missing_its_argument() {
    assert!(matches!(
        parse("/agent claude"),
        Err(ParseProblem::Missing(_))
    ));
    assert!(matches!(parse("/agent"), Err(ParseProblem::Missing(_))));
}

#[test]
fn diff_and_run_take_an_optional_task() {
    assert_eq!(parse("/diff"), Ok(Command::Diff(None)));
    assert_eq!(
        parse("/diff T-04"),
        Ok(Command::Diff(Some(TaskId::new("T-04"))))
    );
    assert_eq!(parse("/run"), Ok(Command::Run(None)));
    assert_eq!(
        parse("/run T-13"),
        Ok(Command::Run(Some(TaskId::new("T-13"))))
    );
}

#[test]
fn task_keeps_the_whole_text() {
    assert_eq!(
        parse("/task Add a rate limit to transfers"),
        Ok(Command::Task("Add a rate limit to transfers".to_owned()))
    );
    assert!(matches!(parse("/task"), Err(ParseProblem::Missing(_))));
}

#[test]
fn brain_takes_a_search_and_needs_one() {
    assert_eq!(
        parse("/brain rate limit"),
        Ok(Command::Brain("rate limit".to_owned()))
    );
    assert!(matches!(parse("/brain"), Err(ParseProblem::Missing(_))));
}

#[test]
fn a_bot_name_suffix_is_ignored() {
    assert_eq!(parse("/status@SentinelBot"), Ok(Command::Status));
    assert_eq!(
        parse("/cap@SentinelBot 2"),
        Ok(Command::Cap(Cost::from_micro_usd(2_000_000, false)))
    );
}

#[test]
fn an_unknown_command_is_unknown() {
    assert_eq!(
        parse("/shell ls"),
        Err(ParseProblem::Unknown("/shell".to_owned()))
    );
}

#[test]
fn text_without_a_slash_is_unknown() {
    assert!(matches!(parse("status"), Err(ParseProblem::Unknown(_))));
}

#[test]
fn each_command_maps_to_its_route() {
    assert_eq!(Command::Status.action(), Some(Action::Status));
    assert_eq!(Command::Cap(Cost::ZERO).action(), Some(Action::LowerCap));
    assert_eq!(Command::Run(None).action(), Some(Action::StartRun));
    assert_eq!(Command::Task("x".into()).action(), Some(Action::AddTask));
    assert_eq!(
        Command::Brain("x".into()).action(),
        Some(Action::BrainSearch)
    );
    assert_eq!(Command::Start.action(), None);
    assert_eq!(Command::Help.action(), None);
}

#[test]
fn help_lists_only_the_commands_the_levels_allow() {
    let reports = BTreeSet::from([Level::Reports]);
    let text = help_text(&reports);
    assert!(text.contains("/report"), "{text}");
    assert!(text.contains("/daily"), "{text}");
    assert!(!text.contains("/pause"), "{text}");
    assert!(text.contains("/help"), "{text}");
}

#[test]
fn help_with_every_level_lists_every_command() {
    let all = BTreeSet::from([Level::Control, Level::Code, Level::Reports]);
    let text = help_text(&all);
    for usage in ["/pause", "/kill", "/agent", "/task", "/run", "/brain"] {
        assert!(text.contains(usage), "{usage} missing in {text}");
    }
}

#[test]
fn only_the_one_allowed_user_passes_the_check() {
    assert!(is_allowed(42, 42));
    assert!(!is_allowed(43, 42));
}
