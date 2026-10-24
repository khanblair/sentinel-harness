//! The stdio loop: one JSON-RPC message per line in, one reply per line out.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use sentinel_core::BrainMode;
use serde_json::{Value, json};

use crate::brain_tools::{self, BrainTools};
use crate::files::{self, FileTools};
use crate::protocol::{
    METHOD_NOT_FOUND, PARSE_ERROR, error_response, initialize_result, response, tool_definitions,
    tool_result,
};
use crate::skill_tools::{self, SkillTools};

/// What the server needs from its environment. The agent CLI sets these when it starts the server.
pub struct ServerConfig {
    workdir: PathBuf,
    home: Option<PathBuf>,
    scope: Vec<String>,
    task_id: Option<String>,
    brain_root: Option<PathBuf>,
    brain_mode: BrainMode,
}

impl ServerConfig {
    pub fn from_env() -> Self {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Read the settings through `lookup`, so a test can pass its own values.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            workdir: lookup("SENTINEL_WORKDIR")
                .filter(|path| !path.is_empty())
                .map_or_else(|| PathBuf::from("."), PathBuf::from),
            home: lookup("HOME")
                .filter(|path| !path.is_empty())
                .map(PathBuf::from),
            scope: lookup("SENTINEL_SCOPE")
                .map(|text| scope_entries(&text))
                .unwrap_or_default(),
            task_id: lookup("SENTINEL_TASK_ID").filter(|id| !id.is_empty()),
            brain_root: lookup("SENTINEL_BRAIN")
                .filter(|path| !path.is_empty())
                .map(PathBuf::from),
            brain_mode: brain_tools::parse_mode(lookup("SENTINEL_BRAIN_MODE").as_deref()),
        }
    }
}

/// One entry per line. Blank lines are dropped, so an empty entry never reserves a whole folder.
fn scope_entries(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

struct Server {
    files: FileTools,
    brain: BrainTools,
    skills: SkillTools,
}

impl Server {
    fn new(config: ServerConfig) -> Self {
        let skills = SkillTools::new(&config.workdir, config.home.as_deref());
        Self {
            files: FileTools::new(config.workdir, config.scope),
            brain: BrainTools::new(config.brain_root, config.brain_mode, config.task_id),
            skills,
        }
    }

    /// The reply to one line, or `None` for a notification.
    fn reply(&self, line: &str) -> Option<Value> {
        let Ok(request) = serde_json::from_str::<Value>(line) else {
            return Some(error_response(&Value::Null, PARSE_ERROR, "Parse error."));
        };
        let method = request["method"].as_str().unwrap_or_default();
        if method.starts_with("notifications/") {
            return None;
        }
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        Some(match method {
            "initialize" => response(&id, &initialize_result()),
            "ping" => response(&id, &json!({})),
            "tools/list" => response(&id, &json!({"tools": tool_definitions()})),
            "tools/call" => response(&id, &self.call_tool(&request["params"])),
            _ => error_response(
                &id,
                METHOD_NOT_FOUND,
                &format!("Method not found: {method}."),
            ),
        })
    }

    fn call_tool(&self, params: &Value) -> Value {
        let name = params["name"].as_str().unwrap_or_default();
        let args = &params["arguments"];
        let outcome = if files::TOOL_NAMES.contains(&name) {
            self.files.call(name, args)
        } else if brain_tools::TOOL_NAMES.contains(&name) {
            self.brain.call(name, args)
        } else if skill_tools::TOOL_NAMES.contains(&name) {
            self.skills.call(name, args)
        } else {
            Err(format!("Unknown tool: {name}."))
        };
        match outcome {
            Ok(text) => tool_result(&text, false),
            Err(text) => tool_result(&text, true),
        }
    }
}

/// Serve requests until the input ends. A line that is not JSON gets a parse error and the
/// server keeps going. Bytes that are not UTF-8 are read as replacement characters.
pub fn serve(
    mut input: impl BufRead,
    mut output: impl Write,
    config: ServerConfig,
) -> io::Result<()> {
    let server = Server::new(config);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        if input.read_until(b'\n', &mut buffer)? == 0 {
            return Ok(());
        }
        let line = String::from_utf8_lossy(&buffer);
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(reply) = server.reply(line) {
            writeln!(output, "{reply}")?;
            output.flush()?;
        }
    }
}
