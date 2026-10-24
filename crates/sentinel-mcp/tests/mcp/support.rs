//! Helpers shared by the test modules: run the server over lines and read the replies.

use std::collections::HashMap;
use std::io::Cursor;

use sentinel_mcp::{ServerConfig, serve};
use serde_json::{Value, json};

/// Run the server with these settings over these request lines. Returns one value per reply.
pub fn exchange(env: &[(&str, &str)], lines: &[String]) -> Vec<Value> {
    let vars: HashMap<String, String> = env
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    let config = ServerConfig::from_lookup(|key| vars.get(key).cloned());
    let input = format!("{}\n", lines.join("\n"));
    let mut output = Vec::new();
    serve(Cursor::new(input), &mut output, config).expect("the server answers");
    String::from_utf8(output)
        .expect("replies are UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("each reply is JSON"))
        .collect()
}

pub fn request(id: u64, method: &str, params: Value) -> String {
    let mut message = json!({"jsonrpc": "2.0", "id": id, "method": method});
    message["params"] = params;
    message.to_string()
}

pub fn call_tool(id: u64, name: &str, arguments: Value) -> String {
    let mut params = json!({"name": name});
    params["arguments"] = arguments;
    request(id, "tools/call", params)
}

pub fn tool_text(reply: &Value) -> &str {
    reply["result"]["content"][0]["text"]
        .as_str()
        .expect("a tool answer has text")
}

pub fn is_error(reply: &Value) -> bool {
    reply["result"]["isError"] == true
}
