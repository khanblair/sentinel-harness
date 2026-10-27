//! The first phase of a run: read the backlog, plan it, settle contradictions, and mark which
//! tasks are ready. A message that needs no work ends here with its reply, and no run starts.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sentinel_core::api::RunRequest;
use sentinel_core::{DecisionState, Event, Task, TaskId, TaskStatus};
use sentinel_gate::policy::{next_mode_after_plan_approval, plan_gate};
use sentinel_plan::backlog::{BacklogItem, parse_backlog};
use sentinel_plan::contradiction::{
    ContradictionPair, Resolution, apply_answer, decision_for, find_contradictions,
};
use sentinel_plan::planner::{
    Plan, PlanRequest, build_tasks, fill_empty_scopes, order_shared_scopes,
};
use sentinel_store::VerificationRecord;

use crate::answer::unreached_text;
use crate::ask::{ask_approval, ask_decision};
use crate::engine::{Engine, EngineError, Result, lock};
use crate::resume::resume_phase;
use crate::run::Ending;
use crate::run_ctx::RunCtx;
use crate::run_source::{backlog_path, with_context, words_item};

impl Engine {
    /// Put the run on the stream. The run is registered by now, so the event belongs to its chat.
    pub(crate) fn announce_run_started(&self) -> Result<()> {
        let run = lock(&self.inner.live)
            .run
            .clone()
            .ok_or_else(|| EngineError::Invalid("run not registered".to_owned()))?;
        self.emit(&Event::RunStarted {
            run: run.id,
            backlog: run.backlog,
        })?;
        Ok(())
    }
}

/// What the plan phase found.
pub(crate) enum PlanOutcome {
    /// The plan is approved, or needed no approval: the run goes on.
    Approved,
    /// The user denied the plan card: the run ends before any task starts.
    Declined,
    /// The message got a reply, or no planner answered. The run never starts.
    NoRun,
    /// The chat's agent gave no plan. Its reason is in the chat, and no other agent planned.
    Halted,
}

/// What the planning step produced.
pub(crate) enum PlanStep {
    Plan(Plan),
    /// The planner answered a message that needs no plan. The text goes to the chat as it is.
    Reply(String),
    /// No planning agent answered. The reason goes to the chat.
    Unreached(String),
    /// The chat's agent gave no plan. The text names the agent and the reason.
    Halted(String),
}

/// The plan of a run, or how the run ends without one.
enum Planned {
    Plan(Plan),
    Ended(PlanOutcome),
}

/// What one planner agent said, as far as the run needs it.
pub(crate) enum Asked {
    Plan(Plan),
    Reply(String),
    /// The agent could not answer. A message run stops here, a backlog run tries another agent.
    Failed(String),
    /// The agent answered, but not with a usable plan.
    Unusable,
}

pub(crate) async fn plan_phase(ctx: &Arc<RunCtx>, request: &RunRequest) -> Result<PlanOutcome> {
    let items = read_items(ctx, request)?;
    let plan_request = PlanRequest {
        items,
        repo_files: repo_files(ctx),
    };
    let plan = match plan_or_message(ctx, &plan_request).await? {
        Planned::Plan(plan) => plan,
        Planned::Ended(outcome) => return Ok(outcome),
    };
    let message = ctx.source.words.is_some();
    if message {
        ctx.engine.announce_run_started()?;
    }
    let plan = shape_plan(ctx, &plan_request, plan);
    let (tasks, dependencies) =
        build_tasks(&plan_request, &plan).map_err(|e| EngineError::Invalid(e.to_string()))?;
    if message {
        warn_if_no_files(ctx, &tasks)?;
    }
    let pairs = find_contradictions(&plan);
    store_plan(ctx, &tasks, &dependencies)?;
    let already_done = flag_claimed_done(ctx, &tasks);
    ctx.engine.emit(&Event::PlanReady {
        tasks: count(tasks.len()),
        contradictions: count(pairs.len()),
        already_done,
    })?;
    settle_contradictions(ctx, &pairs, &tasks, &plan).await?;
    mark_ready(ctx)?;
    if approve_plan(ctx).await {
        Ok(PlanOutcome::Approved)
    } else {
        Ok(PlanOutcome::Declined)
    }
}

fn store_plan(
    ctx: &RunCtx,
    tasks: &[Task],
    dependencies: &[sentinel_core::Dependency],
) -> Result<()> {
    let store = ctx.engine.store();
    for task in tasks {
        store.upsert_task(task)?;
    }
    for dependency in dependencies {
        store.add_dependency(dependency)?;
    }
    Ok(())
}

