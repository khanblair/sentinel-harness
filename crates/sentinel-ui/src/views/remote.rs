//! The Remote panel: Tailscale state, pairing a phone, paired devices, and what a phone can do
//! (`docs/mobile.md`, Pairing and Security).

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::TailscaleState;
use sentinel_core::api::{PairStart, TailscaleStatus};
use serde::Deserialize;

use crate::api;
use crate::components::device_list::{DeviceList, active_devices};
use crate::components::icon::Icon;
use crate::components::pair_qr::PairQr;
use crate::components::status_badge::shape_markup;
use crate::state::{ToastKind, Ui, refresh, use_ui};

/// How often the open pairing asks for new requests.
const PENDING_POLL_MS: u32 = 2_000;

/// A phone that asked to pair and waits for approval.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PendingPairing {
    pub id: String,
    pub device_name: String,
}

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

/// `mm:ss` for a number of seconds, such as `04:59`.
pub fn format_countdown(seconds: u32) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
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

/// What a phone can do. From `docs/mobile.md`, Security.
pub const PHONE_CAN: [&str; 5] = [
    "Pause, resume, and kill an agent",
    "Answer approvals and decisions",
    "Lower the cap",
    "Review lessons and Brain proposals",
    "Read notes and the graph",
];

/// What a phone cannot do. From `docs/mobile.md`, Security.
pub const PHONE_CANNOT: [&str; 4] = [
    "Raise the cap",
    "Loosen the Brain mode",
    "Edit files or tests",
    "Run shell commands",
];

/// Runs `action` every `interval_ms` until `alive` turns false. The loop ends with the panel.
fn tick_every(interval_ms: u32, alive: RwSignal<bool>, action: impl Fn() + 'static) {
    spawn_local(async move {
        loop {
            TimeoutFuture::new(interval_ms).await;
            if alive.try_get_untracked() != Some(true) {
                break;
            }
            action();
        }
    });
}

