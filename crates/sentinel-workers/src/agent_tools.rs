//! The tools the `OpenRouter` worker offers its model, and the executor that runs them.
//!
//! The executor keeps no state. Each call gets its own `ToolEnv` and returns a `ToolResult`.
//! A refused call comes back as plain text, so the model can read why and adjust.

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const READ_LIMIT_BYTES: usize = 100_000;
const LIST_LIMIT: usize = 200;
const COMMAND_LIMIT: Duration = Duration::from_secs(120);
const COMMAND_TAIL_LINES: usize = 40;
/// Folders the model may not read, list, or write, at any depth.
const HIDDEN_DIRS: [&str; 2] = [".git", ".sentinel"];
const ALLOWED_SUBCOMMANDS: [&str; 4] = ["test", "check", "fmt", "clippy"];
/// Flags that point cargo at another project or write outside the task folder.
const BLOCKED_FLAGS: [&str; 4] = ["--config", "--manifest-path", "--target-dir", "-Z"];
const REFUSED_PATH: &str = "That path is not allowed.";
const REFUSED_COMMAND: &str = "That command is not allowed.";

/// A call the model made: its id, the tool name, and the parsed arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

impl ToolCall {
    /// Read one entry of a reply's `tool_calls`. The API sends the arguments as a JSON string.
    pub fn from_reply(entry: &Value) -> Self {
        let function = &entry["function"];
        let arguments = function["arguments"]
            .as_str()
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .unwrap_or(Value::Null);
        Self {
            id: entry["id"].as_str().unwrap_or_default().to_string(),
            name: function["name"].as_str().unwrap_or_default().to_string(),
            arguments,
        }
    }

    /// Short text for the output stream: the tool name and the path or command it acts on.
    pub fn label(&self) -> String {
        let detail =
            argument(&self.arguments, "path").or_else(|| argument(&self.arguments, "command"));
        match detail {
            Some(detail) => format!("{} {detail}", self.name),
            None => self.name.clone(),
        }
    }
}

/// Where a call runs: the task folder and the paths the task may write.
#[derive(Debug, Clone)]
pub struct ToolEnv {
    pub workdir: PathBuf,
    pub scope: Vec<String>,
}

/// What a tool returns to the model. `finished` is set by `finish`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    pub text: String,
    pub finished: bool,
}

/// The function definitions sent with every request.
pub fn tool_definitions() -> Value {
    json!([
        function(
            "read_file",
            "Read a text file in the task folder. Files over 100 KB are cut.",
            &json!({"path": text_param("File path relative to the task folder.")}),
            &["path"],
        ),
        function(
            "list_dir",
            "List the names in a folder. Folders end with a slash.",
            &json!({"path": text_param("Folder path relative to the task folder. Use . for the root.")}),
            &["path"],
        ),
        function(
            "write_file",
            "Write a file. Only the files in your scope may be written.",
            &json!({
                "path": text_param("File path relative to the task folder."),
                "content": text_param("The whole new content of the file."),
            }),
            &["path", "content"],
        ),
        function(
            "run_command",
            "Run one allowed cargo command: test, check, fmt --check, or clippy.",
            &json!({"command": text_param("The full command, for example: cargo test -p sentinel-workers")}),
            &["command"],
        ),
        function(
            "finish",
            "End the task. Call this when the task is done.",
            &json!({"summary": text_param("One sentence on what was done.")}),
            &["summary"],
        ),
    ])
}

fn function(name: &str, description: &str, properties: &Value, required: &[&str]) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": {"type": "object", "properties": properties, "required": required},
        },
    })
}

fn text_param(description: &str) -> Value {
    json!({"type": "string", "description": description})
}

/// Run one tool call. Errors come back as text, never as a Rust error.
pub fn run_tool(call: &ToolCall, env: &ToolEnv) -> ToolResult {
    let args = &call.arguments;
    let outcome = match call.name.as_str() {
        "read_file" => read_file(env, args),
        "list_dir" => list_dir(env, args),
        "write_file" => write_file(env, args),
        "run_command" => run_command(env, args),
        "finish" => {
            let summary = argument(args, "summary").unwrap_or_default();
            return ToolResult {
                text: format!("Finished: {summary}"),
                finished: true,
            };
        }
        other => Err(format!("Unknown tool: {other}.")),
    };
    ToolResult {
        text: outcome.unwrap_or_else(|error| error),
        finished: false,
    }
}