/// The plan to run, or how the run ends without one. The text goes to the chat, and no run starts
/// for a message. A backlog run that fails here still ends as a run.
async fn plan_or_message(ctx: &RunCtx, request: &PlanRequest) -> Result<Planned> {
    let message = ctx.source.words.is_some();
    let (text, outcome) = match crate::agents::make_plan(ctx, request).await {
        Ok(PlanStep::Plan(plan)) => return Ok(Planned::Plan(plan)),
        Ok(PlanStep::Reply(text)) => (text, PlanOutcome::NoRun),
        Ok(PlanStep::Unreached(reason)) => (unreached_text(&reason), PlanOutcome::NoRun),
        Ok(PlanStep::Halted(text)) => (text, PlanOutcome::Halted),
        Err(error) if message => (
            format!("The run ended because of an error: {error}"),
            PlanOutcome::NoRun,
        ),
        Err(error) => return Err(error),
    };
    ctx.engine.emit_quiet(&Event::Message {
        from: "sentinel".to_owned(),
        to: None,
        text,
    });
    Ok(Planned::Ended(outcome))
}

/// A message run that plans a task with no files cannot change anything. Said once, naming the
/// first such task, so the owner can name the files.
fn warn_if_no_files(ctx: &RunCtx, tasks: &[Task]) -> Result<()> {
    let Some(task) = tasks.iter().find(|t| t.scope.is_empty()) else {
        return Ok(());
    };
    ctx.engine.emit(&Event::Message {
        from: "sentinel".to_owned(),
        to: None,
        text: format!(
            "{} has no files to write, so it cannot change anything. Name the files in your message.",
            task.id
        ),
    })?;
    Ok(())
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn read_items(ctx: &RunCtx, request: &RunRequest) -> Result<Vec<BacklogItem>> {
    let source = &ctx.source;
    if let Some(words) = &source.words {
        let mut item = words_item(words);
        // Only a change gets the rule in its task. A plan waits for the owner's approval first.
        if let Some(intent) = request.intent.filter(|intent| intent.writes()) {
            item.body = format!("{}\n\n{}", intent.rule(), item.body);
        }
        return Ok(with_context(vec![item], &source.context));
    }
    let engine = &ctx.engine;
    let name = source
        .backlog
        .clone()
        .unwrap_or_else(|| ctx.config.backlog.clone());
    let path = resolve(&engine.repo_path(), &backlog_path(&name)?);
    let text = fs::read_to_string(&path).map_err(|e| {
        EngineError::Invalid(format!(
            "Could not read the backlog {}: {e}",
            path.display()
        ))
    })?;
    let mut items = parse_backlog(&text).map_err(|e| EngineError::Invalid(e.to_string()))?;
    if let Some(only) = &request.task {
        items.retain(|item| &item.id == only);
        if items.is_empty() {
            return Err(EngineError::NotFound(format!("task {only} in the backlog")));
        }
    }
    Ok(with_context(items, &source.context))
}

fn resolve(repo: &Path, name: &str) -> PathBuf {
    let path = Path::new(name);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo.join(path)
    }
}

/// Files of the repository, for the planner to choose scopes from.
fn repo_files(ctx: &RunCtx) -> Vec<String> {
    ctx.git
        .run(&["ls-files"])
        .map(|text| text.lines().take(2000).map(str::to_owned).collect())
        .unwrap_or_default()
}

/// Puts a note from the run in the stream, from `sentinel`.
fn say(ctx: &RunCtx, text: String) {
    ctx.engine.emit_quiet(&Event::Message {
        from: "sentinel".to_owned(),
        to: None,
        text,
    });
}

/// Gives unnamed tasks a scope, then orders the tasks that write the same file. Each change is
/// said in the stream, so the owner can see why a task waits.
fn shape_plan(ctx: &RunCtx, request: &PlanRequest, plan: Plan) -> Plan {
    let (plan, folder_only) = fill_empty_scopes(request, plan);
    for (id, folder) in &folder_only {
        say(
            ctx,
            format!(
                "{id} has no named files, so it may write inside {folder} and waits for other tasks that use it."
            ),
        );
    }
    let before = dependency_count(&plan);
    let plan = order_shared_scopes(plan);
    let added = dependency_count(&plan).saturating_sub(before);
    if added > 0 {
        say(
            ctx,
            format!(
                "Tasks that write the same file run in id order. {added} ordering link(s) added."
            ),
        );
    }
    plan
}

