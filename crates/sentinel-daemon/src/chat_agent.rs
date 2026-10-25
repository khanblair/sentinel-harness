//! The agent a chat uses. A new chat takes the project default when that tool is ready. A chat
//! may change its agent between runs, never during one.

use sentinel_core::chat::AUTO_AGENT;
use sentinel_core::{ChatId, Event, ToolInfo};

use crate::controls::Caller;
use crate::engine::{Engine, EngineError, Result};

/// The refusal for an agent that cannot take messages. The UI shows it as it is.
const NOT_READY_AGENT: &str = "That agent is not ready. Pick a ready agent in Settings, Agents.";

impl Engine {
    /// The agent a new chat of this project starts with. The project default counts only while
    /// its tool is still ready and on. Otherwise the chat starts on auto.
    pub(crate) fn starting_agent(&self) -> Result<Option<String>> {
        let Some(agent) = self.config().default_agent else {
            return Ok(None);
        };
        let usable = self
            .store()
            .list_tools()?
            .iter()
            .any(|tool| tool.id.as_str() == agent && usable_agent(tool));
        Ok(usable.then_some(agent))
    }

    /// A chat asks for a new agent. It is refused during the chat's live run, then checked.
    pub(crate) fn agent_request(&self, id: &ChatId, value: &str) -> Result<Option<ToolInfo>> {
        self.check_agent_change(id)?;
        self.agent_choice(value)
    }

    /// The tool a chat or the project may use, or `None` for auto. Only a ready, enabled tool
    /// passes, so a refused choice never reaches the store.
    fn agent_choice(&self, value: &str) -> Result<Option<ToolInfo>> {
        let value = value.trim();
        if value.is_empty() || value == AUTO_AGENT {
            return Ok(None);
        }
        let found = self
            .store()
            .list_tools()?
            .into_iter()
            .find(|tool| tool.id.as_str() == value);
        match found {
            Some(tool) if usable_agent(&tool) => Ok(Some(tool)),
            _ => Err(EngineError::Invalid(NOT_READY_AGENT.to_owned())),
        }
    }

    /// Sets the agent that new chats of this project start with. `auto` clears it.
    pub fn set_default_agent(&self, value: &str, caller: &Caller) -> Result<()> {
        let agent = self.agent_choice(value)?;
        let name = agent
            .as_ref()
            .map_or_else(|| AUTO_AGENT.to_owned(), |tool| tool.name.clone());
        let saved = agent.map(|tool| tool.id.as_str().to_owned());
        self.update_config(|config| config.default_agent = saved)?;
        self.note_device_action(caller, &format!("set the default agent to {name}"));
        Ok(())
    }

    /// Tells the stream which agent the chat uses from now on.
    pub(crate) fn emit_agent_note(&self, id: &ChatId, agent: Option<&ToolInfo>) -> Result<()> {
        let text = match agent {
            Some(tool) => format!("This chat now uses {}.", tool.name),
            None => "This chat now picks the agent for each task.".to_owned(),
        };
        self.emit_in(
            Some(id),
            &Event::Message {
                from: "sentinel".to_owned(),
                to: None,
                text,
            },
        )?;
        Ok(())
    }

    /// A chat keeps its agent for the whole of its live run.
    fn check_agent_change(&self, id: &ChatId) -> Result<()> {
        if self.is_chat_live(id) {
            return Err(EngineError::Refused("Stop the run first.".to_owned()));
        }
        Ok(())
    }
}

