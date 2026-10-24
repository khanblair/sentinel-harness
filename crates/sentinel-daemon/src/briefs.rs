//! The facts a brief reports, and the plain words it is written in. The text builders take counts,
//! task ids, reasons, and titles. Code, files, keys, env values, and Brain note text never reach
//! them, so they cannot be sent.

use sentinel_core::routines::RoutineKind;
use sentinel_core::{
    ApprovalState, BrainMode, Cost, DecisionState, TailscaleState, TaskId, TaskStatus, ToolStatus,
};
use sentinel_verify::report::TaskReport;
use time::{Duration, OffsetDateTime, Time, UtcOffset};

use crate::engine::{Engine, EngineError, Result, lock};
use crate::telegram::utc_offset_minutes;

/// Tasks named in a brief. More than this is a list, not a brief.
const STUCK_LIMIT: usize = 5;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TaskCounts {
    pub verified: u32,
    pub partial: u32,
    pub blocked: u32,
    pub skipped: u32,
    pub open: u32,
    pub waiting: u32,
}

/// A task that needs a look. The title is empty in hackathon mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StuckTask {
    pub id: TaskId,
    pub title: String,
    pub reason: String,
    pub blocked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spend {
    pub today: Cost,
    pub week: Cost,
    pub cap: Cost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backlog {
    pub name: String,
    /// Count of `## ` headings in the backlog file.
    pub tasks: u32,
}

/// What a brief says. Built from the store and the host. It holds no free text from a user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefFacts {
    pub counts: TaskCounts,
    /// Blocked tasks first, then partial ones.
    pub stuck: Vec<StuckTask>,
    pub spend: Spend,
    pub approvals_pending: u32,
    pub decisions_pending: u32,
    pub proposals_open: Option<u32>,
    pub agents_ready: u32,
    pub agents_total: u32,
    pub tailnet_up: bool,
    pub run_live: bool,
    pub backlog: Option<Backlog>,
}

impl Engine {
    /// The title and the text of a morning brief or a sundown wrap-up.
    pub fn build_brief(&self, kind: RoutineKind) -> Result<(String, String)> {
        let hackathon = self.config().hackathon_mode;
        let text = match kind {
            RoutineKind::Morning => morning_text(&self.brief_facts(hackathon)?, hackathon),
            RoutineKind::Sundown => sundown_text(&self.brief_facts(hackathon)?, hackathon),
            RoutineKind::Reminder | RoutineKind::BacklogRun => {
                return Err(EngineError::Invalid(
                    "Only the morning brief and the sundown wrap-up are built here.".to_owned(),
                ));
            }
        };
        Ok((kind.title().to_owned(), text))
    }

    fn brief_facts(&self, hackathon: bool) -> Result<BriefFacts> {
        let report = self.build_report()?;
        let config = self.config();
        let (approvals_pending, decisions_pending) = self.waiting_counts()?;
        let (agents_ready, agents_total) = self.agent_counts()?;
        let (today, week) = self.spend_today_and_week()?;
        Ok(BriefFacts {
            counts: task_counts(&report.tasks),
            stuck: stuck_tasks(&report.tasks, hackathon),
            spend: Spend {
                today,
                week,
                cap: self.cap(&config),
            },
            approvals_pending,
            decisions_pending,
            proposals_open: self.proposals_open(),
            agents_ready,
            agents_total,
            tailnet_up: *lock(&self.inner.tailscale) == TailscaleState::Running,
            run_live: self.is_run_live(),
            backlog: self.backlog_facts(),
        })
    }

    fn waiting_counts(&self) -> Result<(u32, u32)> {
        let approvals = self.store().list_approvals(&[ApprovalState::Pending])?;
        let decisions = self.store().list_decisions(&[DecisionState::Pending])?;
        Ok((count(approvals.len()), count(decisions.len())))
    }

    fn agent_counts(&self) -> Result<(u32, u32)> {
        let tools = self.store().list_tools()?;
        let ready = tools
            .iter()
            .filter(|tool| tool.enabled && tool.status == ToolStatus::Ready)
            .count();
        Ok((count(ready), count(tools.len())))
    }

    /// Spend since local midnight and since local Monday, in micro-dollars.
    fn spend_today_and_week(&self) -> Result<(Cost, Cost)> {
        let local = OffsetDateTime::now_utc().to_offset(local_offset());
        let monday = local - Duration::days(i64::from(local.weekday().number_days_from_monday()));
        let today = self.store().cost_since(local_midnight_utc(local))?;
        let week = self.store().cost_since(local_midnight_utc(monday))?;
        Ok((today, week))
    }

