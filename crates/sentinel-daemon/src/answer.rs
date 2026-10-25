//! Replies that need no run: a chat reply, an answer to a question, and research. Each asks one
//! agent once and posts what it says. The planner's calls also live here: they ask agents in turn
//! for a plan, or for a plain reply to a message that needs no plan.

use std::sync::Arc;
use std::time::Duration;

use sentinel_brain::mode::may_write;
use sentinel_brain::note::{Author, Note};
use sentinel_brain::strength;
use sentinel_core::{Cost, Event, HarnessState, Intent, NoteId, NoteType, Tier, ToolInfo};
use sentinel_plan::planner::{PlanRequest, holds_plan_json, planner_prompt};
use sentinel_workers::{Budget, EndKind, RunOutcome, Worker};
use tokio::sync::watch;

use crate::agents::{TEXT_TIMEOUT, TextJob, run_for_outcome};
use crate::brain::{note_written, slug, write_note};
use crate::controls::Caller;
use crate::engine::{Engine, Result};
use crate::intent::Typed;
use crate::plan_phase::{Asked, read_answer};
use crate::run_ctx::RunCtx;
use crate::run_source::RunSource;

/// The longest reply the chat gets from an agent.
pub(crate) const REPLY_CHARS: usize = 4000;

const ASK_CEILING_MICRO_USD: u64 = 50_000;
const ASK_TIMEOUT: Duration = Duration::from_secs(120);
const RESEARCH_CEILING_MICRO_USD: u64 = 250_000;
const RESEARCH_TIMEOUT: Duration = Duration::from_secs(300);

/// The longest title of a research note.
const TITLE_CHARS: usize = 80;
/// The first words of a question that name its research note.
const RESEARCH_WORDS: usize = 6;

/// Said when no planning agent is on and ready.
const NO_PLANNER: &str = "no planning agent is on and ready";

/// One question to one agent: the tier that picks it, the prompt, a cost ceiling in micro-dollars,
/// and the time it may take.
pub(crate) struct OneShot {
    pub tier: Tier,
    pub prompt: String,
    pub ceiling_micro_usd: u64,
    pub timeout: Duration,
}

/// Ask the cheapest ready agent of a tier once. Returns its plain-text answer, or the reason there
/// is none.
///
/// The call runs outside the write guard, which starts only with a run. The prompt's rule is the
/// only limit on what the agent writes, so every type that writes nothing says so in its rule.
pub(crate) async fn ask_once(
    engine: &Engine,
    shot: OneShot,
) -> std::result::Result<String, String> {
    let (tool, worker) = planner_agent(engine, shot.tier)?;
    let (budget, _budget_kill) = Budget::new(Cost::from_micro_usd(shot.ceiling_micro_usd, true));
    let (_stop_tx, stop) = watch::channel(false);
    let job = TextJob {
        stop,
        budget,
        worker: worker.as_ref(),
        tool: &tool,
        prompt: shot.prompt,
    };
    let (text, outcome) = run_for_outcome(engine, job, shot.timeout).await;
    if outcome.end != EndKind::Completed {
        return Err(no_answer_reason(&outcome));
    }
    reply_from(&text).ok_or_else(|| "it gave no usable answer".to_owned())
}

/// Ask the planner agents in turn. A message run stops at the first answer that is not a plan,
/// so a greeting costs one call.
pub(crate) async fn ask_agent_for_plan(ctx: &RunCtx, request: &PlanRequest) -> Asked {
    let message = ctx.source.words.is_some();
    let mut tier = Tier::Cheap;
    for _ in 0..2 {
        match ask_tier(ctx, request, tier).await {
            Asked::Unusable => {}
            Asked::Failed(_) if !message => {}
            asked @ (Asked::Plan(_) | Asked::Reply(_) | Asked::Failed(_)) => return asked,
        }
        tier = tier.escalate().unwrap_or(tier);
    }
    Asked::Unusable
}

