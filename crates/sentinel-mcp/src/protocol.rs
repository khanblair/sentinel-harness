//! The JSON-RPC 2.0 messages MCP needs, and the tool list. Replies are built as `serde_json`
//! values so every reply has the same shape.

use serde_json::{Map, Value, json};

use crate::brain_tools;

pub const PROTOCOL_VERSION: &str = "2024-11-05";
pub const PARSE_ERROR: i64 = -32700;
pub const METHOD_NOT_FOUND: i64 = -32601;

pub fn response(id: &Value, result: &Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

pub fn error_response(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// A tool answer. A refusal is an answer too: the model reads the text and adjusts.
pub fn tool_result(text: &str, is_error: bool) -> Value {
    json!({"content": [{"type": "text", "text": text}], "isError": is_error})
}

pub fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {"tools": {}},
        "serverInfo": {"name": "sentinel", "version": env!("CARGO_PKG_VERSION")},
    })
}

pub fn tool_definitions() -> Value {
    let mut tools = file_tools();
    tools.extend(brain_tool_definitions());
    Value::Array(tools)
}

fn file_tools() -> Vec<Value> {
    vec![
        tool(
            "read_file",
            "Read a text file in the repository.",
            &[("path", "Relative path from the repository root.")],
        ),
        tool(
            "list_dir",
            "List the names in a folder. Folders end with a slash.",
            &[("path", "Relative folder path. Use . for the root.")],
        ),
        tool(
            "write_file",
            "Write a file. Only paths reserved for this task are accepted.",
            &[
                (
                    "path",
                    "Relative path of a reserved file, or of a file under a reserved folder.",
                ),
                ("content", "The whole new content of the file."),
            ],
        ),
        tool(
            "request_files",
            "Ask for files outside the reservation. Nothing is granted here.",
            &[("paths", "Relative paths, one per line.")],
        ),
    ]
}

fn brain_tool_definitions() -> Vec<Value> {
    let note_types = brain_tools::proposable_types();
    vec![
        tool(
            "brain_search",
            "Search the Brain notes. Returns up to 8 hits.",
            &[("query", "Words to search for.")],
        ),
        tool(
            "brain_read",
            "Read one Brain note by its id.",
            &[("id", "The note id from brain_search.")],
        ),
        tool(
            "brain_propose",
            "Propose a new Brain note. A reviewer decides whether it becomes a note.",
            &[
                ("title", "Short title of the note."),
                ("body", "Markdown body of the note."),
                ("note_type", note_types.as_str()),
            ],
        ),
    ]
}

/// A string argument of a tool call. A missing one is a refusal the model can read.
pub fn argument<'a>(arguments: &'a Value, name: &str) -> Result<&'a str, String> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{name} is required."))
}

/// One tool with string arguments, all required.
fn tool(name: &str, description: &str, arguments: &[(&str, &str)]) -> Value {
    let properties: Map<String, Value> = arguments
        .iter()
        .map(|(key, text)| {
            (
                (*key).to_owned(),
                json!({"type": "string", "description": text}),
            )
        })
        .collect();
    let required: Vec<&str> = arguments.iter().map(|(key, _)| *key).collect();
    json!({
        "name": name,
        "description": description,
        "inputSchema": {"type": "object", "properties": properties, "required": required},
    })
}
