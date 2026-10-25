//! The Tailscale half of the Remote panel: the three checks on this computer, the setup steps
//! when Tailscale is missing or signed out, and the sign-in action (`docs/mobile.md`, Pairing).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::TailscaleState;
use sentinel_core::api::TailscaleStatus;

use crate::api;
use crate::components::icon::Icon;
use crate::components::pair_checklist::{StepList, pair_steps};
use crate::components::status_badge::shape_markup;
use crate::state::{ToastKind, Ui, use_ui};

/// Mirrors the three dot states of the prototype's pairing checklist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Warn,
    Bad,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckRow {
    pub level: Level,
    pub title: &'static str,
    pub detail: String,
}

/// Color token and shape name for a checklist level.
pub fn level_look(level: Level) -> (&'static str, &'static str) {
    match level {
        Level::Ok => ("ok", "dot"),
        Level::Warn => ("warn", "ring"),
        Level::Bad => ("danger", "oct"),
    }
}

/// The word after each checklist row.
pub fn level_word(level: Level) -> &'static str {
    match level {
        Level::Ok => "OK",
        Level::Warn => "Needs action",
        Level::Bad => "Missing",
    }
}

/// The three checks on this computer: installed, signed in, and an address.
pub fn tailscale_rows(status: &TailscaleStatus) -> Vec<CheckRow> {
    let installed = status.state != TailscaleState::NotInstalled;
    let (run_level, run_detail) = running_check(status);
    let (address_level, address_detail) = address_check(status);
    vec![
        CheckRow {
            level: if installed { Level::Ok } else { Level::Bad },
            title: "Tailscale is installed",
            detail: if installed {
                "The tailscale command was found on this computer.".to_owned()
            } else {
                "The tailscale command was not found.".to_owned()
            },
        },
        CheckRow {
            level: run_level,
            title: "Tailscale is running and signed in",
            detail: run_detail,
        },
        CheckRow {
            level: address_level,
            title: "This computer has a private address",
            detail: address_detail,
        },
    ]
}

fn running_check(status: &TailscaleStatus) -> (Level, String) {
    match (status.state, &status.machine_name) {
        (TailscaleState::Running, Some(name)) => {
            (Level::Ok, format!("Signed in. This computer is {name}."))
        }
        (TailscaleState::Running, None) => (Level::Ok, "Signed in.".to_owned()),
        (TailscaleState::SignedOut, _) => (Level::Warn, "Installed, but signed out.".to_owned()),
        (TailscaleState::NotInstalled, _) => (Level::Bad, "Install it first.".to_owned()),
    }
}

fn address_check(status: &TailscaleStatus) -> (Level, String) {
    match &status.address {
        Some(address) if status.state == TailscaleState::Running => (
            Level::Ok,
            format!("{address}. Only your own devices can reach it."),
        ),
        _ => (Level::Bad, "No address yet.".to_owned()),
    }
}

