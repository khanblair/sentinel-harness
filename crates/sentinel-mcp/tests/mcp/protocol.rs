use serde_json::json;

use super::support::{exchange, request};

#[test]
fn initialize_then_tools_list_offers_the_seven_tools() {
    let replies = exchange(
        &[],
        &[
            request(1, "initialize", json!({})),
            request(2, "tools/list", json!({})),
        ],
    );
    assert_eq!(replies[0]["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "sentinel");
    let names: Vec<&str> = replies[1]["result"]["tools"]
        .as_array()
        .expect("tools is a list")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert_eq!(
        names,
        [
            "read_file",
            "list_dir",
            "write_file",
            "request_files",
            "brain_search",
            "brain_read",
            "brain_propose",
        ]
    );
}

#[test]
fn a_malformed_line_gets_a_parse_error_and_the_server_keeps_going() {
    let replies = exchange(
        &[],
        &["not json at all".to_owned(), request(7, "ping", json!({}))],
    );
    assert_eq!(replies[0]["error"]["code"], -32700);
    assert_eq!(replies[1]["id"], 7);
    assert_eq!(replies[1]["result"], json!({}));
}

#[test]
fn a_notification_gets_no_reply() {
    let notification = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    let replies = exchange(
        &[],
        &[notification.to_string(), request(3, "ping", json!({}))],
    );
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["id"], 3);
}
