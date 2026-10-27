//! The evidence section of the report: what the store and the event log show about the agents,
//! the collisions, the spend, and each task's claim. The functions here are pure. The engine reads
//! the store into an `EvidenceInput` and passes it in.

use std::collections::{BTreeMap, BTreeSet};

use sentinel_core::{Cost, Event, EventRecord, TaskId, TaskStatus, ToolId};
use serde::{Deserialize, Serialize};

mod render;
mod timing;

pub use render::render_evidence;
use timing::{current_run, waves_for};

/// How many blocked paths the report names. The rest are only counted.
const LISTED_PATHS: usize = 10;

/// What the engine read from the store, as plain data.
#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceInput {
    pub tasks: Vec<TaskRow>,
    pub verifications: Vec<VerificationRow>,
    pub events: Vec<EventRecord>,
    pub total_cost: Cost,
    pub cap: Cost,
}

/// One task as the store has it now.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskRow {
    pub id: TaskId,
    pub status: TaskStatus,
    pub cost: Cost,
}

/// The four checks and the reason, as the verifier stored them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one flag for each of the four checks of the verdict"
)]
pub struct VerificationRow {
    pub task: TaskId,
    pub tests_pass: bool,
    pub new_test: bool,
    pub reviewer_ok: bool,
    pub tamper_clear: bool,
    pub reason: String,
}

/// The evidence for one run. Money is micro-dollars inside `Cost`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub agents: Vec<AgentEvidence>,
    pub waves: WaveEvidence,
    pub collisions: Collisions,
    pub cost: CostEvidence,
    pub claims: Vec<ClaimRow>,
    pub honesty: Honesty,
}

/// One agent's share of the run. Spend is the cost of the tasks it last started.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentEvidence {
    pub agent: ToolId,
    pub tasks_started: usize,
    pub tasks_verified: usize,
    pub spend: Cost,
    /// `reported` when the tool gave a dollar figure, `estimate` when Sentinel guessed one, and
    /// empty when nothing was spent.
    pub spend_source: String,
}

/// How the work was spread over waves and time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaveEvidence {
    pub count: usize,
    pub most_tasks: usize,
    /// Summed task run time divided by the wall time. 1.8 means 1.8 tasks ran at once on average.
    pub parallel_share: f64,
}

/// What the harness stopped from writing outside a task's reservation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Collisions {
    pub writes_blocked: usize,
    pub blocked_paths: Vec<String>,
    pub writes_restored: usize,
    pub tampers_blocked: usize,
}

/// The money side of the run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostEvidence {
    pub spent: Cost,
    pub cap: Cost,
    /// `None` when there is no cap to measure against.
    pub percent_of_cap: Option<u8>,
    pub tasks_verified: usize,
    /// `None` when no task was Verified.
    pub per_verified: Option<Cost>,
    pub highest_threshold: Option<u8>,
}

/// An agent's claim for one task, next to what the harness found. `None` in a check means no
/// check was stored, so it is not known. It is never read as false.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimRow {
    pub task: TaskId,
    pub finished: bool,
    pub tests_pass: Option<bool>,
    pub new_test: Option<bool>,
    pub reviewer_ok: Option<bool>,
    pub tamper_clear: Option<bool>,
    pub status: TaskStatus,
    pub reason: String,
}

/// Task counts by final status, and the claims the harness did not confirm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Honesty {
    pub verified: usize,
    pub partial: usize,
    pub blocked: usize,
    pub skipped: usize,
    pub failed: usize,
    /// Tasks that reached a checkpoint and were not Verified.
    pub claims_not_confirmed: usize,
}

/// Build the evidence from the store's rows. Only the last run in the event log counts.
pub fn build_evidence(input: &EvidenceInput) -> Evidence {
    let run = current_run(&input.events);
    let starts = run_starts(run);
    let last_agent: BTreeMap<TaskId, ToolId> = starts.iter().cloned().collect();
    let finished = finished_tasks(run);
    let honesty = honesty_for(&input.tasks, &finished);
    Evidence {
        agents: agents_for(input, &starts, &last_agent),
        waves: waves_for(run),
        collisions: collisions_for(run),
        cost: cost_for(input, run, honesty.verified),
        claims: claims_for(input, &finished),
        honesty,
    }
}