fn refresh_status(status: RwSignal<Option<TailscaleStatus>>, ui: Ui) {
    spawn_local(async move {
        match api::get::<TailscaleStatus>("/api/tailscale/status").await {
            Ok(value) => status.set(Some(value)),
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

fn refresh_pending(pending: RwSignal<Vec<PendingPairing>>) {
    spawn_local(async move {
        if let Ok(list) = api::get::<Vec<PendingPairing>>("/api/pair/pending").await {
            pending.set(list);
        }
    });
}

fn start_pairing(ui: Ui, pairing: RwSignal<Option<PairStart>>, remaining: RwSignal<u32>) {
    spawn_local(async move {
        match api::post_empty::<PairStart>("/api/pair/start").await {
            Ok(start) => {
                remaining.set(start.expires_in_seconds);
                pairing.set(Some(start));
            }
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

/// One second passes on the pairing code. At zero the pairing closes.
fn count_down(ui: Ui, pairing: RwSignal<Option<PairStart>>, remaining: RwSignal<u32>) {
    if pairing.with_untracked(Option::is_none) {
        return;
    }
    let left = remaining.get_untracked().saturating_sub(1);
    remaining.set(left);
    if left == 0 {
        pairing.set(None);
        ui.show_toast(
            "The pairing code expired. Start pairing again.",
            ToastKind::Warn,
        );
    }
}

fn decide(ui: Ui, pending: RwSignal<Vec<PendingPairing>>, id: &str, approve: bool) {
    let verb = if approve { "approve" } else { "deny" };
    let path = format!("/api/pair/{id}/{verb}");
    spawn_local(async move {
        match api::post_empty::<serde_json::Value>(&path).await {
            Ok(_) => refresh_pending(pending),
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
        refresh(ui);
    });
}

fn sign_in(ui: Ui, status: RwSignal<Option<TailscaleStatus>>) {
    spawn_local(async move {
        match api::post_empty::<serde_json::Value>("/api/tailscale/signin").await {
            Ok(body) => {
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
fn TailscaleCard(status: RwSignal<Option<TailscaleStatus>>) -> impl IntoView {
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
    match state {
        TailscaleState::NotInstalled => view! {
            <div style="margin-top:.7em">
                <div class="sub" style="margin-bottom:.3em">"Install Tailscale, then press Check again."</div>
                <ol class="ol">
                    <li><span>"macOS or Windows: download the app from tailscale.com/download, open it, and sign in."</span></li>
                    <li><span>"Linux: install Tailscale with the official steps from tailscale.com, then run sudo tailscale up."</span></li>
                    <li><span>"Come back here and press Check again."</span></li>
                </ol>
            </div>
        }
        .into_any(),
        TailscaleState::SignedOut => view! {
            <div style="margin-top:.7em">
                <div class="sub" style="margin-bottom:.5em">"Tailscale is installed but signed out. Signing in opens your browser."</div>
                <button class="btn pri" on:click=move |_| sign_in(ui, status)>"Sign in to Tailscale"</button>
            </div>
        }
        .into_any(),
        TailscaleState::Running => ().into_any(),
    }
}

#[component]
fn PendingList(pending: RwSignal<Vec<PendingPairing>>) -> impl IntoView {
    let ui = use_ui();
    view! {
        {move || {
            let list = pending.get();
            if list.is_empty() {
                return view! { <div class="sub">"Waiting for your phone."</div> }.into_any();
            }
            list.into_iter()
                .map(|request| {
                    let approve_id = request.id.clone();
                    let deny_id = request.id.clone();
                    view! {
                        <div class="tcard">
                            <div class="row sp wrap">
                                <b>{request.device_name}</b>
                                <span class="row" style="gap:.4em">
                                    <button class="btn sm pri" on:click=move |_| decide(ui, pending, &approve_id, true)>"Approve"</button>
                                    <button class="btn sm" on:click=move |_| decide(ui, pending, &deny_id, false)>"Deny"</button>
                                </span>
                            </div>
                        </div>
                    }
                })
                .collect_view()
                .into_any()
        }}
    }
}

#[component]
fn PairCard(
    status: RwSignal<Option<TailscaleStatus>>,
    pairing: RwSignal<Option<PairStart>>,
    remaining: RwSignal<u32>,
    pending: RwSignal<Vec<PendingPairing>>,
) -> impl IntoView {
    let ui = use_ui();
    let can_pair = move || {
        status.with(|s| {
            s.as_ref()
                .is_some_and(|value| value.state == TailscaleState::Running)
        })
    };
    view! {
        <Show when=can_pair fallback=|| ()>
            <div class="card" style="margin-top:.8em">
                <div class="row sp wrap">
                    <b>"Part 2. On your phone"</b>
                    <Show when=move || pairing.with(Option::is_none) fallback=|| ()>
                        <button class="btn sm pri" on:click=move |_| start_pairing(ui, pairing, remaining)>"Start pairing"</button>
                    </Show>
                </div>
                <ol class="ol">
                    <li><span>"Install Tailscale from the App Store or Google Play."</span></li>
                    <li><span>"Open it and sign in with the same account as this computer."</span></li>
                    <li><span>"Scan the code below with the phone camera, or type the link into the phone's browser."</span></li>
                    <li><span>"Enter the one-time code. This computer then asks you to approve the new device."</span></li>
                </ol>
                {move || pairing.get().map(|start| pairing_body(start, remaining, pending))}
            </div>
        </Show>
    }
}

fn pairing_body(
    start: PairStart,
    remaining: RwSignal<u32>,
    pending: RwSignal<Vec<PendingPairing>>,
) -> impl IntoView {
    let host = start.tailscale.machine_name.clone().unwrap_or_default();
    view! {
        <div class="row wrap" style="gap:1em;align-items:flex-start;margin-top:.8em">
            <PairQr svg=start.qr_svg />
            <div style="min-width:0;flex:1">
                <div class="sub">"Link"</div>
                <div class="cmd" style="margin:.2em 0 .6em"><span>{start.url}</span></div>
                <div class="sub">"Or by name, if MagicDNS is on"</div>
                <div class="cmd" style="margin:.2em 0 .6em"><span>{host}</span></div>
                <div class="sub">"One-time code"</div>
                <div class="mono" style="font-size:1.7em;font-weight:600;letter-spacing:.08em">{start.code}</div>
                <div class="sub">{move || format!("Expires in {}", format_countdown(remaining.get()))}</div>
            </div>
        </div>
        <div style="margin-top:.8em"><PendingList pending=pending /></div>
    }
}

#[component]
pub fn RemotePanel() -> impl IntoView {
    let ui = use_ui();
    let alive = RwSignal::new(true);
    let status: RwSignal<Option<TailscaleStatus>> = RwSignal::new(None);
    let pairing: RwSignal<Option<PairStart>> = RwSignal::new(None);
    let remaining = RwSignal::new(0u32);
    let pending: RwSignal<Vec<PendingPairing>> = RwSignal::new(Vec::new());
    on_cleanup(move || {
        alive.try_set(false);
    });
    refresh_status(status, ui);
    tick_every(1_000, alive, move || count_down(ui, pairing, remaining));
    tick_every(PENDING_POLL_MS, alive, move || {
        if pairing.with_untracked(Option::is_some) {
            refresh_pending(pending);
        }
    });
    let devices = Signal::derive(move || ui.snapshot.with(|s| active_devices(&s.devices)));
    view! {
        <TailscaleCard status=status />
        <PairCard status=status pairing=pairing remaining=remaining pending=pending />
        <div class="psec" style="margin-top:1em">
            <h5>{move || format!("Paired devices · {}", devices.get().len())}</h5>
            <DeviceList devices=devices />
        </div>
        <PhoneLimits />
    }
}

#[component]
fn PhoneLimits() -> impl IntoView {
    view! {
        <div class="psec">
            <h5>"What a phone can do"</h5>
            <div class="tree" style="font-family:var(--font-ui)">
                {PHONE_CAN.into_iter().map(|text| view! {
                    <div class="row" style="padding:.2em 0;color:var(--ok)"><Icon name="check" size=14 /><span style="color:var(--text)">{text}</span></div>
                }).collect_view()}
                {PHONE_CANNOT.into_iter().map(|text| view! {
                    <div class="row" style="padding:.2em 0;color:var(--danger)"><Icon name="x" size=14 /><span style="color:var(--muted)">{text}</span></div>
                }).collect_view()}
            </div>
            <div class="sub" style="margin-top:.5em">"Secrets are never sent to the phone. Every phone action is recorded with the device name. A lost phone can be revoked from this computer."</div>
        </div>
    }
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
    fn countdown_is_minutes_and_seconds() {
        assert_eq!(format_countdown(300), "05:00");
        assert_eq!(format_countdown(59), "00:59");
        assert_eq!(format_countdown(0), "00:00");
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
    fn phone_lists_cover_both_sides() {
        assert!(PHONE_CANNOT.contains(&"Raise the cap"));
        assert!(PHONE_CAN.contains(&"Lower the cap"));
    }
}