/// One question to the planner agent of a tier.
async fn ask_tier(ctx: &RunCtx, request: &PlanRequest, tier: Tier) -> Asked {
    let (tool, worker) = match planner_agent(&ctx.engine, tier) {
        Ok(found) => found,
        Err(reason) => return Asked::Failed(reason),
    };
    let job = TextJob {
        stop: ctx.stop.clone(),
        budget: ctx.budget.clone(),
        worker: worker.as_ref(),
        tool: &tool,
        prompt: planner_prompt(request),
    };
    let (text, outcome) = run_for_outcome(&ctx.engine, job, TEXT_TIMEOUT).await;
    read_answer(&text, &outcome, request, ctx.source.words.is_some())
}

/// The tool and worker of the planner agent for a tier, or the reason there is none.
fn planner_agent(
    engine: &Engine,
    tier: Tier,
) -> std::result::Result<(ToolInfo, Arc<dyn Worker>), String> {
    let router = engine.router().map_err(|e| e.to_string())?;
    let agent = router.pick(tier).ok_or_else(|| NO_PLANNER.to_owned())?;
    let tool = engine
        .store()
        .list_tools()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|t| t.id == agent)
        .ok_or_else(|| NO_PLANNER.to_owned())?;
    let worker = engine.worker_for(&tool).map_err(|e| e.to_string())?;
    Ok((tool, worker))
}

/// The answer as a chat reply. `None` when the answer holds a plan or is empty.
pub(crate) fn reply_from(answer: &str) -> Option<String> {
    if holds_plan_json(answer) {
        return None;
    }
    let text = strip_fence(answer.trim());
    if text.is_empty() {
        return None;
    }
    Some(text.chars().take(REPLY_CHARS).collect())
}

/// Drop a code fence that wraps the whole answer, with its language line when it has one.
fn strip_fence(text: &str) -> &str {
    let Some(inner) = text
        .strip_prefix("```")
        .and_then(|rest| rest.strip_suffix("```"))
    else {
        return text;
    };
    inner
        .split_once('\n')
        .map_or(inner, |(_, body)| body)
        .trim()
}

/// Why an agent gave no answer, in words for the owner.
pub(crate) fn no_answer_reason(outcome: &RunOutcome) -> String {
    match outcome.end {
        EndKind::Completed => "it gave no answer".to_owned(),
        EndKind::Failed => match outcome.exit_code {
            Some(code) => format!("it exited with code {code}"),
            None => "it failed".to_owned(),
        },
        EndKind::TimedOut => "it timed out".to_owned(),
        EndKind::Killed => "it was stopped".to_owned(),
        EndKind::CostCeiling => "it hit its cost limit".to_owned(),
    }
}

/// What the owner reads when no planning agent answered.
pub(crate) fn unreached_text(reason: &str) -> String {
    format!("I could not reach a planning agent: {reason}. Check Settings, Agents and press Test.")
}

/// What the owner reads when a question got no answer.
fn unanswered_text(reason: &str) -> String {
    format!("I could not get an answer: {reason}. Check Settings, Agents and press Test.")
}

impl Engine {
    /// Post a message that needs no run: the words, their type, then the answer. The harness is
    /// idle again afterwards, since no run ever started.
    pub(crate) async fn answer_without_run(
        &self,
        source: &RunSource,
        caller: &Caller,
        typed: &Typed,
    ) -> Result<()> {
        let chat = &source.chat;
        self.emit_in(
            Some(chat),
            &Event::Message {
                from: caller.name.clone(),
                to: None,
                text: source.message.clone(),
            },
        )?;
        self.announce_intent(chat, Some(typed))?;
        let question = source.words.as_deref().unwrap_or_default();
        let text = self.reply_text(question, typed.intent).await;
        self.emit_in(
            Some(chat),
            &Event::Message {
                from: "sentinel".to_owned(),
                to: None,
                text,
            },
        )?;
        self.set_state(HarnessState::Idle);
        Ok(())
    }

