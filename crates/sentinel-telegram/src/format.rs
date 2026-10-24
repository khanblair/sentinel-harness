//! Turns Sentinel events into chat messages (`docs/product-scope.md`, F20). The renderer has no
//! input for file contents, keys, environment values, or Brain note text, so it cannot send them.

mod wave;

use sentinel_core::{Approval, ConnectionState, Decision, Event, HarnessState, TaskId, TaskStatus};

use crate::buttons::ButtonAction;
use crate::levels::{Action, ChatSettings, Detail, EventKind, Level, allowed};
pub use crate::quiet::Priority;
use crate::redact::redact;
pub use wave::{WaveLine, WaveSummary, WaveTracker, render_wave};

/// Telegram takes 4096 characters. The text stops here, with some room to spare.
const MAX_TEXT_CHARS: usize = 4000;

pub struct RenderContext<'a> {
    pub settings: &'a ChatSettings,
    pub project: &'a str,
    pub hackathon: bool,
    /// A ready line such as `Cost $0.88 of $2.00`. Shown at `Summary` and above.
    pub cost_line: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub text: String,
    pub buttons: Vec<Vec<ButtonSpec>>,
    pub priority: Priority,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonSpec {
    pub label: String,
    /// The one-time token is added later, by `buttons.rs`.
    pub action: ButtonAction,
}

/// The message for one event, or `None` when the chat should not get it.
pub fn render(event: &Event, ctx: &RenderContext<'_>) -> Option<Rendered> {
    if !ctx.settings.levels.contains(&Level::Notify) {
        return None;
    }
    let covered = classify(event)?;
    if let Some(kind) = covered.kind()
        && !ctx.settings.events.contains(&kind)
    {
        return None;
    }
    let card = card_for(&covered, ctx);
    Some(finish(card, ctx))
}

/// Events the chat can switch on, with the fields the message needs.
enum Covered<'a> {
    Approval(&'a Approval),
    Decision(&'a Decision),
    Budget { kind: EventKind, percent: u8 },
    TaskBlocked(&'a TaskId),
    RunFinished { verified: u32, total: u32 },
    Proposal,
    CareFinished,
    HostBack { minutes: u64 },
    Crashed,
    ConnectionFailed(&'a str),
}

impl Covered<'_> {
    /// The switch that turns this event on. Host messages have none and always go out.
    fn kind(&self) -> Option<EventKind> {
        match self {
            Covered::Approval(_) => Some(EventKind::ApprovalNeeded),
            Covered::Decision(_) => Some(EventKind::DecisionNeeded),
            Covered::Budget { kind, .. } => Some(*kind),
            Covered::TaskBlocked(_) => Some(EventKind::TaskBlocked),
            Covered::RunFinished { .. } => Some(EventKind::RunFinished),
            Covered::Proposal | Covered::CareFinished => Some(EventKind::BrainProposals),
            Covered::Crashed | Covered::ConnectionFailed(_) => Some(EventKind::Errors),
            Covered::HostBack { .. } => None,
        }
    }
}

fn classify(event: &Event) -> Option<Covered<'_>> {
    if let Event::ApprovalRequested { approval } = event {
        return Some(Covered::Approval(approval));
    }
    if let Event::DecisionRequested { decision } = event {
        return Some(Covered::Decision(decision));
    }
    if let Event::BudgetThreshold { percent } = event {
        return budget(*percent);
    }
    if let Event::TaskStatus { task, status } = event {
        return (*status == TaskStatus::Blocked).then_some(Covered::TaskBlocked(task));
    }
    classify_run_and_host(event)
}

fn classify_run_and_host(event: &Event) -> Option<Covered<'_>> {
    if let Event::RunFinished { verified, total } = event {
        return Some(Covered::RunFinished {
            verified: *verified,
            total: *total,
        });
    }
    if let Event::ProposalCreated { .. } = event {
        return Some(Covered::Proposal);
    }
    if let Event::CareFinished { .. } = event {
        return Some(Covered::CareFinished);
    }
    if let Event::HostBackOnline { gap_seconds } = event {
        return Some(Covered::HostBack {
            minutes: gap_seconds / 60,
        });
    }
    if let Event::HarnessState { state } = event {
        return (*state == HarnessState::Crashed).then_some(Covered::Crashed);
    }
    if let Event::ConnectionChanged { name, state } = event {
        let failed = matches!(state, ConnectionState::Failed);
        return failed.then_some(Covered::ConnectionFailed(name));
    }
    None
}

/// Only 70 and 95 percent are alerts. Other thresholds are not sent.
fn budget(percent: u8) -> Option<Covered<'static>> {
    match percent {
        70 => Some(Covered::Budget {
            kind: EventKind::Budget70,
            percent,
        }),
        95 => Some(Covered::Budget {
            kind: EventKind::Budget95,
            percent,
        }),
        _ => None,
    }
}