fn argument<'a>(arguments: &'a Value, name: &str) -> Option<&'a str> {
    arguments.get(name)?.as_str()
}

/// A path from the model as a clean relative path. `..`, a leading `/`, or a drive prefix is
/// refused, so no path can name a place outside the task folder by its text.
fn relative_path(raw: &str) -> Option<PathBuf> {
    let mut clean = PathBuf::new();
    for component in Path::new(raw).components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(clean)
}

fn is_hidden(relative: &Path) -> bool {
    relative.components().any(|part| {
        part.as_os_str()
            .to_str()
            .is_some_and(|name| HIDDEN_DIRS.contains(&name))
    })
}

/// True when the path, or its deepest existing part, resolves inside the workdir. A symlink at
/// the path itself is refused, so a write cannot go through one, even a dangling one.
fn resolves_inside(workdir: &Path, target: &Path) -> bool {
    let Ok(root) = workdir.canonicalize() else {
        return false;
    };
    if target
        .symlink_metadata()
        .is_ok_and(|meta| meta.file_type().is_symlink())
    {
        return false;
    }
    let mut probe = target;
    loop {
        if let Ok(resolved) = probe.canonicalize() {
            return resolved.starts_with(&root);
        }
        let Some(parent) = probe.parent() else {
            return false;
        };
        probe = parent;
    }
}

fn read_target(env: &ToolEnv, raw: &str) -> Result<PathBuf, String> {
    let relative = relative_path(raw).ok_or_else(|| REFUSED_PATH.to_string())?;
    let target = env.workdir.join(&relative);
    if is_hidden(&relative) || !resolves_inside(&env.workdir, &target) {
        return Err(REFUSED_PATH.to_string());
    }
    Ok(target)
}

fn read_file(env: &ToolEnv, args: &Value) -> Result<String, String> {
    let target = read_target(env, argument(args, "path").unwrap_or_default())?;
    let bytes = fs::read(&target).map_err(|error| format!("Cannot read the file: {error}."))?;
    let text = String::from_utf8(bytes).map_err(|_| "That file is not text.".to_string())?;
    Ok(cut_at_limit(&text))
}

fn cut_at_limit(text: &str) -> String {
    if text.len() <= READ_LIMIT_BYTES {
        return text.to_string();
    }
    let end = (0..=READ_LIMIT_BYTES)
        .rev()
        .find(|&index| text.is_char_boundary(index))
        .unwrap_or(0);
    format!("{}\n[... file cut at 100 KB ...]", &text[..end])
}

fn list_dir(env: &ToolEnv, args: &Value) -> Result<String, String> {
    let folder = read_target(env, argument(args, "path").unwrap_or("."))?;
    let entries =
        fs::read_dir(&folder).map_err(|error| format!("Cannot list the folder: {error}."))?;
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry_name(&entry))
        .collect();
    names.sort();
    let total = names.len();
    names.truncate(LIST_LIMIT);
    if total > LIST_LIMIT {
        names.push(format!("[... {} names cut ...]", total - LIST_LIMIT));
    }
    Ok(names.join("\n"))
}

fn entry_name(entry: &fs::DirEntry) -> Option<String> {
    let name = entry.file_name().into_string().ok()?;
    if HIDDEN_DIRS.contains(&name.as_str()) {
        return None;
    }
    let is_folder = entry.file_type().is_ok_and(|kind| kind.is_dir());
    Some(if is_folder { format!("{name}/") } else { name })
}

fn write_file(env: &ToolEnv, args: &Value) -> Result<String, String> {
    let raw = argument(args, "path").unwrap_or_default();
    let content = argument(args, "content").unwrap_or_default();
    let relative = relative_path(raw).ok_or_else(|| REFUSED_PATH.to_string())?;
    if !in_scope(&relative, &env.scope) {
        return Err(not_reserved(raw, &env.scope));
    }
    let target = env.workdir.join(&relative);
    if is_hidden(&relative) || !resolves_inside(&env.workdir, &target) {
        return Err(REFUSED_PATH.to_string());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Cannot create the folder: {error}."))?;
    }
    fs::write(&target, content).map_err(|error| format!("Cannot write the file: {error}."))?;
    Ok(format!("Wrote {raw} ({} bytes).", content.len()))
}

