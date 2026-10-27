//! Replies that need no run: a chat reply, an answer to a question, and research. Each asks one
//! agent once and posts what it says. The planner's calls also live here: they ask agents in turn
//! for a plan, or for a plain reply to a message that needs no plan. A chat with its own agent
//! asks that agent alone, and a failure stops the run instead of trying another agent.

use std::time::Duration;

use sentinel_brain::mode::may_write;
use sentinel_core::{ChatId, Cost, Event, HarnessState, Intent, NoteId, NoteType, Tier, ToolId};
use sentinel_plan::planner::{PlanRequest, holds_plan_json, parse_plan_json};
use sentinel_workers::{Budget, EndKind, RunOutcome};
use tokio::sync::watch;

use crate::agents::{TextJob, run_for_outcome};
use crate::answer_plan::planner_agent;
use crate::brain::{note_written, slug, write_note};
use crate::controls::Caller;
use crate::engine::{Engine, Result};
use crate::intent::Typed;
use crate::plan_phase::Asked;
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
pub(crate) const NO_PLANNER: &str = "no planning agent is on and ready";

/// One question to one agent: the tier that picks it, the prompt, a cost ceiling in micro-dollars,
/// the time it may take, and the chat's agent when the chat has one.
pub(crate) struct OneShot {
    pub tier: Tier,
    pub prompt: String,
    pub ceiling_micro_usd: u64,
    pub timeout: Duration,
    pub forced: Option<ToolId>,
}

/// Ask the cheapest ready agent of a tier once, or the chat's agent. Returns its plain-text answer,
/// or the reason there is none.
///
/// The call runs outside the write guard, which starts only with a run. The prompt's rule is the
/// only limit on what the agent writes, so every type that writes nothing says so in its rule.
pub(crate) async fn ask_once(
    engine: &Engine,
    shot: OneShot,
) -> std::result::Result<String, String> {
    let (tool, worker) = planner_agent(engine, shot.tier, shot.forced.as_ref())?;
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

/// The answer in a tool's output. A tool run with a JSON output format (claude) wraps its answer
/// in an object with a `result` text. Anything else is the plain output.
pub(crate) fn answer_text(output: &str) -> String {
    output
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line.trim()).ok())
        .find_map(|value| value.get("result")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| output.to_owned())
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

/// Read one planner answer. A message run reads prose as a reply, and a failed run as no answer.
pub(crate) fn read_answer(
    text: &str,
    outcome: &RunOutcome,
    request: &PlanRequest,
    message: bool,
) -> Asked {
    if let Ok(plan) = parse_plan_json(text, request) {
        return Asked::Plan(crate::agents::without_existing_tests(plan, request));
    }
    if !message {
        return Asked::Unusable;
    }
    if outcome.end != EndKind::Completed || text.trim().is_empty() {
        return Asked::Failed(no_answer_reason(outcome));
    }
    reply_from(text).map_or(Asked::Unusable, Asked::Reply)
}

/// What the owner reads when no planning agent answered.
pub(crate) fn unreached_text(reason: &str) -> String {
    format!("I could not reach a planning agent: {reason}. Check Settings, Agents and press Test.")
}

/// What the owner reads when a question got no answer.
fn unanswered_text(reason: &str) -> String {
    format!("I could not get an answer: {reason}. Check Settings, Agents and press Test.")
}

/// What the owner reads when the chat's own agent gave no answer. The agent is named, and the run
/// does not go on with another one.
pub(crate) fn could_not_answer_text(agent: &str, reason: &str) -> String {
    format!(
        "{agent} could not answer: {}.",
        reason.trim_end_matches('.')
    )
}

/// Who a reply is for, and the agent that must give it when the chat has its own.
pub(crate) struct Asker<'a> {
    pub(crate) caller: &'a Caller,
    pub(crate) forced: Option<&'a ToolId>,
}

impl Engine {
    /// Post a message that needs no run: the words, their type, then the answer. The harness is
    /// idle again afterwards, since no run ever started.
    pub(crate) async fn answer_without_run(
        &self,
        source: &RunSource,
        asker: &Asker<'_>,
        typed: &Typed,
    ) -> Result<()> {
        let chat = &source.chat;
        self.announce_words(source, asker.caller)?;
        self.announce_intent(chat, Some(typed))?;
        let text = self.reply_text(source, typed.intent, asker.forced).await?;
        self.post_sentinel(chat, text)?;
        self.set_state(HarnessState::Idle);
        Ok(())
    }

    /// The type step could not get the chat's agent to type the message. The words and the reason
    /// go to the chat, and no run starts.
    pub(crate) fn answer_halted(
        &self,
        source: &RunSource,
        asker: &Asker<'_>,
        reason: &str,
    ) -> Result<()> {
        self.announce_words(source, asker.caller)?;
        let text = self.no_answer(&source.chat, asker.forced, reason)?;
        self.post_sentinel(&source.chat, text)?;
        self.set_state(HarnessState::Idle);
        Ok(())
    }

