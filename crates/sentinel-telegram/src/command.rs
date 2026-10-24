//! Slash commands from the chat. A command maps to one route. Parsing never runs anything.

use std::collections::BTreeSet;

use sentinel_core::Cost;
use sentinel_core::ids::{TaskId, ToolId};

use crate::levels::{Action, Level, allowed};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Start,
    Help,
    Status,
    Pause,
    Resume,
    Stop,
    Cap(Cost),
    Kill(ToolId),
    Agent { tool: ToolId, text: String },
    Diff(Option<TaskId>),
    Task(String),
    Run(Option<TaskId>),
    Report,
    Daily,
    Brain(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseProblem {
    /// The command name is not one Sentinel knows.
    Unknown(String),
    /// A required argument is missing. The text is the usage line.
    Missing(&'static str),
    /// The `/cap` amount is not a dollar amount.
    BadCap(String),
}

/// The only sender the bot answers. Every other sender is ignored and logged.
pub fn is_allowed(update_from: i64, allowed_user: i64) -> bool {
    update_from == allowed_user
}

/// Parses `/name args`, with an optional `@BotName` after the name.
pub fn parse(text: &str) -> Result<Command, ParseProblem> {
    let text = text.trim();
    let (head, rest) = split_first(text);
    let Some(name) = head.strip_prefix('/') else {
        return Err(ParseProblem::Unknown(head.to_owned()));
    };
    let name = name.split_once('@').map_or(name, |(name, _)| name);
    parse_named(name, rest)
}

fn parse_named(name: &str, rest: &str) -> Result<Command, ParseProblem> {
    match name {
        "start" => Ok(Command::Start),
        "help" => Ok(Command::Help),
        "status" => Ok(Command::Status),
        "pause" => Ok(Command::Pause),
        "resume" => Ok(Command::Resume),
        "stop" => Ok(Command::Stop),
        "report" => Ok(Command::Report),
        "daily" => Ok(Command::Daily),
        "cap" => parse_cap(rest),
        "kill" => required(rest, "Usage: /kill <agent>")
            .map(|agent| Command::Kill(ToolId::new(split_first(agent).0))),
        "agent" => parse_agent(rest),
        "diff" => Ok(Command::Diff(optional_task(rest))),
        "run" => Ok(Command::Run(optional_task(rest))),
        "task" => required(rest, "Usage: /task <text>").map(|text| Command::Task(text.to_owned())),
        "brain" => {
            required(rest, "Usage: /brain <search>").map(|search| Command::Brain(search.to_owned()))
        }
        other => Err(ParseProblem::Unknown(format!("/{other}"))),
    }
}

fn parse_cap(rest: &str) -> Result<Command, ParseProblem> {
    let amount = required(rest, "Usage: /cap 1.50")?;
    Cost::parse_usd(amount)
        .map(Command::Cap)
        .map_err(|_| ParseProblem::BadCap(amount.to_owned()))
}

fn parse_agent(rest: &str) -> Result<Command, ParseProblem> {
    let usage = "Usage: /agent <name> <text>";
    let (tool, text) = split_first(required(rest, usage)?);
    if text.is_empty() {
        return Err(ParseProblem::Missing(usage));
    }
    Ok(Command::Agent {
        tool: ToolId::new(tool),
        text: text.to_owned(),
    })
}

fn optional_task(rest: &str) -> Option<TaskId> {
    (!rest.is_empty()).then(|| TaskId::new(split_first(rest).0))
}

fn required<'a>(rest: &'a str, usage: &'static str) -> Result<&'a str, ParseProblem> {
    if rest.is_empty() {
        Err(ParseProblem::Missing(usage))
    } else {
        Ok(rest)
    }
}

/// The first word, and the rest of the text with its spaces trimmed.
fn split_first(text: &str) -> (&str, &str) {
    match text.split_once(char::is_whitespace) {
        Some((word, tail)) => (word, tail.trim()),
        None => (text, ""),
    }
}

impl Command {
    /// The route this command calls. `/start` and `/help` call none.
    pub fn action(&self) -> Option<Action> {
        match self {
            Command::Start | Command::Help => None,
            Command::Status => Some(Action::Status),
            Command::Pause => Some(Action::Pause),
            Command::Resume => Some(Action::Resume),
            Command::Stop => Some(Action::Stop),
            Command::Cap(_) => Some(Action::LowerCap),
            Command::Kill(_) => Some(Action::Kill),
            Command::Agent { .. } => Some(Action::AgentMessage),
            Command::Diff(_) => Some(Action::DiffSummary),
            Command::Task(_) => Some(Action::AddTask),
            Command::Run(_) => Some(Action::StartRun),
            Command::Report => Some(Action::Report),
            Command::Daily => Some(Action::Daily),
            Command::Brain(_) => Some(Action::BrainSearch),
        }
    }
}

/// The commands a chat may use, with their usage and a short description.
const HELP_ROWS: [(Action, &str, &str); 13] = [
    (Action::Status, "/status", "Show the run"),
    (Action::Pause, "/pause", "Pause the run"),
    (Action::Resume, "/resume", "Resume the run"),
    (Action::Stop, "/stop", "Stop the run"),
    (Action::LowerCap, "/cap <amount>", "Lower the spend cap"),
    (Action::Kill, "/kill <agent>", "Stop one agent"),
    (
        Action::AgentMessage,
        "/agent <name> <text>",
        "Message one agent",
    ),
    (Action::DiffSummary, "/diff [task]", "Short diff summary"),
    (Action::AddTask, "/task <text>", "Add a task to the backlog"),
    (Action::StartRun, "/run [task]", "Start a run"),
    (Action::Report, "/report", "Run summary"),
    (Action::Daily, "/daily", "Daily summary"),
    (Action::BrainSearch, "/brain <search>", "Search note titles"),
];

pub fn help_text(levels: &BTreeSet<Level>) -> String {
    let mut lines = vec!["/help  Show this list".to_owned()];
    for (action, usage, about) in HELP_ROWS {
        if allowed(levels, action) {
            lines.push(format!("{usage}  {about}"));
        }
    }
    lines.join("\n")
}