/// Each `TASK goes to AGENT` line, oldest first: the task and the agent it went to.
fn run_starts(run: &[EventRecord]) -> Vec<(TaskId, ToolId)> {
    run.iter()
        .filter_map(|record| start_line(&record.event))
        .collect()
}

fn start_line(event: &Event) -> Option<(TaskId, ToolId)> {
    let Event::Message {
        to: Some(agent),
        text,
        ..
    } = event
    else {
        return None;
    };
    let (task, _) = text.split_once(" goes to ")?;
    Some((TaskId::new(task), agent.clone()))
}

fn agents_for(
    input: &EvidenceInput,
    starts: &[(TaskId, ToolId)],
    last_agent: &BTreeMap<TaskId, ToolId>,
) -> Vec<AgentEvidence> {
    let mut rows: BTreeMap<ToolId, AgentEvidence> = BTreeMap::new();
    for (_, agent) in starts {
        rows.entry(agent.clone())
            .or_insert_with(|| empty_agent(agent))
            .tasks_started += 1;
    }
    for task in &input.tasks {
        let Some(agent) = last_agent.get(&task.id) else {
            continue;
        };
        let row = rows
            .entry(agent.clone())
            .or_insert_with(|| empty_agent(agent));
        row.spend = row.spend + task.cost;
        if task.status == TaskStatus::Verified {
            row.tasks_verified += 1;
        }
    }
    rows.into_values().map(with_spend_source).collect()
}

fn empty_agent(agent: &ToolId) -> AgentEvidence {
    AgentEvidence {
        agent: agent.clone(),
        tasks_started: 0,
        tasks_verified: 0,
        spend: Cost::ZERO,
        spend_source: String::new(),
    }
}

fn with_spend_source(mut row: AgentEvidence) -> AgentEvidence {
    spend_source(row.spend).clone_into(&mut row.spend_source);
    row
}

fn spend_source(spend: Cost) -> &'static str {
    if spend.micro_usd() == 0 {
        ""
    } else if spend.is_estimated() {
        "estimate"
    } else {
        "reported"
    }
}

fn collisions_for(run: &[EventRecord]) -> Collisions {
    let mut collisions = Collisions::default();
    for record in run {
        count_collision(&mut collisions, &record.event);
    }
    collisions
}

fn count_collision(collisions: &mut Collisions, event: &Event) {
    if let Event::WriteBlocked { path, restored, .. } = event {
        collisions.writes_blocked += 1;
        if *restored {
            collisions.writes_restored += 1;
        }
        if collisions.blocked_paths.len() < LISTED_PATHS {
            collisions.blocked_paths.push(path.clone());
        }
    } else if let Event::Verdict { reason, .. } = event
        && reason.to_lowercase().contains("tamper")
    {
        collisions.tampers_blocked += 1;
    }
}

fn cost_for(input: &EvidenceInput, run: &[EventRecord], verified: usize) -> CostEvidence {
    CostEvidence {
        spent: input.total_cost,
        cap: input.cap,
        percent_of_cap: percent_of_cap(input.total_cost, input.cap),
        tasks_verified: verified,
        per_verified: per_verified(input.total_cost, verified),
        highest_threshold: run.iter().filter_map(|r| threshold(&r.event)).max(),
    }
}

/// A zero cap leaves nothing to measure against, so there is no percent.
fn percent_of_cap(total: Cost, cap: Cost) -> Option<u8> {
    (cap.micro_usd() > 0).then(|| total.percent_of(cap))
}

fn per_verified(total: Cost, verified: usize) -> Option<Cost> {
    let count = u64::try_from(verified).ok().filter(|n| *n > 0)?;
    Some(Cost::from_micro_usd(
        total.micro_usd() / count,
        total.is_estimated(),
    ))
}

