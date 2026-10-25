//! Slash commands. One parser and one list. Each command maps to one API route in `composer.rs`,
//! and never does more than that route (`docs/code-standards.md`, UI Code).

use sentinel_core::{BrainMode, Intent};

/// A command name and the text shown beside it in the menu.
pub type SlashEntry = (&'static str, &'static str);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommand {
    Pause,
    Resume,
    Stop,
    Cap(String),
    Agent(String),
    Focus(String),
    Kill(String),
    Skill(String),
    Brain(BrainMode),
    Report,
    Help,
}

/// The prototype's `SLASH` list, with the message types after `/plan`.
const COMMANDS: [SlashEntry; 16] = [
    ("/pause", "Pause the run"),
    ("/resume", "Resume the run"),
    ("/stop", "Stop the run"),
    ("/cap 1.50", "Lower the budget cap"),
    ("/agent claude", "Set the agent of this chat"),
    ("/focus codex", "Show one agent in the stream"),
    ("/kill codex", "Kill an agent"),
    ("/plan", "Send the next message as a plan"),
    ("/ask", "Ask a question. Changes no file."),
    (
        "/research",
        "Investigate and give evidence. Changes no file.",
    ),
    ("/fix", "Change files in the reserved files."),
    ("/chat", "Small talk. Reads no file."),
    ("/skill house-style", "Add a skill to your next message"),
    ("/brain off", "Set the Brain mode"),
    ("/report", "Open the report"),
    ("/help", "List commands"),
];

pub fn help_lines() -> Vec<SlashEntry> {
    COMMANDS.to_vec()
}

/// `None` when the text is not a command. `Err` carries the message for a bad command.
pub fn parse(text: &str) -> Option<Result<SlashCommand, String>> {
    let body = text.trim().strip_prefix('/')?;
    let mut words = body.split_whitespace();
    let name = words.next().unwrap_or("");
    let arg = words.collect::<Vec<_>>().join(" ");
    Some(build(name, &arg))
}

fn build(name: &str, arg: &str) -> Result<SlashCommand, String> {
    match name {
        "pause" => no_arg(name, arg, SlashCommand::Pause),
        "resume" => no_arg(name, arg, SlashCommand::Resume),
        "stop" => no_arg(name, arg, SlashCommand::Stop),
        "report" => no_arg(name, arg, SlashCommand::Report),
        "help" => no_arg(name, arg, SlashCommand::Help),
        "cap" => amount(arg),
        "agent" => required(name, arg, SlashCommand::Agent),
        "focus" => required(name, arg, SlashCommand::Focus),
        "kill" => required(name, arg, SlashCommand::Kill),
        "brain" => brain_mode(arg),
        "skill" => skill_name(arg),
        "" => Err("Type a command name after the slash. Type /help for the list.".to_owned()),
        name if intent_for_name(name).is_some() => Err(format!(
            "/{name} sends your text with that type. Type it before the text."
        )),
        other => Err(format!(
            "Unknown command /{other}. Type /help for the list."
        )),
    }
}

/// The message type a command names. `/fix` names a change.
pub fn intent_for_name(name: &str) -> Option<Intent> {
    match name {
        "chat" => Some(Intent::Chat),
        "ask" => Some(Intent::Ask),
        "research" => Some(Intent::Research),
        "plan" => Some(Intent::Plan),
        "fix" => Some(Intent::Change),
        _ => None,
    }
}

/// A type command typed before the text, such as `/ask where is the fee logic?`. The text is
/// what follows the command, with its line breaks kept. It is empty for a bare command.
pub fn parse_intent_command(text: &str) -> Option<(Intent, String)> {
    let body = text.trim().strip_prefix('/')?;
    let (name, rest) = body.split_once(char::is_whitespace).unwrap_or((body, ""));
    Some((intent_for_name(name)?, rest.trim().to_owned()))
}

fn no_arg(name: &str, arg: &str, command: SlashCommand) -> Result<SlashCommand, String> {
    if arg.is_empty() {
        Ok(command)
    } else {
        Err(format!("/{name} takes no text."))
    }
}

fn required(
    name: &str,
    arg: &str,
    build: fn(String) -> SlashCommand,
) -> Result<SlashCommand, String> {
    if arg.is_empty() {
        Err(format!("/{name} needs a name, for example /{name} claude."))
    } else {
        Ok(build(arg.to_owned()))
    }
}

fn amount(arg: &str) -> Result<SlashCommand, String> {
    match arg.parse::<f64>() {
        Ok(_) => Ok(SlashCommand::Cap(arg.to_owned())),
        Err(_) => Err("/cap needs a number, for example /cap 1.50.".to_owned()),
    }
}

fn skill_name(arg: &str) -> Result<SlashCommand, String> {
    if arg.is_empty() {
        Err("/skill needs a name, for example /skill house-style.".to_owned())
    } else {
        Ok(SlashCommand::Skill(arg.to_owned()))
    }
}

fn brain_mode(arg: &str) -> Result<SlashCommand, String> {
    match arg {
        "off" => Ok(SlashCommand::Brain(BrainMode::Off)),
        "read-only" => Ok(SlashCommand::Brain(BrainMode::ReadOnly)),
        "learn" => Ok(SlashCommand::Brain(BrainMode::Learn)),
        _ => Err("Use /brain off, /brain read-only or /brain learn.".to_owned()),
    }
}

