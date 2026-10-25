//! `KeysSection`: the bring-your-own-key block on the Connections page. The daemon keeps each key
//! in the Mac's keychain. This page sends keys, and shows only a hint: the last four characters.

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

use crate::api::{self, ApiFailure};
use crate::components::key_card::KeyCard;
use crate::state::use_ui;

/// One row of `GET /api/keys`. `hint` is `...abcd` or null.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct KeyInfo {
    pub provider: String,
    pub label: String,
    pub has_key: bool,
    pub hint: Option<String>,
}

/// The body of `PUT /api/keys/{provider}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyBody {
    pub key: String,
}

/// The answer of `POST /api/keys/{provider}/test`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct KeyTest {
    pub provider: String,
    pub ok: bool,
    pub message: String,
}

/// Whether the list load is running, finished, or failed with the daemon's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Load {
    Loading,
    Ready,
    Failed(String),
}

pub fn key_path(provider: &str) -> String {
    format!("/api/keys/{provider}")
}

/// The daemon's own words for a failure. A status error shows its message without the status.
pub fn failure_message(failure: &ApiFailure) -> String {
    match failure {
        ApiFailure::Status { message, .. } => message.clone(),
        ApiFailure::Network(_) | ApiFailure::Decode(_) => failure.to_string(),
    }
}

/// The row for `provider`. A provider the list does not hold shows as having no key.
pub fn key_for(keys: &[KeyInfo], provider: &str) -> KeyInfo {
    keys.iter()
        .find(|key| key.provider == provider)
        .cloned()
        .unwrap_or_else(|| KeyInfo {
            provider: provider.to_owned(),
            label: provider.to_owned(),
            has_key: false,
            hint: None,
        })
}

pub fn provider_names(keys: &[KeyInfo]) -> Vec<String> {
    keys.iter().map(|key| key.provider.clone()).collect()
}

#[component]
pub fn KeysSection() -> impl IntoView {
    let ui = use_ui();
    let keys = RwSignal::new(Vec::<KeyInfo>::new());
    let status = RwSignal::new(Load::Loading);
    let providers = Memo::new(move |_| provider_names(&keys.get()));
    let on_change = Callback::new(move |()| load_keys(keys, status));
    Effect::new(move |_| {
        if !ui.on_phone() {
            load_keys(keys, status);
        }
    });
    view! {
        <div class="psec">
            <h5>"API keys"</h5>
            <p class="sub">
                "Bring your own key. Keys are kept in your Mac's keychain, never in a file. Agents of a kind use its key instead of their own login."
            </p>
            {move || key_status(ui.on_phone(), status.get())}
            <For
                each=move || providers.get()
                key=|provider| provider.clone()
                children=move |provider| key_row(keys, provider, on_change)
            />
        </div>
    }
}

/// The line above the cards: the phone note, the loading line, or the daemon's error. A loaded
/// list shows nothing here.
fn key_status(on_phone: bool, status: Load) -> AnyView {
    if on_phone {
        return view! { <div class="note">"Keys need the computer."</div> }.into_any();
    }
    match status {
        Load::Loading => view! { <div class="sub">"Loading keys."</div> }.into_any(),
        Load::Failed(text) => view! { <div class="note" role="alert">{text}</div> }.into_any(),
        Load::Ready => ().into_any(),
    }
}

/// One card. Its info follows the list, while the card itself keeps its typed text and its
/// last result when the list reloads.
fn key_row(
    keys: RwSignal<Vec<KeyInfo>>,
    provider: String,
    on_change: Callback<()>,
) -> impl IntoView {
    let info = Memo::new(move |_| key_for(&keys.get(), &provider));
    view! { <KeyCard info=info on_change=on_change /> }
}

/// Reloads the list. The list keeps its last good answer when a reload fails, so the cards stay.
fn load_keys(keys: RwSignal<Vec<KeyInfo>>, status: RwSignal<Load>) {
    spawn_local(async move {
        match api::get::<Vec<KeyInfo>>("/api/keys").await {
            Ok(list) => {
                keys.set(list);
                status.set(Load::Ready);
            }
            Err(failure) => status.set(Load::Failed(failure_message(&failure))),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(provider: &str, has_key: bool) -> KeyInfo {
        KeyInfo {
            provider: provider.to_owned(),
            label: provider.to_uppercase(),
            has_key,
            hint: None,
        }
    }

    #[test]
    fn a_key_path_names_the_provider() {
        assert_eq!(key_path("openrouter"), "/api/keys/openrouter");
    }

    #[test]
    fn a_status_error_shows_only_the_daemon_message() {
        let failure = ApiFailure::Status {
            code: 400,
            message: "That does not look like a key.".to_owned(),
        };
        assert_eq!(failure_message(&failure), "That does not look like a key.");
    }

    #[test]
    fn a_network_failure_keeps_its_own_text() {
        let failure = ApiFailure::Network("timeout".to_owned());
        assert!(failure_message(&failure).starts_with("Could not reach the daemon."));
    }

    #[test]
    fn a_provider_in_the_list_gives_its_own_row() {
        let keys = [key("claude", true), key("gemini", false)];
        assert!(key_for(&keys, "claude").has_key);
        assert_eq!(key_for(&keys, "gemini").label, "GEMINI");
    }

    #[test]
    fn a_provider_missing_from_the_list_shows_no_key() {
        let keys = [key("claude", true)];
        let row = key_for(&keys, "deepseek");
        assert!(!row.has_key);
        assert_eq!(row.provider, "deepseek");
    }

    #[test]
    fn providers_keep_the_order_the_daemon_sent() {
        let keys = [key("openrouter", false), key("claude", true)];
        assert_eq!(provider_names(&keys), ["openrouter", "claude"]);
    }

    #[test]
    fn a_key_row_decodes_with_a_null_hint() {
        let json = r#"{"provider":"claude","label":"Claude","has_key":true,"hint":null}"#;
        let row: KeyInfo = serde_json::from_str(json).unwrap();
        assert!(row.has_key);
        assert_eq!(row.hint, None);
    }
}
