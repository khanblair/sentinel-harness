//! The router turns one chat command or one button press into one Engine call, made as the phone
//! caller. It checks the chat's levels first. Nothing here can raise the cap, change the permission
//! mode, edit notes or tests, or run a shell: no branch calls those routes.

use sentinel_core::api::RunRequest;
use sentinel_core::{Event, PermissionMode, TaskId, ToolId};
use sentinel_telegram::Button;
use sentinel_telegram::buttons::{ButtonAction, ButtonError};
use sentinel_telegram::command::{self, Command, ParseProblem, help_text};
use sentinel_telegram::levels::{allowed, refusal_text};
use sentinel_telegram::redact::redact;

use crate::controls::{Caller, usd_text};
use crate::engine::{Engine, Result};
use crate::telegram::{CALLER_NAME, Shared, unix_now};
use crate::telegram_poll::log_received;
use crate::telegram_store::{self, ChatRow};

/// Telegram takes 4096 characters. A report is cut well before that.
const REPORT_CHARS: usize = 3000;
const BRAIN_LIMIT: usize = 10;
const NOT_PAIRED: &str = "Send /start to connect this chat first.";
const BYPASS_OFF: &str =
    "Runs from Telegram are off in Bypass permissions. Change the mode at the computer.";
const NOT_YET: &str = "That is not available from Telegram yet.";

/// A button press as Telegram sends it: the callback id to answer, and the one-time token.
pub(crate) struct Press<'a> {
    pub callback_id: &'a str,
    pub token: &'a str,
}

/// What the bot says back. A keyboard is optional.
#[derive(Debug, Default)]
pub(crate) struct Reply {
    pub text: String,
    pub keyboard: Vec<Vec<Button>>,
}

impl Reply {
    fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            keyboard: Vec::new(),
        }
    }
}

/// One chat turn: the engine, the shared tasks state, and the chat row it speaks for.
struct Turn<'a> {
    engine: &'a Engine,
    shared: &'a Shared,
    row: &'a ChatRow,
}

pub(crate) async fn on_text(engine: &Engine, shared: &Shared, row: &ChatRow, text: &str) {
    log_received(engine, true, receipt_name(text));
    let turn = Turn {
        engine,
        shared,
        row,
    };
    let reply = match command::parse(text) {
        Ok(command) => turn.command(command),
        Err(problem) => Reply::text(problem_text(&problem)),
    };
    send(shared, row.user_id, reply).await;
}

pub(crate) async fn on_press(engine: &Engine, shared: &Shared, row: &ChatRow, press: Press<'_>) {
    let _ = shared.client.answer_callback(press.callback_id, "").await;
    let now = unix_now(engine);
    let redeemed = crate::engine::lock(&shared.tokens).redeem(press.token, row.user_id, now);
    let turn = Turn {
        engine,
        shared,
        row,
    };
    let reply = match redeemed {
        Err(error) => Reply::text(button_error_text(&error)),
        Ok(action) => {
            log_received(engine, true, button_name(&action));
            turn.press(action).await
        }
    };
    send(shared, row.user_id, reply).await;
}

/// The name of a command or button, for the log. It never holds the message body or a token.
pub(crate) fn receipt_name(text: &str) -> &'static str {
    if !text.trim_start().starts_with('/') {
        return "(text)";
    }
    match command::parse(text) {
        Ok(command) => command_name(&command),
        Err(_) => "(unknown)",
    }
}

fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Start => "/start",
        Command::Help => "/help",
        Command::Status => "/status",
        Command::Pause => "/pause",
        Command::Resume => "/resume",
        Command::Stop => "/stop",
        Command::Cap(_) => "/cap",
        Command::Kill(_) => "/kill",
        Command::Agent { .. } => "/agent",
        Command::Diff(_) => "/diff",
        Command::Task(_) => "/task",
        Command::Run(_) => "/run",
        Command::Report => "/report",
        Command::Daily => "/daily",
        Command::Brain(_) => "/brain",
    }
}

