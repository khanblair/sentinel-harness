//! The live stream on `GET /ws`. The first frame must authenticate. Then the client subscribes
//! from a cursor: stored events come first, then live ones, with no gap and no repeat.

use std::time::Duration;

use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use sentinel_core::EventRecord;
use sentinel_core::api::{ClientFrame, ServerFrame};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::auth::caller_for_token;
use crate::engine::Engine;

const AUTH_DEADLINE: Duration = Duration::from_secs(5);
const BACKLOG_PAGE: u32 = 500;
const CLOSE_NORMAL: u16 = 1000;
const CLOSE_POLICY: u16 = 1008;

/// How the socket ended. It decides the close frame the client sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// The first frame was not a valid token. Nothing was sent.
    Refused,
    /// The client fell behind the live buffer. It should reconnect and subscribe again.
    Lagged,
    /// The client left, or the store or bus failed.
    Gone,
}

pub async fn upgrade(ws: WebSocketUpgrade, State(engine): State<Engine>) -> Response {
    ws.on_upgrade(move |socket| serve_socket(socket, engine))
}

async fn serve_socket(mut socket: WebSocket, engine: Engine) {
    let ending = match handshake(&mut socket, &engine).await {
        Some(after) => stream(&mut socket, &engine, after).await,
        None => Ending::Refused,
    };
    close_for(&mut socket, ending).await;
}

async fn close_for(socket: &mut WebSocket, ending: Ending) {
    match ending {
        Ending::Refused => close(socket, CLOSE_POLICY, "Sign in first.").await,
        Ending::Lagged => close(socket, CLOSE_NORMAL, "Reconnect to catch up.").await,
        Ending::Gone => {}
    }
}

/// Reads the auth frame and the subscribe frame. Returns the cursor, or `None` to refuse.
async fn handshake(socket: &mut WebSocket, engine: &Engine) -> Option<i64> {
    let Ok(Some(ClientFrame::Auth { token })) =
        tokio::time::timeout(AUTH_DEADLINE, next_frame(socket)).await
    else {
        return None;
    };
    caller_for_token(engine, token).await?;
    loop {
        match next_frame(socket).await? {
            ClientFrame::Ping if !send_frame(socket, &ServerFrame::Pong).await => return None,
            ClientFrame::Ping => {}
            ClientFrame::Subscribe { after } => return Some(after),
            ClientFrame::Auth { .. } => return None,
        }
    }
}

/// Sends the backlog, then live events. The live channel is opened first, so an event that
/// lands while the backlog is read is kept and sent once.
async fn stream(socket: &mut WebSocket, engine: &Engine, after: i64) -> Ending {
    let mut live = engine.subscribe();
    let mut last_sent = after;
    if let Err(ending) = send_backlog(socket, engine, after, &mut last_sent).await {
        return ending;
    }
    relay_live(socket, &mut live, &mut last_sent).await
}

async fn send_backlog(
    socket: &mut WebSocket,
    engine: &Engine,
    after: i64,
    last_sent: &mut i64,
) -> Result<(), Ending> {
    let mut cursor = after;
    loop {
        let page = backlog_page(engine, cursor).await.ok_or(Ending::Gone)?;
        let Some(last) = page.last() else {
            return Ok(());
        };
        cursor = last.id;
        for record in page {
            if !send_event(socket, last_sent, record).await {
                return Err(Ending::Gone);
            }
        }
    }
}

async fn relay_live(
    socket: &mut WebSocket,
    live: &mut broadcast::Receiver<EventRecord>,
    last_sent: &mut i64,
) -> Ending {
    loop {
        let ending = tokio::select! {
            received = live.recv() => relay_event(socket, last_sent, received).await,
            frame = next_frame(socket) => answer_client(socket, frame).await,
        };
        if let Some(ending) = ending {
            return ending;
        }
    }
}

