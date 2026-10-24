//! The Remote panel: Tailscale state, pairing a phone, paired devices, and what a phone can do
//! (`docs/mobile.md`, Pairing and Security). The Tailscale checks live in `remote_tailscale`.

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
use crate::state::{ToastKind, Ui, refresh, use_ui};
use crate::views::remote_tailscale::{TailscaleCard, refresh_status};

/// How often the open pairing asks for new requests.
const PENDING_POLL_MS: u32 = 2_000;

/// A phone that asked to pair and waits for approval.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PendingPairing {
    pub id: String,
    pub device_name: String,
}

/// `mm:ss` for a number of seconds, such as `04:59`.
pub fn format_countdown(seconds: u32) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
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

    #[test]
    fn countdown_is_minutes_and_seconds() {
        assert_eq!(format_countdown(300), "05:00");
        assert_eq!(format_countdown(59), "00:59");
        assert_eq!(format_countdown(0), "00:00");
    }

    #[test]
    fn phone_lists_cover_both_sides() {
        assert!(PHONE_CANNOT.contains(&"Raise the cap"));
        assert!(PHONE_CAN.contains(&"Lower the cap"));
    }
}
