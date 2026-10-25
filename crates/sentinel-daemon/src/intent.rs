//! What a message asks for. Plain rules decide first. A message that no rule matches goes to the
//! cheapest agent, and an unclear answer falls back to a question. Nothing falls back to a change.

use std::time::Duration;

use sentinel_core::api::RunRequest;
use sentinel_core::{ChatId, Event, Intent, PermissionMode, Tier, ToolId};
use serde::Deserialize;

use crate::answer::{Asker, OneShot, ask_once};
use crate::controls::Caller;
use crate::engine::{Engine, Result};
use crate::run_source::RunSource;

/// What the owner reads when the type of a message is not clear.
const UNSURE_REASON: &str = "Not sure what you want. Say /fix to change files.";
const CLASSIFY_CEILING_MICRO_USD: u64 = 50_000;
const CLASSIFY_TIMEOUT: Duration = Duration::from_secs(30);
/// The longest reason the router may give. The stream shows it.
const REASON_CHARS: usize = 200;

const GREETING_WORDS: &[&str] = &["hi", "hello", "hey", "yo", "thanks"];
const CHANGE_VERBS: &[&str] = &[
    "fix",
    "add",
    "implement",
    "refactor",
    "rename",
    "remove",
    "delete",
    "update",
    "change",
    "write",
    "create",
    "build",
    "make",
    "move",
    "migrate",
];
const QUESTION_WORDS: &[&str] = &[
    "what", "where", "why", "how", "which", "who", "when", "does", "is", "are", "can", "explain",
];

/// A message's type, who chose it, and why.
pub(crate) struct Typed {
    pub intent: Intent,
    pub reason: String,
    /// `router` when the classifier chose it, `you` when the request named it.
    pub by: &'static str,
}

/// What the type step did with a message. A reply needs no run; a run goes on with its type.
pub(crate) enum Typing {
    /// The reply is in the chat, and no run starts.
    Answered,
    /// A run goes on. Only a message has a type: a backlog or task run has none.
    Run(Option<Typed>),
}

/// The type of a text from the plain rules alone. `None` when no rule matches.
pub(crate) fn heuristic(text: &str) -> Option<(Intent, &'static str)> {
    let lower = text.trim().to_lowercase();
    let words: Vec<&str> = lower
        .split_whitespace()
        .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|word| !word.is_empty())
        .collect();
    let first = *words.first()?;
    // "hey fix it" is a request: look past a greeting or a please at the start.
    let opening = words
        .iter()
        .copied()
        .find(|word| !GREETING_WORDS.contains(word) && *word != "please")
        .unwrap_or(first);
    let has_change_verb = words.iter().any(|word| CHANGE_VERBS.contains(word));
    if is_greeting(&words, has_change_verb) {
        return Some((Intent::Chat, "A greeting."));
    }
    if lower.starts_with("plan ") {
        return Some((Intent::Plan, "You asked for a plan."));
    }
    let researches =
        matches!(first, "research" | "compare" | "investigate") || lower.starts_with("look into ");
    if researches {
        return Some((Intent::Research, "You asked for research."));
    }
    let asks =
        lower.ends_with('?') || QUESTION_WORDS.contains(&first) || lower.starts_with("show me ");
    if asks && !has_change_verb {
        return Some((Intent::Ask, "A question about the project."));
    }
    if CHANGE_VERBS.contains(&opening) {
        return Some((Intent::Change, "A request to change the project."));
    }
    None
}

/// A greeting is at most four words, opens with a greeting word, and names no change.
fn is_greeting(words: &[&str], has_change_verb: bool) -> bool {
    let opens_with_greeting = matches!(words, [first, ..] if GREETING_WORDS.contains(first))
        || matches!(words, ["thank", "you", ..] | ["good", "morning", ..]);
    words.len() <= 4 && !has_change_verb && opens_with_greeting
}