/// Send a live event the client has not seen. `Some` ends the stream.
async fn relay_event(
    socket: &mut WebSocket,
    last_sent: &mut i64,
    received: Result<EventRecord, RecvError>,
) -> Option<Ending> {
    match received {
        Ok(record) => (is_fresh(record.id, *last_sent)
            && !send_event(socket, last_sent, record).await)
            .then_some(Ending::Gone),
        Err(RecvError::Lagged(_)) => Some(Ending::Lagged),
        Err(RecvError::Closed) => Some(Ending::Gone),
    }
}

/// Answer a frame from the client while events flow. `Some` ends the stream.
async fn answer_client(socket: &mut WebSocket, frame: Option<ClientFrame>) -> Option<Ending> {
    match frame {
        Some(ClientFrame::Ping) => {
            (!send_frame(socket, &ServerFrame::Pong).await).then_some(Ending::Gone)
        }
        Some(_) => None,
        None => Some(Ending::Gone),
    }
}

async fn backlog_page(engine: &Engine, after: i64) -> Option<Vec<EventRecord>> {
    let engine = engine.clone();
    tokio::task::spawn_blocking(move || engine.events_after(after, BACKLOG_PAGE))
        .await
        .ok()?
        .ok()
}

/// Sends one event unless the client already has it. Returns `false` when the socket is gone.
async fn send_event(socket: &mut WebSocket, last_sent: &mut i64, record: EventRecord) -> bool {
    let id = record.id;
    if !send_frame(socket, &ServerFrame::Event { record }).await {
        return false;
    }
    *last_sent = id;
    true
}

/// The next client frame that parses. `None` when the socket closed or sent something invalid.
async fn next_frame(socket: &mut WebSocket) -> Option<ClientFrame> {
    loop {
        match socket.recv().await? {
            Ok(Message::Text(text)) => return parse_client_frame(text.as_str()).ok(),
            Ok(Message::Ping(_) | Message::Pong(_)) => {}
            Ok(_) | Err(_) => return None,
        }
    }
}

async fn send_frame(socket: &mut WebSocket, frame: &ServerFrame) -> bool {
    match encode_server_frame(frame) {
        Some(text) => socket.send(Message::Text(text.into())).await.is_ok(),
        None => false,
    }
}

async fn close(socket: &mut WebSocket, code: u16, reason: &'static str) {
    let frame = CloseFrame {
        code,
        reason: reason.into(),
    };
    // The peer may already be gone, so a failed close is not an error here.
    let _ = socket.send(Message::Close(Some(frame))).await;
}

/// An event with an id above the last one sent. Ids only grow, so anything else is a repeat.
pub fn is_fresh(id: i64, last_sent: i64) -> bool {
    id > last_sent
}

pub fn parse_client_frame(text: &str) -> Result<ClientFrame, serde_json::Error> {
    serde_json::from_str(text)
}

pub fn encode_server_frame(frame: &ServerFrame) -> Option<String> {
    serde_json::to_string(frame).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_auth_frame_parses_with_its_token() {
        let frame = parse_client_frame(r#"{"op":"auth","token":"t0k"}"#);
        assert_eq!(
            frame.ok(),
            Some(ClientFrame::Auth {
                token: "t0k".to_owned()
            })
        );
    }

    #[test]
    fn a_subscribe_frame_carries_its_cursor() {
        let frame = parse_client_frame(r#"{"op":"subscribe","after":12}"#);
        assert_eq!(frame.ok(), Some(ClientFrame::Subscribe { after: 12 }));
    }

    #[test]
    fn an_unknown_op_does_not_parse() {
        assert!(parse_client_frame(r#"{"op":"cap_raise"}"#).is_err());
        assert!(parse_client_frame("not json").is_err());
    }

    #[test]
    fn a_pong_frame_encodes_with_its_op() {
        assert_eq!(
            encode_server_frame(&ServerFrame::Pong).as_deref(),
            Some(r#"{"op":"pong"}"#)
        );
    }

    #[test]
    fn only_ids_above_the_last_sent_are_fresh() {
        assert!(is_fresh(5, 4));
        assert!(!is_fresh(4, 4));
        assert!(!is_fresh(3, 4));
    }
}