    /// Open brain proposals, when Brain is on. A failed read shows nothing, not an error.
    fn proposals_open(&self) -> Option<u32> {
        if self.config().brain_mode == BrainMode::Off {
            return None;
        }
        self.brain_proposals()
            .ok()
            .map(|proposals| count(proposals.len()))
    }

    /// The configured backlog and its heading count. A missing file shows nothing.
    fn backlog_facts(&self) -> Option<Backlog> {
        let name = self.config().backlog;
        let text = std::fs::read_to_string(self.repo_root().join(&name)).ok()?;
        let tasks = text.lines().filter(|line| line.starts_with("## ")).count();
        Some(Backlog {
            name,
            tasks: count(tasks),
        })
    }
}

/// The offset this computer is in. The owner's local clock decides day and week boundaries.
pub(crate) fn local_offset() -> UtcOffset {
    UtcOffset::from_whole_seconds(utc_offset_minutes() * 60).unwrap_or(UtcOffset::UTC)
}

/// Local midnight of the day of `local`, in UTC.
pub(crate) fn local_midnight_utc(local: OffsetDateTime) -> OffsetDateTime {
    local.replace_time(Time::MIDNIGHT).to_offset(UtcOffset::UTC)
}

fn count(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}

fn task_counts(tasks: &[TaskReport]) -> TaskCounts {
    let mut counts = TaskCounts::default();
    for task in tasks {
        count_status(&mut counts, task.status);
    }
    counts
}

fn count_status(counts: &mut TaskCounts, status: TaskStatus) {
    match status {
        TaskStatus::Verified => counts.verified += 1,
        TaskStatus::Partial => counts.partial += 1,
        TaskStatus::Blocked => counts.blocked += 1,
        TaskStatus::Skipped => counts.skipped += 1,
        TaskStatus::Waiting | TaskStatus::Contradiction => counts.waiting += 1,
        TaskStatus::Planned
        | TaskStatus::Ready
        | TaskStatus::Reserved
        | TaskStatus::Running
        | TaskStatus::Checkpointed
        | TaskStatus::Verifying
        | TaskStatus::Failed => counts.open += 1,
    }
}

fn stuck_tasks(tasks: &[TaskReport], hackathon: bool) -> Vec<StuckTask> {
    let mut stuck: Vec<StuckTask> = tasks
        .iter()
        .filter(|task| matches!(task.status, TaskStatus::Blocked | TaskStatus::Partial))
        .take(STUCK_LIMIT)
        .map(|task| StuckTask {
            id: task.id.clone(),
            title: if hackathon {
                String::new()
            } else {
                task.title.clone()
            },
            reason: task.reason.clone(),
            blocked: task.status == TaskStatus::Blocked,
        })
        .collect();
    stuck.sort_by_key(|task| !task.blocked);
    stuck
}

/// The morning brief: what waits, what is left, and where to start.
pub fn morning_text(facts: &BriefFacts, hackathon: bool) -> String {
    let mut lines = vec!["Good morning.".to_owned(), waiting_line(facts)];
    if let Some(open) = facts.proposals_open {
        lines.push(format!("Brain proposals open: {open}."));
    }
    lines.push(format!(
        "Spend this week: {} of {}.",
        facts.spend.week, facts.spend.cap
    ));
    lines.push(format!(
        "Agents ready: {} of {}.",
        facts.agents_ready, facts.agents_total
    ));
    lines.push(tailnet_line(facts));
    lines.extend(backlog_line(facts));
    lines.push(format!("Start with: {}.", next_step(facts, hackathon)));
    lines.join("\n")
}

/// The sundown wrap-up: how the day went, what is stuck, and what comes next.
pub fn sundown_text(facts: &BriefFacts, hackathon: bool) -> String {
    let counts = facts.counts;
    let mut lines = vec![
        "Day wrap-up.".to_owned(),
        format!(
            "Tasks: {} verified, {} partial, {} blocked, {} skipped.",
            counts.verified, counts.partial, counts.blocked, counts.skipped
        ),
        format!("Spent today {}.", facts.spend.today),
    ];
    lines.extend(
        facts
            .stuck
            .iter()
            .map(|task| format!("Stuck: {}.", task_phrase(task, hackathon))),
    );
    lines.push(waiting_line(facts));
    lines.push(format!(
        "Tomorrow: start with {}.",
        next_step(facts, hackathon)
    ));
    lines.push(closing_line(facts));
    lines.join("\n")
}