/// True when the path is a scope entry or lies under a scope folder. An empty entry is ignored,
/// so it cannot reserve the whole folder by accident.
fn in_scope(relative: &Path, scope: &[String]) -> bool {
    scope
        .iter()
        .filter_map(|entry| relative_path(entry))
        .filter(|reserved| !reserved.as_os_str().is_empty())
        .any(|reserved| relative.starts_with(reserved))
}

fn not_reserved(raw: &str, scope: &[String]) -> String {
    let allowed = if scope.is_empty() {
        "nothing".to_string()
    } else {
        scope.join(", ")
    };
    format!("{raw} is not reserved for this task. Write only: {allowed}.")
}

fn run_command(env: &ToolEnv, args: &Value) -> Result<String, String> {
    let command = argument(args, "command").unwrap_or_default();
    let cargo_args = allowed_cargo_args(command).ok_or_else(|| REFUSED_COMMAND.to_string())?;
    run_cargo(&env.workdir, &cargo_args)
}

/// The cargo arguments for an allow-listed command, or `None` when the command is not allowed.
fn allowed_cargo_args(command: &str) -> Option<Vec<&str>> {
    let tokens: Vec<&str> = command.split_whitespace().collect();
    let (program, rest) = tokens.split_first()?;
    let (subcommand, extra) = rest.split_first()?;
    let allowed = *program == "cargo"
        && ALLOWED_SUBCOMMANDS.contains(subcommand)
        && (*subcommand != "fmt" || extra.contains(&"--check"))
        && extra_args_allowed(extra);
    allowed.then(|| rest.to_vec())
}

/// Extra arguments must be flags, or `-p NAME` for one package. A bare value is refused.
fn extra_args_allowed(extra: &[&str]) -> bool {
    let mut words = extra.iter();
    while let Some(word) = words.next() {
        let allowed = match *word {
            "-p" => words.next().copied().is_some_and(is_package_name),
            flag => {
                flag.starts_with('-')
                    && !BLOCKED_FLAGS
                        .iter()
                        .any(|blocked| flag.starts_with(blocked))
            }
        };
        if !allowed {
            return false;
        }
    }
    true
}

fn is_package_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn run_cargo(workdir: &Path, args: &[&str]) -> Result<String, String> {
    let mut child = Command::new("cargo")
        .args(args)
        .current_dir(workdir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Cannot start cargo: {error}."))?;
    let (bytes, timed_out) = collect_output(&mut child, COMMAND_LIMIT);
    let status = child.wait().ok().and_then(|status| status.code());
    Ok(command_report(&bytes, status, timed_out))
}

/// Read both pipes on background threads, until both close or the limit passes. A grandchild
/// that keeps a pipe open cannot block the caller past the limit.
fn collect_output(child: &mut Child, limit: Duration) -> (Vec<u8>, bool) {
    let (sender, receiver) = mpsc::channel::<Vec<u8>>();
    if let Some(stdout) = child.stdout.take() {
        pump(stdout, sender.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        pump(stderr, sender.clone());
    }
    drop(sender);
    let deadline = Instant::now() + limit;
    let mut bytes = Vec::new();
    loop {
        match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(chunk) => bytes.extend_from_slice(&chunk),
            Err(RecvTimeoutError::Disconnected) => return (bytes, false),
            Err(RecvTimeoutError::Timeout) => {
                let _ = child.kill();
                return (bytes, true);
            }
        }
    }
}

fn pump(mut pipe: impl Read + Send + 'static, sender: mpsc::Sender<Vec<u8>>) {
    thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        while let Ok(count) = pipe.read(&mut buffer) {
            if count == 0 || sender.send(buffer[..count].to_vec()).is_err() {
                break;
            }
        }
    });
}

fn command_report(bytes: &[u8], status: Option<i32>, timed_out: bool) -> String {
    if timed_out {
        return format!(
            "The command took longer than {} seconds.",
            COMMAND_LIMIT.as_secs()
        );
    }
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text.lines().collect();
    let tail = lines[lines.len().saturating_sub(COMMAND_TAIL_LINES)..].join("\n");
    let code = status.map_or_else(|| "none".to_string(), |code| code.to_string());
    format!("exit code {code}\n{tail}")
}
