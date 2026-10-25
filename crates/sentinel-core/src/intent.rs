//! What a message asks for. The type decides what the agent may do, and the agent is told its type.

use serde::{Deserialize, Serialize};

/// The type of a message. Only `Change` may write files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    /// A greeting or small talk.
    Chat,
    /// A question about the project.
    Ask,
    /// Read-only investigation.
    Research,
    /// A plan only. Nothing runs until the owner approves it.
    Plan,
    /// A request to add, fix, refactor, or write. A real run.
    Change,
}

impl Intent {
    /// The short name shown in the stream.
    pub fn label(self) -> &'static str {
        match self {
            Intent::Chat => "chat",
            Intent::Ask => "ask",
            Intent::Research => "research",
            Intent::Plan => "plan",
            Intent::Change => "change",
        }
    }

    /// The one sentence the agent reads about its type.
    pub fn rule(self) -> &'static str {
        match self {
            Intent::Chat => {
                "This is a greeting or small talk. Answer in one or two sentences. Do not read or change any file."
            }
            Intent::Ask => {
                "This is a question. Read what you need and answer in plain words. Change no file."
            }
            Intent::Research => {
                "This is research. Read what you need, compare, and give findings with the evidence. Change no file."
            }
            Intent::Plan => {
                "This is a planning request. Make a plan of tasks with the files each one needs. Write nothing."
            }
            Intent::Change => {
                "This is a request to change the project. Do the task inside the files reserved for you."
            }
        }
    }

    /// Only a change may write files.
    pub fn writes(self) -> bool {
        self == Intent::Change
    }

    /// The type named by its label, in any letter case. `None` for any other word.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "chat" => Some(Intent::Chat),
            "ask" => Some(Intent::Ask),
            "research" => Some(Intent::Research),
            "plan" => Some(Intent::Plan),
            "change" => Some(Intent::Change),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_type_has_its_label() {
        let labels: Vec<&str> = [
            Intent::Chat,
            Intent::Ask,
            Intent::Research,
            Intent::Plan,
            Intent::Change,
        ]
        .into_iter()
        .map(Intent::label)
        .collect();
        assert_eq!(labels, ["chat", "ask", "research", "plan", "change"]);
    }

    #[test]
    fn a_chat_rule_forbids_reading_and_changing_files() {
        assert_eq!(
            Intent::Chat.rule(),
            "This is a greeting or small talk. Answer in one or two sentences. Do not read or change any file."
        );
    }

    #[test]
    fn a_question_rule_says_change_no_file() {
        assert!(Intent::Ask.rule().ends_with("Change no file."));
    }

    #[test]
    fn a_research_rule_asks_for_evidence() {
        assert!(
            Intent::Research
                .rule()
                .contains("findings with the evidence")
        );
    }

    #[test]
    fn a_plan_rule_says_write_nothing() {
        assert!(Intent::Plan.rule().ends_with("Write nothing."));
    }

    #[test]
    fn a_change_rule_names_the_reserved_files() {
        assert!(Intent::Change.rule().contains("files reserved for you"));
    }

    #[test]
    fn only_a_change_may_write() {
        let writing: Vec<Intent> = [
            Intent::Chat,
            Intent::Ask,
            Intent::Research,
            Intent::Plan,
            Intent::Change,
        ]
        .into_iter()
        .filter(|intent| intent.writes())
        .collect();
        assert_eq!(writing, [Intent::Change]);
    }

    #[test]
    fn parse_takes_any_letter_case_and_surrounding_space() {
        assert_eq!(Intent::parse(" Research "), Some(Intent::Research));
        assert_eq!(Intent::parse("CHANGE"), Some(Intent::Change));
    }

    #[test]
    fn parse_refuses_a_word_that_is_no_type() {
        assert_eq!(Intent::parse("dance"), None);
        assert_eq!(Intent::parse(""), None);
    }

    #[test]
    fn a_type_is_stored_as_its_snake_case_name() {
        let json = serde_json::to_string(&Intent::Research).unwrap();
        assert_eq!(json, "\"research\"");
        assert_eq!(
            serde_json::from_str::<Intent>("\"plan\"").unwrap(),
            Intent::Plan
        );
    }
}