/// The first blocked task, else the backlog. Partial tasks wait until nothing is blocked.
fn next_step(facts: &BriefFacts, hackathon: bool) -> String {
    match facts.stuck.iter().find(|task| task.blocked) {
        Some(task) => task_phrase(task, hackathon),
        None => "a run from the backlog".to_owned(),
    }
}

fn task_phrase(task: &StuckTask, hackathon: bool) -> String {
    if hackathon || task.title.is_empty() {
        format!("{}: {}", task.id, task.reason)
    } else {
        format!("{} {}: {}", task.id, task.title, task.reason)
    }
}

fn waiting_line(facts: &BriefFacts) -> String {
    if facts.approvals_pending + facts.decisions_pending == 0 {
        return "Nothing is waiting for you.".to_owned();
    }
    format!(
        "Waiting for you: {}, {}.",
        plural(facts.approvals_pending, "approval"),
        plural(facts.decisions_pending, "decision")
    )
}

fn plural(number: u32, word: &str) -> String {
    if number == 1 {
        format!("1 {word}")
    } else {
        format!("{number} {word}s")
    }
}

fn tailnet_line(facts: &BriefFacts) -> String {
    let state = if facts.tailnet_up { "up" } else { "down" };
    format!("Tailscale: {state}.")
}

fn backlog_line(facts: &BriefFacts) -> Option<String> {
    let backlog = facts.backlog.as_ref()?;
    Some(format!(
        "Backlog {}: {} tasks.",
        backlog.name, backlog.tasks
    ))
}

/// One honest line. It never says more than the counts do.
fn closing_line(facts: &BriefFacts) -> String {
    let counts = facts.counts;
    let total = counts.verified
        + counts.partial
        + counts.blocked
        + counts.skipped
        + counts.open
        + counts.waiting;
    if total == 0 {
        return "Nothing is on the board.".to_owned();
    }
    let needs_look = counts.blocked + counts.partial;
    if needs_look == 0 {
        return "No task is blocked or partial.".to_owned();
    }
    format!("Tasks needing a look: {needs_look}.")
}

#[cfg(test)]
mod tests {
    use sentinel_core::{Cost, TaskId};

    use super::{BriefFacts, Spend, StuckTask, TaskCounts, morning_text, sundown_text};

    fn facts() -> BriefFacts {
        BriefFacts {
            counts: TaskCounts {
                verified: 2,
                blocked: 1,
                ..TaskCounts::default()
            },
            stuck: vec![StuckTask {
                id: TaskId::new("T-04"),
                title: "Secret feature".to_owned(),
                reason: "Tests failed twice.".to_owned(),
                blocked: true,
            }],
            spend: Spend {
                today: Cost::parse_usd("0.88").unwrap(),
                week: Cost::parse_usd("3.10").unwrap(),
                cap: Cost::parse_usd("20.00").unwrap(),
            },
            approvals_pending: 1,
            decisions_pending: 0,
            proposals_open: Some(2),
            agents_ready: 1,
            agents_total: 3,
            tailnet_up: true,
            run_live: false,
            backlog: None,
        }
    }

    #[test]
    fn the_morning_brief_starts_with_the_first_blocked_task() {
        let text = morning_text(&facts(), false);
        assert!(text.starts_with("Good morning."));
        assert!(text.contains("Waiting for you: 1 approval, 0 decisions."));
        assert!(text.contains("Start with: T-04 Secret feature: Tests failed twice."));
    }

    #[test]
    fn hackathon_mode_leaves_task_titles_out_of_both_briefs() {
        assert!(!morning_text(&facts(), true).contains("Secret feature"));
        let sundown = sundown_text(&facts(), true);
        assert!(!sundown.contains("Secret feature"));
        assert!(sundown.contains("T-04: Tests failed twice."));
    }

    #[test]
    fn the_sundown_wrap_up_counts_tasks_and_spend() {
        let text = sundown_text(&facts(), false);
        assert!(text.contains("Tasks: 2 verified, 0 partial, 1 blocked, 0 skipped."));
        assert!(text.contains("Spent today $0.88."));
        assert!(text.ends_with("Tasks needing a look: 1."));
    }

    #[test]
    fn a_brief_has_at_most_twelve_lines() {
        assert!(morning_text(&facts(), false).lines().count() <= 12);
        assert!(sundown_text(&facts(), false).lines().count() <= 12);
    }
}