    fn announce_words(&self, source: &RunSource, caller: &Caller) -> Result<()> {
        self.emit_in(
            Some(&source.chat),
            &Event::Message {
                from: caller.name.clone(),
                to: None,
                text: source.message.clone(),
            },
        )?;
        Ok(())
    }

    fn post_sentinel(&self, chat: &ChatId, text: String) -> Result<()> {
        self.emit_in(
            Some(chat),
            &Event::Message {
                from: "sentinel".to_owned(),
                to: None,
                text,
            },
        )?;
        Ok(())
    }

    /// The reply to a question of a type that needs no run. Research is kept as a Brain note in
    /// learn mode, and the reply says so.
    async fn reply_text(
        &self,
        source: &RunSource,
        intent: Intent,
        forced: Option<&ToolId>,
    ) -> Result<String> {
        let chat = &source.chat;
        let question = source.words.as_deref().unwrap_or_default();
        let shot = one_shot(self, intent, question, forced);
        let answer = match ask_once(self, shot).await {
            Ok(answer) => answer,
            Err(reason) => return self.no_answer(chat, forced, &reason),
        };
        if intent != Intent::Research || !may_write(self.config().brain_mode) {
            return Ok(answer);
        }
        Ok(match self.save_research_note(question, &answer) {
            Ok(()) => format!("{answer}\n\nSaved to the Brain as a note."),
            Err(error) => format!("{answer}\n\nThe note could not be saved: {error}"),
        })
    }

    /// The reply when an agent gave no answer. With no forced agent, the owner is told to check the
    /// agents. A forced agent is named, and the stop goes to the stream.
    pub(crate) fn no_answer(
        &self,
        chat: &ChatId,
        forced: Option<&ToolId>,
        reason: &str,
    ) -> Result<String> {
        let Some(agent) = forced else {
            return Ok(unanswered_text(reason));
        };
        self.emit_in(
            Some(chat),
            &Event::AgentStopped {
                chat: chat.clone(),
                tool: agent.clone(),
                reason: reason.to_owned(),
            },
        )?;
        Ok(could_not_answer_text(&self.agent_name(agent)?, reason))
    }

    /// The name of an agent for the owner. The id stands in when the store does not know the agent.
    fn agent_name(&self, agent: &ToolId) -> Result<String> {
        let found = self
            .store()
            .list_tools()?
            .into_iter()
            .find(|tool| &tool.id == agent);
        Ok(found.map_or_else(|| agent.to_string(), |tool| tool.name))
    }

    /// Keep a research question and its answer as a how-to note, written by the computer.
    fn save_research_note(&self, question: &str, answer: &str) -> Result<()> {
        let state = self.brain()?;
        let note = sentinel_brain::journal::note_with_body(
            NoteId::new(research_note_id(question)),
            NoteType::HowTo,
            question.chars().take(TITLE_CHARS).collect(),
            format!("{question}\n\n{answer}"),
        );
        write_note(&state, &note, None)?;
        note_written(self, &note.id);
        Ok(())
    }
}

/// The agent, the time, and the cost for a question of one type. Research gets the mid tier and
/// more room, because it reads more.
fn one_shot(engine: &Engine, intent: Intent, question: &str, forced: Option<&ToolId>) -> OneShot {
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
        forced: forced.cloned(),
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
    fn a_json_envelope_gives_its_result_text() {
        let output = concat!(
            r#"{"type":"result","result":"the plan","#,
            r#""total_cost_usd":0.01}"#
        );
        assert_eq!(answer_text(output), "the plan");
    }

    #[test]
    fn plain_output_stays_as_it_is() {
        assert_eq!(answer_text("line one\nline two"), "line one\nline two");
        assert_eq!(answer_text(r#"{"no":"result"}"#), r#"{"no":"result"}"#);
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

    #[test]
    fn a_message_run_reads_a_failed_run_as_no_answer_even_with_text() {
        let request = PlanRequest {
            items: Vec::new(),
            repo_files: Vec::new(),
        };
        let outcome = RunOutcome {
            end: EndKind::Failed,
            exit_code: Some(2),
            output_tail: Vec::new(),
            cost: Cost::ZERO,
        };
        let asked = read_answer("error: boom", &outcome, &request, true);
        assert!(
            matches!(asked, Asked::Failed(reason) if reason == "it exited with code 2"),
            "a failed run gave text, so it is not a reply"
        );
    }

    #[test]
    fn a_forced_agent_with_no_answer_is_named_and_the_reason_ends_in_one_period() {
        assert_eq!(
            could_not_answer_text("Claude", "it timed out."),
            "Claude could not answer: it timed out."
        );
    }
}