/// The type of a message: the plain rules first, then the cheapest agent for what they leave open,
/// or the chat's agent when the chat has one. A chat's agent that gives no answer returns its
/// reason, because no other agent may stand in.
pub(crate) async fn classify(
    engine: &Engine,
    forced: Option<&ToolId>,
    text: &str,
) -> std::result::Result<(Intent, String), String> {
    if let Some((intent, reason)) = heuristic(text) {
        return Ok((intent, reason.to_owned()));
    }
    let shot = OneShot {
        tier: Tier::Cheap,
        prompt: classify_prompt(text),
        ceiling_micro_usd: CLASSIFY_CEILING_MICRO_USD,
        timeout: CLASSIFY_TIMEOUT,
        forced: forced.cloned(),
    };
    match ask_once(engine, shot).await {
        Ok(answer) => Ok(router_answer(&answer).unwrap_or_else(unsure)),
        Err(reason) if forced.is_some() => Err(reason),
        Err(_) => Ok(unsure()),
    }
}

fn classify_prompt(text: &str) -> String {
    format!(
        "Classify the message below as one of these types: chat (a greeting or small talk), \
         ask (a question about the project), research (read-only investigation), plan (a plan \
         only), change (add, fix, refactor, or write files).\n\
         Reply with one line of JSON and nothing else: \
         {{\"intent\":\"<type>\",\"reason\":\"<short reason>\"}}\n\nMessage:\n{text}"
    )
}

#[derive(Deserialize)]
struct RouterAnswer {
    intent: String,
    reason: String,
}

/// The type and reason from the router's answer. `None` when the answer names no type.
fn router_answer(text: &str) -> Option<(Intent, String)> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    let answer: RouterAnswer = serde_json::from_str(text.get(start..=end)?).ok()?;
    let intent = Intent::parse(&answer.intent)?;
    let reason = answer.reason.trim();
    if reason.is_empty() {
        return Some((intent, "The router picked this type.".to_owned()));
    }
    Some((intent, reason.chars().take(REASON_CHARS).collect()))
}

/// The answer when the type is not clear: a question, never a change.
fn unsure() -> (Intent, String) {
    (Intent::Ask, UNSURE_REASON.to_owned())
}

/// A type that writes nothing waits behind a plan card. A change keeps the owner's mode.
pub(crate) fn permission_for(intent: Option<Intent>, mode: PermissionMode) -> PermissionMode {
    match intent {
        Some(intent) if !intent.writes() => PermissionMode::PlanOnly,
        Some(_) | None => mode,
    }
}

impl Engine {
    /// The type step of a run. A message gets its type, from the request or from the classifier.
    /// A question, a chat, or research is answered here, and no run starts. A plan or a change
    /// returns its type, and the run goes on with it.
    pub(crate) async fn type_step(
        &self,
        request: &mut RunRequest,
        source: &RunSource,
        caller: &Caller,
    ) -> Result<Typing> {
        if source.words.is_none() {
            return Ok(Typing::Run(None));
        }
        let forced = self.chat_agent(&source.chat)?;
        let asker = Asker {
            caller,
            forced: forced.as_ref(),
        };
        let typed = match self.typed_for(request, source, forced.as_ref()).await {
            Ok(typed) => typed,
            Err(reason) => {
                self.answer_halted(source, &asker, &reason)?;
                return Ok(Typing::Answered);
            }
        };
        if matches!(typed.intent, Intent::Chat | Intent::Ask | Intent::Research) {
            self.answer_without_run(source, &asker, &typed).await?;
            return Ok(Typing::Answered);
        }
        request.intent = Some(typed.intent);
        Ok(Typing::Run(Some(typed)))
    }

    async fn typed_for(
        &self,
        request: &RunRequest,
        source: &RunSource,
        forced: Option<&ToolId>,
    ) -> std::result::Result<Typed, String> {
        if let Some(intent) = request.intent {
            return Ok(Typed {
                intent,
                reason: "You picked this type.".to_owned(),
                by: "you",
            });
        }
        let text = source.words.as_deref().unwrap_or_default();
        let (intent, reason) = classify(self, forced, text).await?;
        Ok(Typed {
            intent,
            reason,
            by: "router",
        })
    }

