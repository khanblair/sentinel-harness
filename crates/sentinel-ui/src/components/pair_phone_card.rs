//! `PairPhoneCard`: everything about pairing a phone. Settings, Devices, and the Remote panel all
//! show this one card. It checks Tailscale, asks for a one-time code, shows the QR code and the
//! link, and lists the phones that wait for approval (`docs/mobile.md`, Connecting and Pairing).

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::TailscaleState;
use sentinel_core::api::{PairStart, TailscaleStatus};
use serde::Deserialize;

use crate::api;
use crate::components::icon::Icon;
use crate::components::install_log::copy_text;
use crate::components::pair_checklist::{StepList, pair_steps};
use crate::components::pair_qr::PairQr;
use crate::state::{ToastKind, Ui, refresh, use_ui};
use crate::views::remote_tailscale::{TailscaleCard, refresh_status};

/// How often the open code asks the daemon for phones that want to pair.
const PENDING_POLL_MS: u32 = 3_000;
/// The code counts down once a second.
const TICK_MS: u32 = 1_000;

/// A phone that asked to pair and waits for approval (`GET /api/pair/pending`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PendingPairing {
    pub id: String,
    pub device_name: String,
}

/// The signals one pairing card shares between its parts.
#[derive(Clone, Copy)]
struct PairState {
    ui: Ui,
    status: RwSignal<Option<TailscaleStatus>>,
    pairing: RwSignal<Option<PairStart>>,
    remaining: RwSignal<u32>,
    pending: RwSignal<Vec<PendingPairing>>,
    alive: RwSignal<bool>,
}

impl PairState {
    fn new(ui: Ui) -> Self {
        Self {
            ui,
            status: RwSignal::new(None),
            pairing: RwSignal::new(None),
            remaining: RwSignal::new(0),
            pending: RwSignal::new(Vec::new()),
            alive: RwSignal::new(true),
        }
    }

    /// The phone reaches this computer over Tailscale, so pairing waits until Tailscale runs.
    fn is_running(self) -> bool {
        self.status.with(|status| {
            status
                .as_ref()
                .is_some_and(|value| value.state == TailscaleState::Running)
        })
    }

    /// The timers read this without tracking, so they never re-run the view.
    fn code_is_live(self) -> bool {
        self.pairing.with_untracked(Option::is_some) && self.remaining.get_untracked() > 0
    }
}

/// `mm:ss` for a number of seconds, such as `04:59`.
pub fn format_countdown(seconds: u32) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// The line under the code. It reads `Expired` at zero, so a dead code never looks live.
pub fn countdown_text(seconds: u32) -> String {
    if seconds == 0 {
        "Expired".to_owned()
    } else {
        format!("Expires in {}", format_countdown(seconds))
    }
}

/// The link with the machine name in place of the address, for a phone with `MagicDNS` on. The
/// port stays, because the daemon listens on it. `None` when no machine name is known.
pub fn magic_dns_link(url: &str, machine: &str) -> Option<String> {
    let machine = dns_label(machine);
    if machine.is_empty() {
        return None;
    }
    let (_, rest) = url.split_once("://")?;
    let (host_port, path) = rest.split_once('/')?;
    let (_, port) = host_port.rsplit_once(':')?;
    Some(format!("http://{machine}:{port}/{path}"))
}

/// The machine name as `MagicDNS` writes it: lower case letters and digits, with a hyphen for each
/// run of spaces or other marks. `Kolaborate's MacBook Air` becomes `kolaborates-macbook-air`.
pub fn dns_label(name: &str) -> String {
    let mut label = String::new();
    for ch in name.chars() {
        if ch.is_alphanumeric() {
            label.extend(ch.to_lowercase());
        } else if (ch.is_whitespace() || ch == '-' || ch == '_')
            && !label.is_empty()
            && !label.ends_with('-')
        {
            label.push('-');
        }
    }
    label.trim_end_matches('-').to_owned()
}

/// Runs `action` every `interval_ms` until `alive` turns false. The loop ends with the card.
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

fn refresh_pending(pending: RwSignal<Vec<PendingPairing>>) {
    spawn_local(async move {
        if let Ok(list) = api::get::<Vec<PendingPairing>>("/api/pair/pending").await {
            pending.set(list);
        }
    });
}