fn dependency_count(plan: &Plan) -> usize {
    plan.tasks.iter().map(|task| task.depends_on.len()).sum()
}

/// A task the backlog calls done is still checked. It stays ready, and the stream says so.
fn flag_claimed_done(ctx: &RunCtx, tasks: &[Task]) -> u32 {
    let mut flagged = 0;
    for task in tasks.iter().filter(|t| t.flags.already_done) {
        say(
            ctx,
            format!(
                "{} says it is already done. It will be checked, not skipped.",
                task.id
            ),
        );
        flagged += 1;
    }
    flagged
}

/// Move a task to an end status that no worker produced, and record why.
pub(crate) fn finish_without_work(
    ctx: &RunCtx,
    task: &TaskId,
    status: TaskStatus,
    reason: &str,
) -> Result<()> {
    let store = ctx.engine.store();
    store.set_task_status(task, status)?;
    store.upsert_verification(&VerificationRecord {
        task: task.clone(),
        tests_pass: false,
        new_test: false,
        reviewer_ok: false,
        tamper_clear: true,
        verdict: status,
        reason: reason.to_owned(),
        receipt_json: "null".to_owned(),
    })?;
    ctx.engine.emit(&Event::TaskStatus {
        task: task.clone(),
        status,
    })?;
    ctx.engine.emit(&Event::Verdict {
        task: task.clone(),
        status,
        reason: reason.to_owned(),
    })?;
    Ok(())
}

async fn settle_contradictions(
    ctx: &Arc<RunCtx>,
    pairs: &[ContradictionPair],
    tasks: &[Task],
    plan: &Plan,
) -> Result<()> {
    let title = |id: &TaskId| {
        tasks
            .iter()
            .find(|t| &t.id == id)
            .map_or_else(String::new, |t| t.title.clone())
    };
    for pair in pairs {
        if ctx.config.park_contradictions {
            park_pair(ctx, pair, plan)?;
            continue;
        }
        let decision = decision_for(pair, &title);
        let answer = ask_decision(ctx, &decision).await;
        let state = if answer.is_some() {
            DecisionState::Answered
        } else {
            DecisionState::Defaulted
        };
        let resolved = sentinel_core::Decision {
            state,
            answer: answer.clone(),
            ..decision
        };
        let resolution = apply_answer(&resolved, answer.as_deref());
        apply_resolution(ctx, resolution, answer.is_some())?;
    }
    Ok(())
}

/// Skips both tasks of a contradiction without asking. Each task gives its planner note as the reason.
fn park_pair(ctx: &RunCtx, pair: &ContradictionPair, plan: &Plan) -> Result<()> {
    say(
        ctx,
        format!(
            "{} and {} contradict each other, so both are skipped without a question.",
            pair.a, pair.b
        ),
    );
    for id in [&pair.a, &pair.b] {
        let reason = parked_reason(pair, &note_of(plan, id));
        finish_without_work(ctx, id, TaskStatus::Skipped, &reason)?;
    }
    Ok(())
}

/// The reason a parked task is skipped. It names both tasks and, when the planner gave one, the note.
fn parked_reason(pair: &ContradictionPair, note: &str) -> String {
    let base = format!(
        "These two tasks contradict each other: {} and {}. Neither was done.",
        pair.a, pair.b
    );
    let note = note.trim();
    if note.is_empty() {
        base
    } else {
        format!("{base} {note}")
    }
}

fn note_of(plan: &Plan, id: &TaskId) -> String {
    plan.tasks
        .iter()
        .find(|task| &task.id == id)
        .map_or_else(String::new, |task| task.note.clone())
}

/// Carry out what the user chose: skip the loser, or mark both tasks as contradicting.
fn apply_resolution(ctx: &RunCtx, resolution: Resolution, answered: bool) -> Result<()> {
    match resolution {
        Resolution::Keep { chosen, skipped } => {
            let reason = format!("You chose {chosen} instead, so this task was skipped.");
            finish_without_work(ctx, &skipped, TaskStatus::Skipped, &reason)
        }
        Resolution::SkipBoth { a, b } => {
            let why = if answered {
                "You skipped both tasks."
            } else {
                "Nobody answered, so both tasks were skipped."
            };
            let reason = format!("{a} and {b} contradict each other. {why}");
            finish_without_work(ctx, &a, TaskStatus::Contradiction, &reason)?;
            finish_without_work(ctx, &b, TaskStatus::Contradiction, &reason)
        }
    }
}