/// The commands the menu shows under the composer for the text typed so far.
pub fn matches(text: &str) -> Vec<SlashEntry> {
    if !text.starts_with('/') {
        return Vec::new();
    }
    let query = text.split_whitespace().next().unwrap_or("/");
    let typed_argument = text.chars().any(char::is_whitespace);
    let known = COMMANDS
        .iter()
        .any(|(command, _)| command.split_whitespace().next() == Some(query));
    if typed_argument && !known {
        return Vec::new();
    }
    COMMANDS
        .iter()
        .copied()
        .filter(|(command, _)| command.starts_with(query))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(text: &str) -> Result<SlashCommand, String> {
        parse(text).expect("a slash command")
    }

    #[test]
    fn plain_text_is_not_a_command() {
        assert_eq!(parse("fix the fee ledger"), None);
        assert_eq!(parse("  hello"), None);
    }

    #[test]
    fn bare_commands_parse_to_their_variant() {
        assert_eq!(cmd("/pause"), Ok(SlashCommand::Pause));
        assert_eq!(cmd("/resume"), Ok(SlashCommand::Resume));
        assert_eq!(cmd("/stop"), Ok(SlashCommand::Stop));
        assert_eq!(cmd("/report"), Ok(SlashCommand::Report));
        assert_eq!(cmd("/help"), Ok(SlashCommand::Help));
    }

    #[test]
    fn commands_with_a_name_or_amount_keep_the_argument() {
        assert_eq!(cmd("/cap 1.50"), Ok(SlashCommand::Cap("1.50".to_owned())));
        assert_eq!(
            cmd("/agent claude"),
            Ok(SlashCommand::Agent("claude".to_owned()))
        );
        assert_eq!(
            cmd("/focus codex"),
            Ok(SlashCommand::Focus("codex".to_owned()))
        );
        assert_eq!(
            cmd("/kill codex"),
            Ok(SlashCommand::Kill("codex".to_owned()))
        );
    }

    #[test]
    fn skill_takes_the_name_of_one_skill() {
        assert_eq!(
            cmd("/skill house-style"),
            Ok(SlashCommand::Skill("house-style".to_owned()))
        );
        assert!(cmd("/skill").is_err());
    }

    #[test]
    fn brain_takes_one_of_three_modes() {
        assert_eq!(cmd("/brain off"), Ok(SlashCommand::Brain(BrainMode::Off)));
        assert_eq!(
            cmd("/brain read-only"),
            Ok(SlashCommand::Brain(BrainMode::ReadOnly))
        );
        assert_eq!(
            cmd("/brain learn"),
            Ok(SlashCommand::Brain(BrainMode::Learn))
        );
        assert!(cmd("/brain loose").is_err());
        assert!(cmd("/brain").is_err());
    }

    #[test]
    fn bad_inputs_return_a_message() {
        assert!(cmd("/cap").is_err());
        assert!(cmd("/cap lots").is_err());
        assert!(cmd("/agent").is_err());
        assert!(cmd("/focus").is_err());
        assert!(cmd("/kill").is_err());
        assert!(cmd("/pause now").is_err());
        assert!(cmd("/nope").is_err());
        assert!(cmd("/").is_err());
    }

    #[test]
    fn a_type_name_alone_is_not_a_plain_command() {
        assert!(cmd("/ask").is_err());
        assert!(cmd("/fix").is_err());
    }

    #[test]
    fn a_type_command_before_text_gives_its_type_and_the_text() {
        assert_eq!(
            parse_intent_command("/ask where is the fee logic?"),
            Some((Intent::Ask, "where is the fee logic?".to_owned()))
        );
        assert_eq!(
            parse_intent_command("  /fix  add a retry\nto the sync  "),
            Some((Intent::Change, "add a retry\nto the sync".to_owned()))
        );
    }

    #[test]
    fn a_bare_type_command_gives_an_empty_text() {
        assert_eq!(
            parse_intent_command("/research"),
            Some((Intent::Research, String::new()))
        );
    }

    #[test]
    fn each_type_command_names_its_type() {
        assert_eq!(intent_for_name("chat"), Some(Intent::Chat));
        assert_eq!(intent_for_name("plan"), Some(Intent::Plan));
        assert_eq!(intent_for_name("fix"), Some(Intent::Change));
    }

    #[test]
    fn words_that_only_start_with_a_type_name_are_not_type_commands() {
        assert_eq!(parse_intent_command("/askew the fee"), None);
        assert_eq!(parse_intent_command("/cap 1.50"), None);
        assert_eq!(parse_intent_command("ask me"), None);
    }

    #[test]
    fn help_lists_every_command_once() {
        let lines = help_lines();
        assert_eq!(lines.len(), 16);
        assert_eq!(lines[0], ("/pause", "Pause the run"));
        assert!(lines.iter().any(|(name, _)| *name == "/help"));
    }

    #[test]
    fn menu_shows_commands_that_start_with_the_typed_word() {
        assert_eq!(matches("/ca"), vec![("/cap 1.50", "Lower the budget cap")]);
        assert_eq!(
            matches("/as"),
            vec![("/ask", "Ask a question. Changes no file.")]
        );
        assert_eq!(matches("/").len(), 16);
    }

    #[test]
    fn menu_hides_for_plain_text_and_unknown_arguments() {
        assert_eq!(matches("hello"), Vec::new());
        assert_eq!(matches("/zzz"), Vec::new());
        assert_eq!(matches("/zzz arg"), Vec::new());
        assert_ne!(matches("/cap 1"), Vec::new());
    }
}