fn threshold(event: &Event) -> Option<u8> {
    let Event::BudgetThreshold { percent } = event else {
        return None;
    };
    Some(*percent)
}

fn finished_tasks(run: &[EventRecord]) -> BTreeSet<TaskId> {
    run.iter()
        .filter_map(|record| checkpoint_task(&record.event))
        .cloned()
        .collect()
}

fn checkpoint_task(event: &Event) -> Option<&TaskId> {
    let Event::Checkpoint { task, .. } = event else {
        return None;
    };
    Some(task)
}

fn claims_for(input: &EvidenceInput, finished: &BTreeSet<TaskId>) -> Vec<ClaimRow> {
    input
        .tasks
        .iter()
        .map(|task| {
            let checks = input.verifications.iter().find(|v| v.task == task.id);
            claim_for(task, checks, finished.contains(&task.id))
        })
        .collect()
}

fn claim_for(task: &TaskRow, checks: Option<&VerificationRow>, finished: bool) -> ClaimRow {
    ClaimRow {
        task: task.id.clone(),
        finished,
        tests_pass: checks.map(|c| c.tests_pass),
        new_test: checks.map(|c| c.new_test),
        reviewer_ok: checks.map(|c| c.reviewer_ok),
        tamper_clear: checks.map(|c| c.tamper_clear),
        status: task.status,
        reason: checks.map(|c| c.reason.clone()).unwrap_or_default(),
    }
}