fn mark_ready(ctx: &RunCtx) -> Result<()> {
    let store = ctx.engine.store();
    for task in store.list_tasks()? {
        if task.status == TaskStatus::Planned {
            store.set_task_status(&task.id, TaskStatus::Ready)?;
            ctx.engine.emit(&Event::TaskStatus {
                task: task.id,
                status: TaskStatus::Ready,
            })?;
        }
    }
    Ok(())
}

/// Plan only mode puts one card after the plan. After an approval the run continues in Auto.
async fn approve_plan(ctx: &Arc<RunCtx>) -> bool {
    let mode = ctx.permission_mode();
    if !plan_gate(mode) {
        return true;
    }
    let card = lock(&ctx.book).request_plan(ctx.engine.now());
    let allowed = ask_approval(ctx, card).await;
    if allowed {
        let next = next_mode_after_plan_approval(mode);
        *lock(&ctx.mode) = next;
        ctx.engine
            .emit_quiet(&Event::PermissionChanged { mode: next });
        ctx.engine.emit_quiet(&Event::Message {
            from: "sentinel".to_owned(),
            to: None,
            text: format!("Plan approved. The run goes on in {}.", next.label()),
        });
    }
    allowed
}

/// The number of the first wave, or how the run ended before any wave.
pub(crate) async fn first_wave(
    ctx: &Arc<RunCtx>,
    request: &RunRequest,
) -> Result<std::result::Result<u32, Ending>> {
    if ctx.resuming {
        return Ok(Ok(resume_phase(ctx).await?));
    }
    Ok(match plan_phase(ctx, request).await? {
        PlanOutcome::NoRun => Err(Ending::WithoutRun),
        // A backlog run was announced at its start, so it ends as a run. A message never was.
        PlanOutcome::Halted if ctx.source.words.is_some() => Err(Ending::WithoutRun),
        PlanOutcome::Halted => Err(Ending::Finished),
        PlanOutcome::Declined => {
            ctx.engine.emit_quiet(&Event::Message {
                from: "sentinel".to_owned(),
                to: None,
                text: "The plan was not approved, so nothing ran.".to_owned(),
            });
            Err(Ending::Finished)
        }
        PlanOutcome::Approved => Ok(1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::answer::{REPLY_CHARS, reply_from};

    #[test]
    fn a_plain_answer_is_the_reply_as_it_is() {
        assert_eq!(
            reply_from("Hello! What would you like to build?"),
            Some("Hello! What would you like to build?".to_owned())
        );
    }

    #[test]
    fn a_fence_around_the_whole_answer_is_removed() {
        assert_eq!(
            reply_from("```\nHi there.\n```"),
            Some("Hi there.".to_owned())
        );
    }

    #[test]
    fn an_answer_that_holds_a_plan_is_not_a_reply() {
        assert_eq!(reply_from(r#"{"tasks": []}"#), None);
    }

    #[test]
    fn an_empty_answer_is_not_a_reply() {
        assert_eq!(reply_from("  \n "), None);
    }

    #[test]
    fn a_long_reply_is_cut_at_4000_characters() {
        let long = "é".repeat(5000);
        let reply = reply_from(&long).unwrap_or_default();
        assert_eq!(reply.chars().count(), REPLY_CHARS);
    }

    #[test]
    fn the_unreached_message_names_the_reason_and_the_next_step() {
        let text = unreached_text("it timed out");
        assert_eq!(
            text,
            "I could not reach a planning agent: it timed out. Check Settings, Agents and press Test."
        );
    }

    #[test]
    fn a_parked_reason_names_both_tasks_and_adds_the_planner_note() {
        let pair = ContradictionPair {
            a: TaskId::new("T-01"),
            b: TaskId::new("T-02"),
        };
        assert_eq!(
            parked_reason(&pair, ""),
            "These two tasks contradict each other: T-01 and T-02. Neither was done."
        );
        assert_eq!(
            parked_reason(
                &pair,
                "One removes a fee and the other sets a minimum for it."
            ),
            "These two tasks contradict each other: T-01 and T-02. Neither was done. One removes a fee and the other sets a minimum for it."
        );
    }
}