fn button_name(action: &ButtonAction) -> &'static str {
    match action {
        ButtonAction::Approval { .. } => "button:approval",
        ButtonAction::Decision { .. } => "button:decision",
        ButtonAction::Pause => "button:pause",
        ButtonAction::Resume => "button:resume",
        ButtonAction::Stop => "button:stop",
        ButtonAction::Report => "button:report",
        ButtonAction::DiffSummary { .. } => "button:diff",
        ButtonAction::StartRun { .. } => "button:run",
        ButtonAction::AddTask { .. } => "button:task",
        ButtonAction::Kill { .. } => "button:kill",
    }
}

fn problem_text(problem: &ParseProblem) -> String {
    match problem {
        ParseProblem::Unknown(_) => "Not a command. Send /help for the list.".to_owned(),
        ParseProblem::Missing(usage) => (*usage).to_owned(),
        ParseProblem::BadCap(_) => "Use a dollar amount, such as /cap 1.50.".to_owned(),
    }
}

fn button_error_text(error: &ButtonError) -> String {
    format!("That button did not work: {error}.")
}

fn outcome<T>(result: Result<T>, done: &str) -> Reply {
    match result {
        Ok(_) => Reply::text(done),
        Err(error) => Reply::text(error.to_string()),
    }
}

fn phone_caller() -> Caller {
    Caller::phone(CALLER_NAME)
}

