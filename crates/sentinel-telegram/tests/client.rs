#![allow(clippy::unwrap_used, reason = "tests fail loudly on setup errors")]

use sentinel_telegram::{BotClient, Button, TelegramError};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = "123456789:AAFakeTokenValueForTestsOnly_abcdef";

fn client_for(server: &MockServer) -> BotClient {
    BotClient::new(&server.uri(), TOKEN)
}

fn bot_path(method_name: &str) -> String {
    format!("/bot{TOKEN}/{method_name}")
}

#[tokio::test]
async fn get_me_returns_the_bot_identity_when_telegram_answers_ok() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(bot_path("getMe")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ok": true,
            "result": { "id": 42, "is_bot": true, "first_name": "Sentinel", "username": "sentinel_bot" }
        })))
        .mount(&server)
        .await;

    let me = client_for(&server).get_me().await.unwrap();

    assert_eq!(me.id, 42);
    assert_eq!(me.username.as_deref(), Some("sentinel_bot"));
}

#[tokio::test]
async fn a_429_with_retry_after_becomes_rate_limited_with_those_seconds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(bot_path("sendMessage")))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({
            "ok": false,
            "error_code": 429,
            "description": "Too Many Requests: retry after 7",
            "parameters": { "retry_after": 7 }
        })))
        .mount(&server)
        .await;

    let error = client_for(&server)
        .send_message(1, "hello", None)
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TelegramError::RateLimited {
            retry_after_seconds: 7
        }
    );
}

#[tokio::test]
async fn an_updates_page_maps_messages_and_callback_queries() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(bot_path("getUpdates")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ok": true,
            "result": [
                {
                    "update_id": 10,
                    "message": {
                        "message_id": 5,
                        "from": { "id": 99, "is_bot": false, "first_name": "Blair" },
                        "chat": { "id": 99, "type": "private" },
                        "text": "/status"
                    }
                },
                {
                    "update_id": 11,
                    "callback_query": {
                        "id": "cb-1",
                        "from": { "id": 99, "is_bot": false, "first_name": "Blair" },
                        "message": { "message_id": 6, "chat": { "id": 99, "type": "private" } },
                        "data": "tok123"
                    }
                }
            ]
        })))
        .mount(&server)
        .await;

    let updates = client_for(&server).get_updates(10, 0).await.unwrap();

    assert_eq!(updates.len(), 2);
    assert_eq!(updates[0].update_id, 10);
    let message = updates[0].message.as_ref().unwrap();
    assert_eq!(message.from_id, 99);
    assert_eq!(message.chat_id, 99);
    assert_eq!(message.text.as_deref(), Some("/status"));
    let query = updates[1].callback_query.as_ref().unwrap();
    assert_eq!(query.id, "cb-1");
    assert_eq!(query.message_id, 6);
    assert_eq!(query.data.as_deref(), Some("tok123"));
}

#[tokio::test]
async fn send_message_returns_the_new_message_id_and_sends_the_keyboard() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(bot_path("sendMessage")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ok": true,
            "result": { "message_id": 77, "date": 0, "chat": { "id": 99, "type": "private" } }
        })))
        .mount(&server)
        .await;
    let keyboard = vec![vec![Button {
        label: "Allow".to_owned(),
        data: "tok".to_owned(),
    }]];

    let id = client_for(&server)
        .send_message(99, "Approve?", Some(&keyboard))
        .await
        .unwrap();

    assert_eq!(id, 77);
    let requests = server.received_requests().await.unwrap();
    let body: serde_json::Value = requests[0].body_json().unwrap();
    assert_eq!(
        body["reply_markup"]["inline_keyboard"][0][0]["callback_data"],
        "tok"
    );
}

#[tokio::test]
async fn the_token_is_not_in_the_debug_or_display_of_an_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(bot_path("getMe")))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "ok": false,
            "description": format!("Unauthorized: bad token {TOKEN}")
        })))
        .mount(&server)
        .await;

    let error = client_for(&server).get_me().await.unwrap_err();

    assert!(!format!("{error}").contains(TOKEN), "{error}");
    assert!(!format!("{error:?}").contains(TOKEN), "{error:?}");
    assert!(matches!(error, TelegramError::Api(_)));
}

#[test]
fn the_token_is_not_in_the_debug_of_the_client() {
    let client = BotClient::new("https://api.telegram.org", TOKEN);

    assert_eq!(format!("{client:?}"), r#"BotClient { token: "[hidden]" }"#);
}

#[tokio::test]
async fn the_token_is_not_in_a_transport_error() {
    let client = BotClient::new("http://127.0.0.1:1", TOKEN);

    let error = client.get_me().await.unwrap_err();

    assert!(matches!(error, TelegramError::Transport(_)), "{error:?}");
    assert!(!format!("{error}").contains(TOKEN), "{error}");
}
