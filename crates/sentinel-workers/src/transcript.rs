//! The messages of the `OpenRouter` loop, and their compaction. Summaries are built here from the
//! tool calls alone, so compaction costs no model call.

use serde_json::{Value, json};

use crate::agent_tools::ToolCall;
use crate::context::{ContextLimits, compaction_split, estimate_tokens, needs_compaction};

const SUMMARY_HEADER: &str = "Earlier steps, summarized:";
/// Longest summary line in characters. Keeps one long path from using the whole summary budget.
const LINE_CHARS: usize = 160;

/// The conversation sent to the model. The head (system prompt and task prompt) is never
/// dropped. A step is one model reply and the tool results that follow it, so compaction never
/// splits a reply from its results.
pub struct Transcript {
    head: Vec<Value>,
    summary_lines: Vec<String>,
    steps: Vec<Vec<Value>>,
}

impl Transcript {
    pub fn new(system_prompt: &str, task_prompt: &str) -> Self {
        Self {
            head: vec![
                json!({"role": "system", "content": system_prompt}),
                json!({"role": "user", "content": task_prompt}),
            ],
            summary_lines: Vec::new(),
            steps: Vec::new(),
        }
    }

    /// Open a step with the model's reply.
    pub fn start_step(&mut self, reply: Value) {
        self.steps.push(vec![reply]);
    }

    /// Add a tool result to the open step. Results only follow a reply, so a step is open.
    pub fn push_result(&mut self, result: Value) {
        if let Some(step) = self.steps.last_mut() {
            step.push(result);
        }
    }

    /// Every message to send, in order.
    pub fn messages(&self) -> Vec<Value> {
        let mut messages = self.head.clone();
        if !self.summary_lines.is_empty() {
            messages.push(json!({"role": "system", "content": self.summary_text()}));
        }
        messages.extend(self.steps.iter().flatten().cloned());
        messages
    }

    /// Estimated tokens of the whole conversation.
    pub fn used_tokens(&self) -> u32 {
        self.messages()
            .iter()
            .map(|message| estimate_tokens(&message.to_string()))
            .fold(0, u32::saturating_add)
    }

    /// Replace the old steps with a summary when the window is nearly full. The newest steps
    /// stay. Returns true when it compacted.
    pub fn compact_if_needed(&mut self, window_tokens: u32, limits: &ContextLimits) -> bool {
        if !needs_compaction(self.used_tokens(), window_tokens, limits) {
            return false;
        }
        let split = compaction_split(self.steps.len(), limits);
        if split == 0 {
            return false;
        }
        for step in self.steps.drain(..split) {
            self.summary_lines.extend(step_lines(&step));
        }
        self.trim_summary(limits);
        true
    }

    /// Drop the oldest summary lines until the summary fits its token budget.
    fn trim_summary(&mut self, limits: &ContextLimits) {
        while self.summary_lines.len() > 1
            && estimate_tokens(&self.summary_text()) > limits.compacted_summary_tokens
        {
            self.summary_lines.remove(0);
        }
    }

    fn summary_text(&self) -> String {
        format!("{SUMMARY_HEADER}\n{}", self.summary_lines.join("\n"))
    }
}

/// One line per tool call in a step: the tool name and the path or command it used.
fn step_lines(step: &[Value]) -> Vec<String> {
    step.iter()
        .filter_map(|message| message["tool_calls"].as_array())
        .flatten()
        .map(|entry| {
            let line = ToolCall::from_reply(entry).label();
            line.chars().take(LINE_CHARS).collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: u32 = 1_000;
    const TASK: &str = "Fix the parser.";

    fn push_step(transcript: &mut Transcript, path: &str, output: &str) {
        let arguments = json!({"path": path}).to_string();
        transcript.start_step(json!({
            "role": "assistant",
            "content": null,
            "tool_calls": [{"id": "call", "type": "function",
                "function": {"name": "read_file", "arguments": arguments}}],
        }));
        transcript.push_result(json!({"role": "tool", "tool_call_id": "call", "content": output}));
    }

    fn summary_present(transcript: &Transcript) -> bool {
        transcript.messages().iter().any(|message| {
            message["content"]
                .as_str()
                .is_some_and(|text| text.starts_with(SUMMARY_HEADER))
        })
    }

    #[test]
    fn transcript_stays_under_the_window_after_compaction() {
        let limits = ContextLimits::default();
        let mut transcript = Transcript::new(&"system ".repeat(20), TASK);
        let output = "x".repeat(200);
        for index in 0..40 {
            push_step(&mut transcript, &format!("src/file_{index}.rs"), &output);
            transcript.compact_if_needed(WINDOW, &limits);
            assert!(
                transcript.used_tokens() <= WINDOW,
                "step {index} used {} tokens",
                transcript.used_tokens()
            );
        }
        assert!(summary_present(&transcript));
    }

    #[test]
    fn task_prompt_is_never_dropped_by_compaction() {
        let limits = ContextLimits::default();
        let mut transcript = Transcript::new("system", TASK);
        let output = "x".repeat(200);
        for index in 0..40 {
            push_step(&mut transcript, &format!("src/file_{index}.rs"), &output);
            transcript.compact_if_needed(WINDOW, &limits);
        }
        let messages = transcript.messages();
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], TASK);
    }

    #[test]
    fn summary_names_the_tools_of_the_steps_it_replaced() {
        let limits = ContextLimits::default();
        let mut transcript = Transcript::new("system", TASK);
        let output = "x".repeat(200);
        let window = 300;
        for index in 0..6 {
            push_step(&mut transcript, &format!("src/file_{index}.rs"), &output);
            transcript.compact_if_needed(window, &limits);
        }
        let summary = transcript
            .messages()
            .iter()
            .filter_map(|message| message["content"].as_str().map(str::to_string))
            .find(|text| text.starts_with(SUMMARY_HEADER))
            .unwrap_or_default();
        assert!(summary.contains("read_file src/file_0.rs"));
        assert!(!summary.contains("src/file_5.rs"));
    }

    #[test]
    fn short_transcript_is_not_compacted() {
        let limits = ContextLimits::default();
        let mut transcript = Transcript::new("system", TASK);
        push_step(&mut transcript, "src/a.rs", "small");
        assert!(!transcript.compact_if_needed(WINDOW, &limits));
        assert!(!summary_present(&transcript));
    }
}
