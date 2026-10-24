//! The function definitions the `OpenRouter` worker sends with every request.

use serde_json::{Value, json};

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