fn honesty_for(tasks: &[TaskRow], finished: &BTreeSet<TaskId>) -> Honesty {
    let count = |status: TaskStatus| tasks.iter().filter(|t| t.status == status).count();
    Honesty {
        verified: count(TaskStatus::Verified),
        partial: count(TaskStatus::Partial),
        blocked: count(TaskStatus::Blocked),
        skipped: count(TaskStatus::Skipped),
        failed: count(TaskStatus::Failed),
        claims_not_confirmed: tasks
            .iter()
            .filter(|t| finished.contains(&t.id) && t.status != TaskStatus::Verified)
            .count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(seconds)
    }

    /// An event at `seconds` after the epoch. The id is the seconds, which the builder never reads.
    fn record(seconds: i64, event: Event) -> EventRecord {
        EventRecord {
            id: seconds,
            chat: None,
            at: at(seconds),
            event,
        }
    }

    fn goes_to(seconds: i64, task: &str, agent: &str) -> EventRecord {
        record(
            seconds,
            Event::Message {
                from: "sentinel".to_owned(),
                to: Some(ToolId::new(agent)),
                text: format!("{task} goes to {agent} (standard task)."),
            },
        )
    }

    fn task_status(seconds: i64, task: &str, status: TaskStatus) -> EventRecord {
        record(
            seconds,
            Event::TaskStatus {
                task: TaskId::new(task),
                status,
            },
        )
    }

    fn verdict(seconds: i64, task: &str, status: TaskStatus, reason: &str) -> EventRecord {
        record(
            seconds,
            Event::Verdict {
                task: TaskId::new(task),
                status,
                reason: reason.to_owned(),
            },
        )
    }

    fn checkpoint(seconds: i64, task: &str) -> EventRecord {
        record(
            seconds,
            Event::Checkpoint {
                task: TaskId::new(task),
                commit: "abc".to_owned(),
                paths: Vec::new(),
            },
        )
    }

    fn write_blocked(seconds: i64, path: &str, restored: bool) -> EventRecord {
        record(
            seconds,
            Event::WriteBlocked {
                task: None,
                path: path.to_owned(),
                restored,
            },
        )
    }

    /// Three tasks in one wave. T-1 is Verified, T-2 is Partial, and T-3 is Blocked by a tamper
    /// and never checkpointed. The run lasts 12 seconds. The task run times add up to 13.
    fn sample_input() -> EvidenceInput {
        let events = sample_events();
        EvidenceInput {
            tasks: vec![
                TaskRow {
                    id: TaskId::new("T-1"),
                    status: TaskStatus::Verified,
                    cost: Cost::from_micro_usd(100_000, false),
                },
                TaskRow {
                    id: TaskId::new("T-2"),
                    status: TaskStatus::Partial,
                    cost: Cost::from_micro_usd(50_000, false),
                },
                TaskRow {
                    id: TaskId::new("T-3"),
                    status: TaskStatus::Blocked,
                    cost: Cost::from_micro_usd(20_000, true),
                },
            ],
            verifications: sample_verifications(),
            events,
            total_cost: Cost::from_micro_usd(170_000, true),
            cap: Cost::from_micro_usd(2_000_000, false),
        }
    }

    fn sample_verifications() -> Vec<VerificationRow> {
        vec![
            VerificationRow {
                task: TaskId::new("T-1"),
                tests_pass: true,
                new_test: true,
                reviewer_ok: true,
                tamper_clear: true,
                reason: "All four checks passed.".to_owned(),
            },
            VerificationRow {
                task: TaskId::new("T-2"),
                tests_pass: true,
                new_test: false,
                reviewer_ok: true,
                tamper_clear: true,
                reason: "no new test added".to_owned(),
            },
        ]
    }

    fn sample_events() -> Vec<EventRecord> {
        vec![
            record(
                0,
                Event::RunStarted {
                    run: sentinel_core::RunId::new("run-1"),
                    backlog: "backlog.md".to_owned(),
                },
            ),
            goes_to(0, "T-1", "claude"),
            task_status(1, "T-1", TaskStatus::Running),
            goes_to(1, "T-2", "codex"),
            task_status(2, "T-2", TaskStatus::Running),
            record(
                1,
                Event::WaveStarted {
                    wave: 1,
                    tasks: vec![TaskId::new("T-1"), TaskId::new("T-2")],
                },
            ),
            checkpoint(5, "T-1"),
            verdict(6, "T-1", TaskStatus::Verified, "All four checks passed."),
            write_blocked(7, "src/x.rs", false),
            write_blocked(8, "src/y.rs", true),
            checkpoint(9, "T-2"),
            verdict(10, "T-2", TaskStatus::Partial, "no new test added"),
            record(11, Event::BudgetThreshold { percent: 70 }),
        ]
        .into_iter()
        .chain(sample_late_events())
        .collect()
    }

    fn sample_late_events() -> Vec<EventRecord> {
        vec![
            goes_to(10, "T-3", "claude"),
            task_status(10, "T-3", TaskStatus::Running),
            verdict(
                12,
                "T-3",
                TaskStatus::Blocked,
                "Tampered with a protected file: tests/run.sh",
            ),
            record(
                12,
                Event::RunFinished {
                    verified: 1,
                    total: 3,
                },
            ),
        ]
    }

    fn agent<'a>(evidence: &'a Evidence, name: &str) -> &'a AgentEvidence {
        evidence
            .agents
            .iter()
            .find(|row| row.agent == ToolId::new(name))
            .expect("the agent has a row")
    }

    #[test]
    fn agents_count_their_started_and_verified_tasks() {
        let evidence = build_evidence(&sample_input());
        let claude = agent(&evidence, "claude");
        assert_eq!(claude.tasks_started, 2);
        assert_eq!(claude.tasks_verified, 1);
        let codex = agent(&evidence, "codex");
        assert_eq!(codex.tasks_started, 1);
        assert_eq!(codex.tasks_verified, 0);
    }

    #[test]
    fn agent_spend_comes_from_the_tasks_it_last_started() {
        let evidence = build_evidence(&sample_input());
        let claude = agent(&evidence, "claude");
        assert_eq!(claude.spend.micro_usd(), 120_000);
        assert_eq!(claude.spend_source, "estimate");
        assert_eq!(agent(&evidence, "codex").spend_source, "reported");
    }

    #[test]
    fn parallel_share_is_summed_run_time_over_wall_time() {
        let evidence = build_evidence(&sample_input());
        assert_eq!(evidence.waves.count, 1);
        assert_eq!(evidence.waves.most_tasks, 2);
        // Run times 4 s + 7 s + 2 s = 13 s. Wall time 12 s. 13 / 12 is about 1.08.
        assert!((evidence.waves.parallel_share - 1.08).abs() < 0.001);
    }

    #[test]
    fn collisions_count_blocked_writes_restores_and_tampers() {
        let collisions = build_evidence(&sample_input()).collisions;
        assert_eq!(collisions.writes_blocked, 2);
        assert_eq!(collisions.writes_restored, 1);
        assert_eq!(collisions.blocked_paths, vec!["src/x.rs", "src/y.rs"]);
        assert_eq!(collisions.tampers_blocked, 1);
    }

    #[test]
    fn cost_per_verified_divides_the_total_by_verified_tasks() {
        let cost = build_evidence(&sample_input()).cost;
        assert_eq!(cost.tasks_verified, 1);
        assert_eq!(cost.per_verified.map(Cost::micro_usd), Some(170_000));
        assert_eq!(cost.percent_of_cap, Some(8));
        assert_eq!(cost.highest_threshold, Some(70));
    }

    #[test]
    fn per_verified_is_none_when_no_task_was_verified() {
        let mut input = sample_input();
        for task in &mut input.tasks {
            task.status = TaskStatus::Partial;
        }
        assert_eq!(build_evidence(&input).cost.per_verified, None);
    }

    #[test]
    fn claims_not_confirmed_counts_finished_tasks_that_were_not_verified() {
        let evidence = build_evidence(&sample_input());
        // T-2 checkpointed and ended Partial. T-3 never checkpointed, so it is not a claim.
        assert_eq!(evidence.honesty.claims_not_confirmed, 1);
        assert_eq!(evidence.honesty.verified, 1);
        assert_eq!(evidence.honesty.partial, 1);
        assert_eq!(evidence.honesty.blocked, 1);
    }

    #[test]
    fn a_task_with_no_stored_check_is_not_checked_rather_than_false() {
        let evidence = build_evidence(&sample_input());
        let claim = evidence
            .claims
            .iter()
            .find(|row| row.task == TaskId::new("T-3"))
            .expect("T-3 has a claim row");
        assert_eq!(claim.tests_pass, None);
        assert!(!claim.finished);
    }

    #[test]
    fn only_the_last_run_in_the_log_counts() {
        let mut input = sample_input();
        let mut events = vec![
            record(
                -100,
                Event::RunStarted {
                    run: sentinel_core::RunId::new("old"),
                    backlog: "old.md".to_owned(),
                },
            ),
            goes_to(-99, "T-9", "gemini"),
        ];
        events.append(&mut input.events);
        input.events = events;
        let evidence = build_evidence(&input);
        assert!(
            evidence
                .agents
                .iter()
                .all(|row| row.agent != ToolId::new("gemini"))
        );
    }

    #[test]
    fn render_evidence_has_its_headings_and_numbers() {
        let markdown = render_evidence(&build_evidence(&sample_input()));
        assert!(markdown.starts_with("\n## Evidence"));
        assert!(markdown.contains("| Agent | Tasks started | Verified | Spend | Spend source |"));
        assert!(markdown.contains("| claude | 2 | 1 | $0.12 est | estimate |"));
        assert!(markdown.contains("| Writes blocked | 2 |"));
        assert!(markdown.contains("| Tampers blocked | 1 |"));
        assert!(markdown.contains("| Task | Finished | Tests pass |"));
        assert!(markdown.contains("| T-3 | no | not checked |"));
        assert!(markdown.contains("Claims not confirmed: 1."));
        assert!(!markdown.contains('\u{2014}'));
    }

    #[test]
    fn evidence_survives_a_json_round_trip() {
        let evidence = build_evidence(&sample_input());
        let json = serde_json::to_string_pretty(&evidence).expect("evidence serializes");
        let back: Evidence = serde_json::from_str(&json).expect("evidence parses");
        assert_eq!(back, evidence);
    }
}