    /// Put the chosen type on the stream, in the chat of the message. Nothing is said when no type
    /// was chosen, which is a backlog or task run.
    pub(crate) fn announce_intent(&self, chat: &ChatId, typed: Option<&Typed>) -> Result<()> {
        let Some(typed) = typed else {
            return Ok(());
        };
        self.emit_in(
            Some(chat),
            &Event::IntentChosen {
                chat: chat.clone(),
                intent: typed.intent,
                reason: typed.reason.clone(),
                by: typed.by.to_owned(),
            },
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_greeting_is_chat() {
        for text in [
            "hi",
            "Hello!",
            "thanks",
            "thank you",
            "good morning",
            "yo there",
        ] {
            assert_eq!(
                heuristic(text).map(|found| found.0),
                Some(Intent::Chat),
                "{text}"
            );
        }
    }

    #[test]
    fn a_greeting_that_names_a_change_is_not_chat() {
        assert_eq!(heuristic("hey fix it").map(|f| f.0), Some(Intent::Change));
    }

    #[test]
    fn a_word_that_only_starts_like_a_greeting_is_not_chat() {
        assert_eq!(heuristic("history of the ledger"), None);
    }

    #[test]
    fn plan_research_and_question_words_pick_their_type() {
        let cases = [
            ("plan the health check", Intent::Plan),
            ("research the retry logic", Intent::Research),
            ("compare the two queues", Intent::Research),
            ("look into the outage", Intent::Research),
            ("what does the scheduler do", Intent::Ask),
            ("does the ledger round?", Intent::Ask),
            ("how is the cache built", Intent::Ask),
            ("is it done?", Intent::Ask),
            ("show me the stream", Intent::Ask),
        ];
        for (text, intent) in cases {
            assert_eq!(heuristic(text).map(|f| f.0), Some(intent), "{text}");
        }
    }

    #[test]
    fn a_change_verb_picks_change_even_with_a_question_mark() {
        assert_eq!(
            heuristic("Add a health check").map(|f| f.0),
            Some(Intent::Change)
        );
        assert_eq!(
            heuristic("Refactor the ledger?").map(|f| f.0),
            Some(Intent::Change)
        );
        assert_eq!(
            heuristic("fix the typo in readme").map(|f| f.0),
            Some(Intent::Change)
        );
    }

    #[test]
    fn a_text_no_rule_matches_has_no_type() {
        assert_eq!(heuristic("isolate the tests"), None);
        assert_eq!(heuristic("address the warnings"), None);
        assert_eq!(heuristic("   "), None);
    }

    #[test]
    fn a_router_answer_gives_its_type_and_reason() {
        let answer = r#"{"intent":"ask","reason":"It asks how a thing works."}"#;
        assert_eq!(
            router_answer(answer),
            Some((Intent::Ask, "It asks how a thing works.".to_owned()))
        );
    }

    #[test]
    fn a_router_answer_inside_prose_is_still_read() {
        let answer = "Sure: {\"intent\":\"research\",\"reason\":\"Needs a survey.\"} done";
        assert_eq!(
            router_answer(answer).map(|found| found.0),
            Some(Intent::Research)
        );
    }

    #[test]
    fn a_router_answer_with_no_known_type_is_refused() {
        assert_eq!(router_answer(r#"{"intent":"dance","reason":"x"}"#), None);
        assert_eq!(router_answer("not json at all"), None);
        assert_eq!(router_answer(""), None);
    }

    #[test]
    fn an_unclear_answer_is_a_question_and_never_a_change() {
        let (intent, reason) = unsure();
        assert_eq!(intent, Intent::Ask);
        assert_eq!(reason, "Not sure what you want. Say /fix to change files.");
    }

    #[test]
    fn a_plan_or_change_keeps_the_owner_mode_only_for_a_change() {
        assert_eq!(
            permission_for(Some(Intent::Plan), PermissionMode::Bypass),
            PermissionMode::PlanOnly
        );
        assert_eq!(
            permission_for(Some(Intent::Change), PermissionMode::Bypass),
            PermissionMode::Bypass
        );
        assert_eq!(
            permission_for(None, PermissionMode::AskEachWave),
            PermissionMode::AskEachWave
        );
    }
}