impl Turn<'_> {
    fn is_paired(&self) -> bool {
        telegram_store::is_paired(self.engine).unwrap_or(false)
    }

    fn bypass(&self) -> bool {
        self.engine.config().permission_mode == PermissionMode::Bypass
    }

    fn command(&self, command: Command) -> Reply {
        if matches!(command, Command::Start) {
            return self.pair();
        }
        let Some(action) = command.action() else {
            return Reply::text(help_text(&self.row.settings.levels));
        };
        if !self.is_paired() {
            return Reply::text(NOT_PAIRED);
        }
        if !allowed(&self.row.settings.levels, action) {
            let name = command_name(&command);
            return Reply::text(refusal_text(&format!("{name} is not on for this chat")));
        }
        self.run(command)
    }

    /// `/start` pairs the chat. The reply lists what the chat may do.
    fn pair(&self) -> Reply {
        if telegram_store::set_paired(self.engine, true).is_ok() {
            self.engine.emit_quiet(&Event::TelegramPaired {
                user_id: self.row.user_id,
            });
        }
        Reply::text(help_text(&self.row.settings.levels))
    }

    /// Runs a command the chat may use. Each arm calls one route, or sets up a confirm button.
    fn run(&self, command: Command) -> Reply {
        let caller = phone_caller();
        match command {
            Command::Start => self.pair(),
            Command::Help => Reply::text(help_text(&self.row.settings.levels)),
            Command::Status => Reply::text(status_text(self.engine)),
            Command::Pause => outcome(self.engine.pause_run(), "Paused."),
            Command::Resume => outcome(self.engine.resume_run(), "Resumed."),
            Command::Stop => self.confirm("Stop the run?", "Confirm stop", &ButtonAction::Stop),
            Command::Kill(agent) => self.kill_confirm(agent),
            Command::Cap(cost) => self.lower_cap(&usd_text(cost), &caller),
            Command::Agent { tool, text } => self.message_agent(&tool, &text, &caller),
            Command::Diff(_) | Command::Daily | Command::Task(_) => Reply::text(NOT_YET),
            Command::Run(task) => self.run_confirm(task),
            Command::Report => self.report(),
            Command::Brain(search) => self.brain(&search),
        }
    }

    async fn press(&self, action: ButtonAction) -> Reply {
        if !allowed(&self.row.settings.levels, action.action()) {
            return Reply::text(refusal_text("That button is not on for this chat"));
        }
        let caller = phone_caller();
        match action {
            ButtonAction::Approval { approval, allow } => {
                let done = if allow { "Allowed." } else { "Denied." };
                outcome(self.engine.answer_approval(&approval, allow, &caller), done)
            }
            ButtonAction::Decision { decision, answer } => outcome(
                self.engine.answer_decision(&decision, &answer, &caller),
                "Answered.",
            ),
            ButtonAction::Pause => outcome(self.engine.pause_run(), "Paused."),
            ButtonAction::Resume => outcome(self.engine.resume_run(), "Resumed."),
            ButtonAction::Stop => outcome(self.engine.stop_run(), "Stopping the run."),
            ButtonAction::Kill { agent } => {
                outcome(self.engine.kill_agent(&agent), "Stopping the agent.")
            }
            ButtonAction::Report => self.report(),
            ButtonAction::StartRun { task } => self.start_run(task).await,
            ButtonAction::DiffSummary { .. } | ButtonAction::AddTask { .. } => Reply::text(NOT_YET),
        }
    }

    fn confirm(&self, text: &str, label: &str, action: &ButtonAction) -> Reply {
        let now = unix_now(self.engine);
        let token = crate::engine::lock(&self.shared.tokens).issue(self.row.user_id, action, now);
        Reply {
            text: text.to_owned(),
            keyboard: vec![vec![Button {
                label: label.to_owned(),
                data: token,
            }]],
        }
    }

    fn kill_confirm(&self, agent: ToolId) -> Reply {
        let text = format!("Stop {agent}?");
        self.confirm(&text, "Confirm kill", &ButtonAction::Kill { agent })
    }

    fn run_confirm(&self, task: Option<TaskId>) -> Reply {
        if self.bypass() {
            return Reply::text(BYPASS_OFF);
        }
        let label = task
            .as_ref()
            .map_or_else(|| "the whole backlog".to_owned(), ToString::to_string);
        let text = format!("Start a run of {label}?");
        self.confirm(&text, "Confirm run", &ButtonAction::StartRun { task })
    }

    async fn start_run(&self, task: Option<TaskId>) -> Reply {
        if self.bypass() {
            return Reply::text(BYPASS_OFF);
        }
        let request = RunRequest {
            task,
            ..RunRequest::default()
        };
        let started = self.engine.start_run_as(request, &phone_caller()).await;
        outcome(started, "Run started.")
    }

    fn lower_cap(&self, amount: &str, caller: &Caller) -> Reply {
        match self.engine.lower_cap(amount, caller) {
            Ok(cap) => Reply::text(format!("The cap is now {cap}.")),
            Err(error) => Reply::text(error.to_string()),
        }
    }

    fn message_agent(&self, tool: &ToolId, text: &str, caller: &Caller) -> Reply {
        match self.engine.send_message(text, Some(tool.clone()), caller) {
            Ok(()) => Reply::text(format!("Sent to {tool}.")),
            Err(error) => Reply::text(error.to_string()),
        }
    }

    fn report(&self) -> Reply {
        match self.engine.report_markdown() {
            Ok(text) => {
                let cut: String = text.chars().take(REPORT_CHARS).collect();
                Reply::text(cut)
            }
            Err(_) => Reply::text("The report is not ready yet."),
        }
    }

    fn brain(&self, search: &str) -> Reply {
        match self.engine.brain_notes(Some(search), None) {
            Ok(list) => {
                let titles: Vec<&str> = list
                    .notes
                    .iter()
                    .take(BRAIN_LIMIT)
                    .map(|note| note.title.as_str())
                    .collect();
                if titles.is_empty() {
                    return Reply::text("No note titles match.");
                }
                Reply::text(titles.join("\n"))
            }
            Err(_) => Reply::text("Brain search is not available."),
        }
    }
}

fn status_text(engine: &Engine) -> String {
    let state = format!("{:?}", engine.harness_state()).to_lowercase();
    format!("Sentinel is {state}.")
}

async fn send(shared: &Shared, chat_id: i64, reply: Reply) {
    let keyboard = (!reply.keyboard.is_empty()).then_some(reply.keyboard.as_slice());
    let text = redact(&reply.text);
    if let Err(error) = shared.client.send_message(chat_id, &text, keyboard).await {
        eprintln!("sentinel: Telegram reply failed: {error}");
    }
}