/// Only a ready tool that the user left switched on can take a chat's messages.
fn usable_agent(tool: &ToolInfo) -> bool {
    tool.status.is_ready() && tool.enabled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::SystemRunner;
    use crate::engine::EngineOptions;
    use crate::secrets::MemoryStore;
    use sentinel_core::api::UpdateChat;
    use sentinel_core::{Tier, ToolId, ToolStatus};
    use std::sync::Arc;

    fn open_in_memory(repo: &std::path::Path) -> Engine {
        Engine::open(EngineOptions {
            repo_root: repo.to_path_buf(),
            home: repo.to_path_buf(),
            runner: Arc::new(SystemRunner),
            secrets: Arc::new(MemoryStore::new()),
            fake_agents: None,
            in_memory: true,
        })
        .unwrap()
    }

    fn tool(id: &str, status: ToolStatus) -> ToolInfo {
        ToolInfo {
            id: ToolId::new(id),
            name: format!("{id} CLI"),
            path: format!("/usr/bin/{id}"),
            version: "1.0".to_owned(),
            adapter: id.to_owned(),
            status,
            tier: Tier::Mid,
            enabled: true,
        }
    }

    fn agent_update(value: &str) -> UpdateChat {
        UpdateChat {
            agent: Some(value.to_owned()),
            ..UpdateChat::default()
        }
    }

    #[test]
    fn a_new_chat_starts_with_the_project_default_agent() {
        let dir = tempfile::tempdir().unwrap();
        let engine = open_in_memory(dir.path());
        engine
            .store()
            .upsert_tool(&tool("claude", ToolStatus::Ready))
            .unwrap();
        engine
            .set_default_agent("claude", &Caller::pc("You"))
            .unwrap();
        let chat = engine
            .create_chat(Some("Rate limit"), &Caller::pc("You"))
            .unwrap();
        assert_eq!(chat.agent, Some(ToolId::new("claude")));
    }

    #[test]
    fn a_new_chat_starts_on_auto_when_the_default_tool_is_not_ready() {
        let dir = tempfile::tempdir().unwrap();
        let engine = open_in_memory(dir.path());
        engine
            .store()
            .upsert_tool(&tool("claude", ToolStatus::Ready))
            .unwrap();
        engine
            .set_default_agent("claude", &Caller::pc("You"))
            .unwrap();
        engine
            .store()
            .upsert_tool(&tool("claude", ToolStatus::Missing))
            .unwrap();
        let chat = engine
            .create_chat(Some("Rate limit"), &Caller::pc("You"))
            .unwrap();
        assert_eq!(chat.agent, None);
    }

    #[test]
    fn update_sets_then_clears_the_agent_of_a_chat() {
        let dir = tempfile::tempdir().unwrap();
        let engine = open_in_memory(dir.path());
        engine
            .store()
            .upsert_tool(&tool("claude", ToolStatus::Ready))
            .unwrap();
        let chat = engine
            .create_chat(Some("Rate limit"), &Caller::pc("You"))
            .unwrap();
        let set = engine
            .update_chat(&chat.id, &agent_update("claude"), &Caller::pc("You"))
            .unwrap();
        assert_eq!(set.agent, Some(ToolId::new("claude")));
        let cleared = engine
            .update_chat(&chat.id, &agent_update("auto"), &Caller::pc("You"))
            .unwrap();
        assert_eq!(cleared.agent, None);
    }

    #[test]
    fn an_unknown_or_not_ready_agent_is_refused_and_nothing_changes() {
        let dir = tempfile::tempdir().unwrap();
        let engine = open_in_memory(dir.path());
        engine
            .store()
            .upsert_tool(&tool("gemini", ToolStatus::NeedsLogin))
            .unwrap();
        let chat = engine
            .create_chat(Some("Rate limit"), &Caller::pc("You"))
            .unwrap();
        for value in ["ghost", "gemini"] {
            let error = engine
                .update_chat(&chat.id, &agent_update(value), &Caller::pc("You"))
                .unwrap_err();
            assert!(matches!(error, EngineError::Invalid(_)), "{value}");
        }
        assert_eq!(engine.get_chat(&chat.id).unwrap().agent, None);
    }

    #[test]
    fn the_default_agent_refuses_a_tool_that_is_not_ready() {
        let dir = tempfile::tempdir().unwrap();
        let engine = open_in_memory(dir.path());
        let error = engine
            .set_default_agent("ghost", &Caller::pc("You"))
            .unwrap_err();
        assert!(matches!(error, EngineError::Invalid(_)));
        assert_eq!(engine.config().default_agent, None);
    }
}
