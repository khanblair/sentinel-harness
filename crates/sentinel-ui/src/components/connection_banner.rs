//! `ConnectionBanner`: says when the page shows the last known state, not the live one
//! (`docs/ui-rules.md`, Feedback States).

use leptos::prelude::*;

use crate::components::icon::Icon;
use crate::state::{Connection, use_ui};

/// The banner text for a connection state, or `None` when no banner shows. A first connect shows
/// nothing; a retry after a live session says "Reconnecting".
pub fn banner_text(connection: &Connection, had_events: bool) -> Option<String> {
    match connection {
        Connection::Offline { since } => {
            Some(format!("Disconnected. Showing last state from {since}"))
        }
        Connection::Connecting => had_events.then(|| "Reconnecting".to_owned()),
        Connection::Live => None,
    }
}

#[component]
pub fn ConnectionBanner() -> impl IntoView {
    let ui = use_ui();
    move || {
        let had_events = ui.snapshot.with(|snapshot| snapshot.last_event_id > 0);
        let text = ui
            .connection
            .with(|connection| banner_text(connection, had_events));
        text.map(|text| {
            view! {
                <div class="note warnc" role="status">
                    <Icon name="alert" size=15 />
                    <span>{text}</span>
                </div>
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_banner_names_the_last_state_time() {
        let offline = Connection::Offline {
            since: "14:05".to_owned(),
        };
        assert_eq!(
            banner_text(&offline, true).as_deref(),
            Some("Disconnected. Showing last state from 14:05")
        );
    }

    #[test]
    fn live_connection_shows_no_banner() {
        assert_eq!(banner_text(&Connection::Live, true), None);
    }

    #[test]
    fn first_connect_shows_no_banner_but_a_retry_says_reconnecting() {
        assert_eq!(banner_text(&Connection::Connecting, false), None);
        assert_eq!(
            banner_text(&Connection::Connecting, true).as_deref(),
            Some("Reconnecting")
        );
    }
}