fn card_for(covered: &Covered<'_>, ctx: &RenderContext<'_>) -> Card {
    match covered {
        Covered::Approval(approval) => approval_card(approval, ctx),
        Covered::Decision(decision) => decision_card(decision, ctx),
        Covered::Budget { percent, .. } => budget_card(*percent, ctx),
        Covered::TaskBlocked(task) => Card::new(vec![format!("{task} blocked.")], Priority::Normal),
        Covered::RunFinished { verified, total } => run_card(*verified, *total, ctx),
        Covered::Proposal => Card::new(
            vec!["Brain has a new proposal.".to_owned()],
            Priority::Normal,
        ),
        Covered::CareFinished => {
            Card::new(vec!["Brain care finished.".to_owned()], Priority::Normal)
        }
        Covered::HostBack { minutes } => {
            let line = format!("Sentinel is back online after {minutes} minutes.");
            Card::new(vec![line], Priority::Normal).without_header()
        }
        Covered::Crashed => Card::new(
            vec!["Sentinel crashed. Check the PC.".to_owned()],
            Priority::Normal,
        ),
        Covered::ConnectionFailed(name) => {
            Card::new(vec![format!("{name} connection failed.")], Priority::Normal)
        }
    }
}

fn approval_card(approval: &Approval, ctx: &RenderContext<'_>) -> Card {
    let task = approval
        .task
        .as_ref()
        .map_or_else(|| "A task".to_owned(), ToString::to_string);
    let line = match (detail_of(ctx), approval.path.as_deref()) {
        (Detail::Status, _) | (_, None) => format!("{task} needs approval."),
        (_, Some(path)) => format!("{task} wants to write {path}. Nobody has reserved it."),
    };
    let mut card = Card::new(vec![line], Priority::Urgent);
    if allowed(&ctx.settings.levels, Action::Approve) {
        let id = &approval.id;
        card.push_row(vec![
            button(
                "Allow",
                ButtonAction::Approval {
                    approval: id.clone(),
                    allow: true,
                },
            ),
            button(
                "Deny",
                ButtonAction::Approval {
                    approval: id.clone(),
                    allow: false,
                },
            ),
        ]);
    }
    card
}

fn decision_card(decision: &Decision, ctx: &RenderContext<'_>) -> Card {
    let tasks: Vec<String> = decision.tasks.iter().map(ToString::to_string).collect();
    let line = format!("Decision needed for {}.", tasks.join(", "));
    let mut card = Card::new(vec![line], Priority::Normal);
    if allowed(&ctx.settings.levels, Action::Decide) {
        let mut row: Vec<ButtonSpec> = tasks
            .iter()
            .map(|task| {
                let action = ButtonAction::Decision {
                    decision: decision.id.clone(),
                    answer: task.clone(),
                };
                button(task.clone(), action)
            })
            .collect();
        let skip = ButtonAction::Decision {
            decision: decision.id.clone(),
            answer: "skip_both".to_owned(),
        };
        row.push(button("Skip both", skip));
        card.push_row(row);
    }
    card
}

fn budget_card(percent: u8, ctx: &RenderContext<'_>) -> Card {
    let mut lines = vec![format!("Budget at {percent}%.")];
    lines.extend(cost_lines(ctx));
    let mut card = Card::new(lines, Priority::Urgent);
    if allowed(&ctx.settings.levels, Action::Pause) {
        card.push_row(vec![button("Pause", ButtonAction::Pause)]);
    }
    card
}

fn run_card(verified: u32, total: u32, ctx: &RenderContext<'_>) -> Card {
    let mut lines = vec![format!("Run finished. {verified} of {total} verified.")];
    lines.extend(cost_lines(ctx));
    let mut card = Card::new(lines, Priority::Normal);
    if allowed(&ctx.settings.levels, Action::Report) {
        card.push_row(vec![button("Report", ButtonAction::Report)]);
    }
    card
}

/// A hackathon chat gets status only, whatever the setting says.
fn detail_of(ctx: &RenderContext<'_>) -> Detail {
    if ctx.hackathon {
        Detail::Status
    } else {
        ctx.settings.detail
    }
}

fn cost_lines(ctx: &RenderContext<'_>) -> Vec<String> {
    if detail_of(ctx) < Detail::Summary {
        return Vec::new();
    }
    ctx.cost_line
        .map_or_else(Vec::new, |line| vec![line.to_owned()])
}

fn button(label: impl Into<String>, action: ButtonAction) -> ButtonSpec {
    ButtonSpec {
        label: label.into(),
        action,
    }
}

/// The lines of one message, with its buttons and priority.
struct Card {
    header: bool,
    lines: Vec<String>,
    rows: Vec<Vec<ButtonSpec>>,
    priority: Priority,
}

impl Card {
    fn new(lines: Vec<String>, priority: Priority) -> Self {
        Self {
            header: true,
            lines,
            rows: Vec::new(),
            priority,
        }
    }

    fn without_header(mut self) -> Self {
        self.header = false;
        self
    }

    fn push_row(&mut self, row: Vec<ButtonSpec>) {
        if !row.is_empty() {
            self.rows.push(row);
        }
    }
}

fn finish(card: Card, ctx: &RenderContext<'_>) -> Rendered {
    let mut lines = card.lines;
    if card.header {
        lines.insert(0, format!("Sentinel, {}", ctx.project));
    }
    let text = cut(&redact(&lines.join("\n")));
    Rendered {
        text,
        buttons: card.rows,
        priority: card.priority,
    }
}

/// Cuts at a line end when one exists, and marks the cut with `...`.
fn cut(text: &str) -> String {
    if text.chars().count() <= MAX_TEXT_CHARS {
        return text.to_owned();
    }
    let keep = MAX_TEXT_CHARS - 3;
    let limit = text
        .char_indices()
        .nth(keep)
        .map_or(text.len(), |(index, _)| index);
    let cut_at = text[..limit].rfind('\n').unwrap_or(limit);
    format!("{}...", &text[..cut_at])
}