    /// The reply to a question of a type that needs no run. Research is kept as a Brain note in
    /// learn mode, and the reply says so.
    async fn reply_text(&self, question: &str, intent: Intent) -> String {
        let answer = match ask_once(self, one_shot(self, intent, question)).await {
            Ok(answer) => answer,
            Err(reason) => return unanswered_text(&reason),
        };
        if intent != Intent::Research || !may_write(self.config().brain_mode) {
            return answer;
        }
        match self.save_research_note(question, &answer) {
            Ok(()) => format!("{answer}\n\nSaved to the Brain as a note."),
            Err(error) => format!("{answer}\n\nThe note could not be saved: {error}"),
        }
    }

    /// Keep a research question and its answer as a how-to note, written by the computer.
    fn save_research_note(&self, question: &str, answer: &str) -> Result<()> {
        let state = self.brain()?;
        let note = Note {
            id: NoteId::new(research_note_id(question)),
            note_type: NoteType::HowTo,
            title: question.chars().take(TITLE_CHARS).collect(),
            tags: Vec::new(),
            by: Author::Run,
            strength: strength::INITIAL,
            confirmed: None,
            pinned: false,
            lesson: None,
            body: format!("{question}\n\n{answer}"),
            extra: Vec::new(),
        };
        write_note(&state, &note, None)?;
        note_written(self, &note.id);
        Ok(())
    }
}

/// The agent, the time, and the cost for a question of one type. Research gets the mid tier and
/// more room, because it reads more.
fn one_shot(engine: &Engine, intent: Intent, question: &str) -> OneShot {
    let (tier, ceiling_micro_usd, timeout) = if intent == Intent::Research {
        (Tier::Mid, RESEARCH_CEILING_MICRO_USD, RESEARCH_TIMEOUT)
    } else {
        (Tier::Cheap, ASK_CEILING_MICRO_USD, ASK_TIMEOUT)
    };
    let project = engine.repo_path().file_name().map_or_else(
        || "this project".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    OneShot {
        tier,
        prompt: format!(
            "{}\n\nThe project is {project}.\n\nMessage:\n{question}",
            intent.rule()
        ),
        ceiling_micro_usd,
        timeout,
    }
}

/// The id of a research note: `research-` and the first words of the question.
fn research_note_id(question: &str) -> String {
    let words: Vec<&str> = question.split_whitespace().take(RESEARCH_WORDS).collect();
    format!("research-{}", slug(&words.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_research_question_names_its_note_by_its_first_six_words() {
        assert_eq!(
            research_note_id("How does the retry queue back off after a failed wave today?"),
            "research-how-does-the-retry-queue-back"
        );
    }

    #[test]
    fn a_research_note_id_is_lowercase_dashes_only() {
        assert_eq!(
            research_note_id("Compare the Ledger, the Vault!"),
            "research-compare-the-ledger-the-vault"
        );
    }

    #[test]
    fn a_fence_around_the_whole_answer_is_removed() {
        assert_eq!(reply_from("```\nHi there.\n```"), Some("Hi there.".to_owned()));
    }

    #[test]
    fn an_answer_that_holds_a_plan_is_not_a_reply() {
        assert_eq!(reply_from(r#"{"tasks": []}"#), None);
    }

    #[test]
    fn a_long_reply_is_cut_at_4000_characters() {
        let long = "é".repeat(5000);
        let reply = reply_from(&long).unwrap_or_default();
        assert_eq!(reply.chars().count(), REPLY_CHARS);
    }

    #[test]
    fn the_unreached_message_names_the_reason_and_the_next_step() {
        assert_eq!(
            unreached_text("it timed out"),
            "I could not reach a planning agent: it timed out. Check Settings, Agents and press Test."
        );
    }

    #[test]
    fn a_question_with_no_answer_names_the_reason() {
        assert_eq!(
            unanswered_text("it timed out"),
            "I could not get an answer: it timed out. Check Settings, Agents and press Test."
        );
    }
}