/// Asks the daemon for a new one-time code. The countdown starts when the answer arrives.
fn start_pairing(state: PairState) {
    spawn_local(async move {
        match api::post_empty::<PairStart>("/api/pair/start").await {
            Ok(start) => {
                state.remaining.set(start.expires_in_seconds);
                state.pairing.set(Some(start));
            }
            Err(failure) => state.ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

/// One second passes on the code. At zero the code stops being live and the card says Expired.
fn count_down(state: PairState) {
    if !state.code_is_live() {
        return;
    }
    let left = state.remaining.get_untracked();
    state.remaining.set(left.saturating_sub(1));
    if left == 1 {
        state.ui.show_toast(
            "The pairing code expired. Press New code for a new one.",
            ToastKind::Warn,
        );
    }
}

fn decide(state: PairState, id: &str, approve: bool) {
    let verb = if approve { "approve" } else { "deny" };
    let path = format!("/api/pair/{id}/{verb}");
    spawn_local(async move {
        match api::post_empty::<serde_json::Value>(&path).await {
            Ok(_) => refresh_pending(state.pending),
            Err(failure) => state.ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
        refresh(state.ui);
    });
}

fn start_timers(state: PairState) {
    tick_every(TICK_MS, state.alive, move || count_down(state));
    // A request stays approvable for five minutes after its code expires, so the poll
    // keeps running while any code is open, not only while it counts down.
    tick_every(PENDING_POLL_MS, state.alive, move || {
        if state.pairing.with_untracked(Option::is_some) {
            refresh_pending(state.pending);
        }
    });
}

#[component]
pub fn PairPhoneCard() -> impl IntoView {
    let ui = use_ui();
    if ui.on_phone() {
        return view! { <div class="note">"Pair a new phone from the computer."</div> }.into_any();
    }
    let state = PairState::new(ui);
    on_cleanup(move || {
        state.alive.try_set(false);
    });
    refresh_status(state.status, ui);
    start_timers(state);
    view! {
        <TailscaleCard status=state.status />
        <PairCard state=state />
    }
    .into_any()
}

/// Part 2: the phone steps, then the code and the approvals. It shows only while Tailscale runs.
#[component]
fn PairCard(state: PairState) -> impl IntoView {
    view! {
        <Show when=move || state.is_running() fallback=|| ()>
            <div class="card" style="margin-top:.8em">
                <div class="row sp wrap">
                    <b>"Part 2. On your phone"</b>
                    <Show when=move || state.pairing.with(Option::is_none) fallback=|| ()>
                        <button class="btn sm pri" on:click=move |_| start_pairing(state)>"Start pairing"</button>
                    </Show>
                </div>
                <StepList steps=pair_steps(TailscaleState::Running) />
                {move || state.pairing.get().map(|start| view! { <PairCode start=start state=state /> })}
            </div>
        </Show>
    }
}

/// The QR code and the link, the one-time code, and its countdown. The QR and the code hide once
/// the code expires, so nobody scans a dead code.
#[component]
fn PairCode(start: PairStart, state: PairState) -> impl IntoView {
    let live = move || state.remaining.get() > 0;
    view! {
        <Show when=live fallback=|| ()>
            <CodeDetails start=start.clone() state=state />
        </Show>
        <CountdownLine state=state />
        {move || state.pairing.with(Option::is_some).then(|| view! { <PendingList state=state /> })}
    }
}

#[component]
fn CodeDetails(start: PairStart, state: PairState) -> impl IntoView {
    let machine = start.tailscale.machine_name.clone().unwrap_or_default();
    let dns_link = magic_dns_link(&start.url, &machine);
    let url = start.url.clone();
    view! {
        <div class="row wrap" style="gap:1em;align-items:flex-start;margin-top:.8em">
            <PairQr svg=start.qr_svg />
            <div style="min-width:0;flex:1">
                <div class="sub">"Link"</div>
                <div class="cmd" style="margin:.2em 0 .6em">
                    <span>{url.clone()}</span>
                    <button class="ibtn" aria-label="Copy link" on:click=move |_| copy_text(state.ui, url.clone())>
                        <Icon name="copy" size=14 />
                    </button>
                </div>
                {dns_link.map(|text| view! {
                    <div class="sub">"Same link by name, if MagicDNS is on"</div>
                    <div class="cmd" style="margin:.2em 0 .6em"><span>{text}</span></div>
                })}
                <div class="sub">"One-time code"</div>
                <div class="mono" style="font-size:1.7em;font-weight:600;letter-spacing:.08em">{start.code}</div>
            </div>
        </div>
    }
}

#[component]
fn CountdownLine(state: PairState) -> impl IntoView {
    view! {
        <div class="row wrap" style="gap:.6em;margin-top:.6em">
            <span class="sub">{move || countdown_text(state.remaining.get())}</span>
            <button class="btn sm" on:click=move |_| start_pairing(state)>"New code"</button>
        </div>
    }
}

/// The phones that asked to pair. Approve and Deny answer each request.
#[component]
fn PendingList(state: PairState) -> impl IntoView {
    move || {
        let list = state.pending.get();
        if list.is_empty() {
            return view! { <div class="sub">"Waiting for your phone."</div> }.into_any();
        }
        list.into_iter()
            .map(|request| pending_row(state, request))
            .collect_view()
            .into_any()
    }
}

fn pending_row(state: PairState, request: PendingPairing) -> impl IntoView {
    let approve_id = request.id.clone();
    let deny_id = request.id;
    view! {
        <div class="tcard">
            <div class="row sp wrap">
                <b>{request.device_name}</b>
                <span class="row" style="gap:.4em">
                    <button class="btn sm pri" on:click=move |_| decide(state, &approve_id, true)>"Approve"</button>
                    <button class="btn sm" on:click=move |_| decide(state, &deny_id, false)>"Deny"</button>
                </span>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdown_is_minutes_and_seconds() {
        assert_eq!(format_countdown(300), "05:00");
        assert_eq!(format_countdown(59), "00:59");
    }

    #[test]
    fn the_countdown_text_says_expired_at_zero() {
        assert_eq!(countdown_text(0), "Expired");
        assert_eq!(countdown_text(300), "Expires in 05:00");
    }

    #[test]
    fn the_magic_dns_link_keeps_the_port_and_path() {
        assert_eq!(
            magic_dns_link("http://100.64.0.9:8080/pair", "work-laptop"),
            Some("http://work-laptop:8080/pair".to_owned())
        );
    }

    #[test]
    fn a_machine_name_becomes_a_dns_label() {
        assert_eq!(
            dns_label("Kolaborate\u{2019}s MacBook Air"),
            "kolaborates-macbook-air"
        );
        assert_eq!(dns_label("work-laptop"), "work-laptop");
        assert_eq!(dns_label("  "), "");
    }

    #[test]
    fn the_magic_dns_link_needs_a_machine_name() {
        assert_eq!(magic_dns_link("http://100.64.0.9:8080/pair", ""), None);
    }
}