/// Loads the Tailscale state into `status`. A failure shows as a toast.
pub fn refresh_status(status: RwSignal<Option<TailscaleStatus>>, ui: Ui) {
    spawn_local(async move {
        match api::get::<TailscaleStatus>("/api/tailscale/status").await {
            Ok(value) => status.set(Some(value)),
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

/// The message and color for the daemon's sign-in answer. Only `ok: true` is a success.
fn signin_toast(body: &serde_json::Value) -> (String, ToastKind) {
    let message = body
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_owned();
    let kind = if body.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
        ToastKind::Ok
    } else {
        ToastKind::Warn
    };
    (message, kind)
}

fn sign_in(ui: Ui, status: RwSignal<Option<TailscaleStatus>>) {
    spawn_local(async move {
        match api::post_empty::<serde_json::Value>("/api/tailscale/signin").await {
            Ok(body) => {
                let (message, kind) = signin_toast(&body);
                ui.show_toast(message, kind);
                refresh_status(status, ui);
            }
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

#[component]
fn CheckLine(row: CheckRow) -> impl IntoView {
    let (color, shape) = level_look(row.level);
    view! {
        <div class="chk">
            <span><svg class="shp" viewBox="0 0 12 12" aria-hidden="true" inner_html=shape_markup(shape, color)></svg></span>
            <div class="n"><b style="font-weight:500">{row.title}</b><small>{row.detail}</small></div>
            <span class="sub">{level_word(row.level)}</span>
        </div>
    }
}

#[component]
pub fn TailscaleCard(status: RwSignal<Option<TailscaleStatus>>) -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="card">
            <div class="row sp wrap">
                <b>"Part 1. This computer"</b>
                <button class="btn sm" on:click=move |_| refresh_status(status, ui)>
                    <Icon name="refresh" size=13 /> " Check again"
                </button>
            </div>
            {move || match status.get() {
                None => view! { <div class="sub" style="margin-top:.5em">"Checking Tailscale."</div> }.into_any(),
                Some(value) => view! {
                    <div>
                        {tailscale_rows(&value).into_iter().map(|row| view! { <CheckLine row=row /> }).collect_view()}
                        {setup_steps(value.state, ui, status)}
                    </div>
                }.into_any(),
            }}
        </div>
    }
}

/// What to do next when Tailscale is missing or signed out. Empty when it runs.
fn setup_steps(
    state: TailscaleState,
    ui: Ui,
    status: RwSignal<Option<TailscaleStatus>>,
) -> impl IntoView {
    if state == TailscaleState::Running {
        return ().into_any();
    }
    let signed_out = state == TailscaleState::SignedOut;
    view! {
        <div style="margin-top:.7em">
            <StepList steps=pair_steps(state) />
            <Show when=move || signed_out fallback=|| ()>
                <button class="btn pri" on:click=move |_| sign_in(ui, status)>"Sign in to Tailscale"</button>
            </Show>
        </div>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(state: TailscaleState, address: Option<&str>) -> TailscaleStatus {
        TailscaleStatus {
            state,
            address: address.map(str::to_owned),
            machine_name: Some("blair-mac".to_owned()),
        }
    }

    #[test]
    fn a_missing_install_fails_all_three_checks() {
        let rows = tailscale_rows(&status(TailscaleState::NotInstalled, None));
        assert_eq!(rows[0].level, Level::Bad);
        assert_eq!(rows[1].level, Level::Bad);
        assert_eq!(rows[2].level, Level::Bad);
    }

    #[test]
    fn signed_out_needs_action_not_missing() {
        let rows = tailscale_rows(&status(TailscaleState::SignedOut, None));
        assert_eq!(rows[0].level, Level::Ok);
        assert_eq!(rows[1].level, Level::Warn);
        assert_eq!(level_word(rows[1].level), "Needs action");
    }

    #[test]
    fn a_running_client_shows_its_address_and_name() {
        let rows = tailscale_rows(&status(TailscaleState::Running, Some("100.101.102.103")));
        assert!(rows.iter().all(|row| row.level == Level::Ok));
        assert!(rows[1].detail.contains("blair-mac"));
        assert!(rows[2].detail.starts_with("100.101.102.103"));
    }

    #[test]
    fn level_colors_pair_with_shapes() {
        assert_eq!(level_look(Level::Ok), ("ok", "dot"));
        assert_eq!(level_look(Level::Bad), ("danger", "oct"));
    }

    #[test]
    fn a_successful_sign_in_is_an_ok_toast() {
        let body = serde_json::json!({ "ok": true, "message": "Signed in." });
        assert_eq!(
            signin_toast(&body),
            ("Signed in.".to_owned(), ToastKind::Ok)
        );
    }

    #[test]
    fn a_refused_sign_in_is_a_warning_with_no_message_when_none_came() {
        let body = serde_json::json!({ "ok": false });
        assert_eq!(signin_toast(&body), (String::new(), ToastKind::Warn));
    }
}
