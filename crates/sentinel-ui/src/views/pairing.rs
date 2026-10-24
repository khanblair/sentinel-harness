//! The phone step of the setup wizard (`docs/product-scope.md`, F8 step 4). It checks Tailscale
//! first, then shows the phone steps and a one-time code, and waits for the phone to ask to pair.
//! The checklist is `components/pair_checklist.rs`; the code and approvals are
//! `components/pair_steps.rs`.

use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::TailscaleState;
use sentinel_core::api::{PairStart, TailscaleStatus};
use serde::Deserialize;

use crate::api;
use crate::components::pair_checklist::PairChecklist;
use crate::components::pair_steps::PairSteps;
use crate::state::{ToastKind, Ui, refresh};
use crate::views::onboarding::use_wizard;

/// One phone that asked to pair and waits for approval (`GET /api/pair/pending`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PendingPair {
    pub id: String,
    pub device_name: String,
}

/// The signals the phone step shares with its parts.
#[derive(Debug, Clone, Copy)]
pub struct PhoneSignals {
    pub tailscale: RwSignal<Option<TailscaleStatus>>,
    pub checking: RwSignal<bool>,
    pub pair: RwSignal<Option<PairStart>>,
    /// Seconds left on the one-time code.
    pub remaining: RwSignal<u32>,
    pub pending: RwSignal<Vec<PendingPair>>,
}

impl Default for PhoneSignals {
    fn default() -> Self {
        Self {
            tailscale: RwSignal::new(None),
            checking: RwSignal::new(false),
            pair: RwSignal::new(None),
            remaining: RwSignal::new(0),
            pending: RwSignal::new(Vec::new()),
        }
    }
}

impl PhoneSignals {
    /// Pairing starts only once Tailscale runs.
    fn is_running(self) -> bool {
        self.tailscale.with(|status| {
            status
                .as_ref()
                .is_some_and(|s| s.state == TailscaleState::Running)
        })
    }
}

/// Reads `GET /api/tailscale/status` and keeps the answer.
pub fn check_tailscale(ui: Ui, signals: PhoneSignals) {
    signals.checking.set(true);
    spawn_local(async move {
        match api::get::<TailscaleStatus>("/api/tailscale/status").await {
            Ok(status) => signals.tailscale.set(Some(status)),
            Err(error) => ui.show_toast(error.to_string(), ToastKind::Danger),
        }
        signals.checking.set(false);
    });
}

/// Asks the daemon for a new one-time code. The countdown starts when it arrives.
pub fn start_pairing(ui: Ui, signals: PhoneSignals) {
    spawn_local(async move {
        match api::post_empty::<PairStart>("/api/pair/start").await {
            Ok(start) => {
                signals.remaining.set(start.expires_in_seconds);
                signals.pair.set(Some(start));
            }
            Err(error) => ui.show_toast(error.to_string(), ToastKind::Danger),
        }
    });
}

/// Runs `tailscale up` and checks again.
fn sign_in(ui: Ui, signals: PhoneSignals) {
    spawn_local(async move {
        match api::post_empty::<serde_json::Value>("/api/tailscale/signin").await {
            Ok(_) => check_tailscale(ui, signals),
            Err(error) => ui.show_toast(error.to_string(), ToastKind::Danger),
        }
    });
}

/// Approves a phone that asked to pair. The paired device shows up in the snapshot.
pub fn approve_pair(ui: Ui, id: String) {
    spawn_local(async move {
        let path = format!("/api/pair/{id}/approve");
        match api::post_empty::<serde_json::Value>(&path).await {
            Ok(_) => refresh(ui),
            Err(error) => ui.show_toast(error.to_string(), ToastKind::Danger),
        }
    });
}

fn poll_pending(signals: PhoneSignals) {
    spawn_local(async move {
        if let Ok(list) = api::get::<Vec<PendingPair>>("/api/pair/pending").await {
            signals.pending.set(list);
        }
    });
}

/// Polls for pending phones and counts the code down. The timers stop when the code changes or
/// the step closes.
fn keep_pairing_timers(signals: PhoneSignals) {
    let poll = set_interval_with_handle(move || poll_pending(signals), Duration::from_secs(2));
    let tick = set_interval_with_handle(
        move || {
            signals
                .remaining
                .update(|left| *left = left.saturating_sub(1));
        },
        Duration::from_secs(1),
    );
    if let (Ok(poll), Ok(tick)) = (poll, tick) {
        on_cleanup(move || {
            poll.clear();
            tick.clear();
        });
    }
}

#[component]
pub fn PhoneStep() -> impl IntoView {
    let ui = use_wizard().ui;
    let signals = PhoneSignals::default();
    Effect::new(move |_| check_tailscale(ui, signals));
    Effect::new(move |_| {
        if signals.is_running() && signals.pair.with(Option::is_none) {
            start_pairing(ui, signals);
        }
    });
    Effect::new(move |_| {
        if signals.pair.with(Option::is_some) {
            keep_pairing_timers(signals);
        }
    });
    let on_check = Callback::new(move |()| check_tailscale(ui, signals));
    let on_signin = Callback::new(move |()| sign_in(ui, signals));
    let on_new_code = Callback::new(move |()| start_pairing(ui, signals));
    view! {
        <h2>"Pair your phone"</h2>
        <p class="lead">
            "Your phone reaches Sentinel through Tailscale, a private network between your own devices. Nothing is opened to the internet. Follow the two parts below, or skip this and do it later."
        </p>
        <PairChecklist
            status=signals.tailscale
            checking=signals.checking
            on_check=on_check
            on_signin=on_signin
        />
        {move || {
            signals
                .pair
                .get()
                .map(|start| view! { <PairSteps start=start signals=signals on_new_code=on_new_code /> })
        }}
    }
}
