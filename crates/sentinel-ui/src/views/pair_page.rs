//! The page a phone opens from the QR code or the link `/pair`. It asks for the one-time code and
//! a name for the device, waits while the computer asks its owner to approve, and keeps the token.

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use sentinel_core::api::PairSubmit;
use serde::Deserialize;

use crate::api;
use crate::theme;

const POLL_MS: u32 = 2000;

/// What `GET /api/pair/:id/outcome` says.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Outcome {
    Waiting,
    Approved { token: String },
    Denied,
    Expired,
    Unknown,
}

/// Where the page is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Entering,
    Sending,
    Waiting,
    Failed(String),
}

/// The code as typed: no spaces, upper case. The computer shows it as `ABCD-EFGH`.
pub fn normalize_code(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase()
}

/// A code has eight letters and digits, with or without the dash.
pub fn code_looks_complete(code: &str) -> bool {
    code.chars().filter(char::is_ascii_alphanumeric).count() == 8
}

pub fn can_submit(code: &str, name: &str) -> bool {
    code_looks_complete(&normalize_code(code)) && !name.trim().is_empty()
}

/// A starting name for this device from the browser's own description.
pub fn default_device_name(user_agent: &str) -> String {
    let lower = user_agent.to_lowercase();
    if lower.contains("iphone") {
        "iPhone".to_owned()
    } else if lower.contains("ipad") {
        "iPad".to_owned()
    } else if lower.contains("android") {
        "Android phone".to_owned()
    } else {
        "Phone".to_owned()
    }
}

/// The sentence for an outcome that ends the wait without a token.
pub fn failure_message(outcome: &Outcome) -> Option<&'static str> {
    match outcome {
        Outcome::Denied => Some("The computer said no. Ask for a new code and try again."),
        Outcome::Expired => Some("The code ran out of time. Ask the computer for a new one."),
        Outcome::Unknown => Some("The computer does not know this request. Try again."),
        Outcome::Waiting | Outcome::Approved { .. } => None,
    }
}

async fn submit(code: String, name: String) -> Result<String, String> {
    let body = PairSubmit {
        code: normalize_code(&code),
        device_name: name.trim().to_owned(),
    };
    let sent: serde_json::Value = api::post("/api/pair", &body)
        .await
        .map_err(|e| e.to_string())?;
    sent["id"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "The computer sent an answer this page does not understand.".to_owned())
}

/// Ask every two seconds until the computer decides. Returns the token or a sentence.
async fn wait_for_approval(id: String) -> Result<String, String> {
    loop {
        let outcome: Outcome = api::get(&format!("/api/pair/{id}/outcome"))
            .await
            .map_err(|e| e.to_string())?;
        if let Outcome::Approved { token } = outcome {
            return Ok(token);
        }
        if let Some(text) = failure_message(&outcome) {
            return Err(text.to_owned());
        }
        TimeoutFuture::new(POLL_MS).await;
    }
}

fn go_home() {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_href("/");
    }
}

fn start(code: String, name: String, phase: RwSignal<Phase>) {
    phase.set(Phase::Sending);
    leptos::task::spawn_local(async move {
        let id = match submit(code, name).await {
            Ok(id) => id,
            Err(text) => return phase.set(Phase::Failed(text)),
        };
        phase.set(Phase::Waiting);
        match wait_for_approval(id).await {
            Ok(token) => {
                api::set_token(&token);
                go_home();
            }
            Err(text) => phase.set(Phase::Failed(text)),
        }
    });
}

#[component]
pub fn PairPage() -> impl IntoView {
    Effect::new(|_| theme::apply(&crate::app::load_prefs()));
    let agent = web_sys::window()
        .and_then(|w| w.navigator().user_agent().ok())
        .unwrap_or_default();
    let code = RwSignal::new(String::new());
    let name = RwSignal::new(default_device_name(&agent));
    let phase = RwSignal::new(Phase::Entering);
    let busy = move || matches!(phase.get(), Phase::Sending | Phase::Waiting);
    let ready = move || !busy() && can_submit(&code.get(), &name.get());
    view! {
        <div class="wiz" data-layout="phone">
            <div class="wiz-hd"><span class="lg">"Sentinel"</span></div>
            <div class="wiz-body"><div class="wiz-in">
                <h2>"Pair this phone"</h2>
                <p class="lead">"Type the one-time code from the computer. A code works once and runs out after five minutes. Then the computer asks its owner to approve this phone."</p>
                <label class="sub" for="pair-code">"One-time code"</label>
                <input id="pair-code" autocomplete="off" autocapitalize="characters" inputmode="text"
                    prop:value=move || code.get() prop:disabled=busy
                    on:input=move |ev| code.set(event_target_value(&ev)) />
                <label class="sub" for="pair-name">"Name for this phone"</label>
                <input id="pair-name" prop:value=move || name.get() prop:disabled=busy
                    on:input=move |ev| name.set(event_target_value(&ev)) />
                <PhaseNote phase=phase />
            </div></div>
            <div class="wiz-ft">
                <button class="btn pri" prop:disabled=move || !ready()
                    on:click=move |_| start(code.get(), name.get(), phase)>"Pair"</button>
            </div>
        </div>
    }
}

#[component]
fn PhaseNote(phase: RwSignal<Phase>) -> impl IntoView {
    view! {
        <p class="note" role="status">
            {move || match phase.get() {
                Phase::Entering => String::new(),
                Phase::Sending => "Sending the code.".to_owned(),
                Phase::Waiting => "Waiting for the computer to approve this phone.".to_owned(),
                Phase::Failed(text) => text,
            }}
        </p>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_cleaned_and_checked() {
        assert_eq!(normalize_code(" abcd-efgh "), "ABCD-EFGH");
        assert!(code_looks_complete("ABCD-EFGH"));
        assert!(code_looks_complete("ABCDEFGH"));
        assert!(!code_looks_complete("ABCD-EF"));
    }

    #[test]
    fn the_button_needs_a_code_and_a_name() {
        assert!(can_submit("abcd efgh", "Blair phone"));
        assert!(!can_submit("abcd efgh", "  "));
        assert!(!can_submit("abc", "Blair phone"));
    }

    #[test]
    fn the_device_name_comes_from_the_browser() {
        assert_eq!(default_device_name("Mozilla (iPhone; CPU)"), "iPhone");
        assert_eq!(default_device_name("Linux; Android 14"), "Android phone");
        assert_eq!(default_device_name("X11; Linux"), "Phone");
    }

    #[test]
    fn outcomes_parse_and_end_the_wait() {
        let approved: Outcome =
            serde_json::from_str(r#"{"state":"approved","device":"d","token":"t"}"#).unwrap();
        assert_eq!(
            approved,
            Outcome::Approved {
                token: "t".to_owned()
            }
        );
        let waiting: Outcome = serde_json::from_str(r#"{"state":"waiting"}"#).unwrap();
        assert_eq!(failure_message(&waiting), None);
        assert!(failure_message(&Outcome::Denied).is_some());
        assert!(failure_message(&Outcome::Expired).is_some());
    }
}
