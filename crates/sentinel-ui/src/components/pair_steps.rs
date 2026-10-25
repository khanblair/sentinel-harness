//! `PairSteps`: Part 2 of pairing. The phone steps, the QR code and the one-time code with its
//! countdown, and the line that waits for the phone and approves it (`docs/mobile.md`, Connecting
//! and Pairing).

use leptos::prelude::*;
use sentinel_core::api::PairStart;
use sentinel_core::{DeviceInfo, DeviceKind, TailscaleState};

use crate::components::icon::Icon;
use crate::components::install_log::copy_text;
use crate::components::pair_checklist::{StepList, pair_steps};
use crate::state::use_ui;
use crate::views::pairing::{PendingPair, PhoneSignals, approve_pair};

/// `m:ss`, for the code's countdown.
pub fn format_countdown(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// The newest phone that is still paired.
pub fn paired_name(devices: &[DeviceInfo]) -> Option<String> {
    devices
        .iter()
        .rev()
        .find(|device| device.kind == DeviceKind::Phone && !device.revoked)
        .map(|device| device.name.clone())
}

/// The line under the code: the expiry countdown, or a prompt for a new code.
pub fn expiry_line(remaining: u32) -> String {
    if remaining == 0 {
        "The code expired. Press New code.".to_string()
    } else {
        format!("Expires in {}", format_countdown(remaining))
    }
}

/// The phone part. `start` is the code that was issued; the countdown, the pending phones,
/// and the paired phone are read live from `signals`.
#[component]
pub fn PairSteps(
    start: PairStart,
    signals: PhoneSignals,
    on_new_code: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="card" style="margin-top:.8em">
            <b>"Part 2. On your phone"</b>
            <StepList steps=pair_steps(TailscaleState::Running) />
            <div class="row wrap" style="gap:1em;align-items:flex-start">
                <div style="width:9em;max-width:40%" inner_html=start.qr_svg.clone()></div>
                <PairCode start=start remaining=signals.remaining on_new_code=on_new_code />
            </div>
            <PairStatus signals=signals />
        </div>
    }
}

/// The link, the machine name, and the one-time code with its countdown.
#[component]
fn PairCode(
    start: PairStart,
    remaining: RwSignal<u32>,
    on_new_code: Callback<()>,
) -> impl IntoView {
    let ui = use_ui();
    let url = start.url.clone();
    let machine = start
        .tailscale
        .machine_name
        .clone()
        .unwrap_or_else(|| "Not set".to_string());
    view! {
        <div style="min-width:0;flex:1">
            <div class="sub">"Link"</div>
            <div class="cmd" style="margin:.2em 0 .6em">
                <span>{url.clone()}</span>
                <button class="ibtn" aria-label="Copy link" on:click=move |_| copy_text(ui, url.clone())>
                    <Icon name="copy" size=14 />
                </button>
            </div>
            <div class="sub">"Machine name, if MagicDNS is on"</div>
            <div class="mono" style="margin:.2em 0 .6em">{machine}</div>
            <div class="sub">"One-time code"</div>
            <div class="mono" style="font-size:1.7em;font-weight:600;letter-spacing:.08em">{start.code}</div>
            <div class="row wrap">
                <span class="sub">{move || expiry_line(remaining.get())}</span>
                <Show when=move || remaining.get() == 0 fallback=|| ()>
                    <button class="btn sm" on:click=move |_| on_new_code.run(())>"New code"</button>
                </Show>
            </div>
        </div>
    }
}

/// Waiting, the pending approvals, or the paired phone.
#[component]
fn PairStatus(signals: PhoneSignals) -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="row wrap" style="margin-top:.8em">
            {move || match ui.snapshot.with(|s| paired_name(&s.devices)) {
                Some(name) => view! {
                    <span class="row" style="color:var(--ok)">
                        <Icon name="check" size=16 />
                        <span>{format!("{name} is paired")}</span>
                    </span>
                }
                .into_any(),
                None => view! {
                    <span class="row sub">
                        <span class="spin"></span>
                        <span>"Waiting for your phone"</span>
                    </span>
                }
                .into_any(),
            }}
            <PendingApprovals pending=signals.pending />
        </div>
    }
}

/// One approve button per phone that asked to pair.
#[component]
fn PendingApprovals(pending: RwSignal<Vec<PendingPair>>) -> impl IntoView {
    let ui = use_ui();
    move || {
        pending
            .get()
            .into_iter()
            .map(|asking| {
                let id = asking.id.clone();
                view! {
                    <button class="btn pri" on:click=move |_| approve_pair(ui, id.clone())>
                        {format!("Approve {}", asking.device_name)}
                    </button>
                }
            })
            .collect_view()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::DeviceId;
    use time::OffsetDateTime;

    fn device(name: &str, kind: DeviceKind, revoked: bool) -> DeviceInfo {
        DeviceInfo {
            id: DeviceId::new(name),
            name: name.to_string(),
            kind,
            created_at: OffsetDateTime::UNIX_EPOCH,
            revoked,
        }
    }

    #[test]
    fn the_countdown_reads_minutes_and_seconds() {
        assert_eq!(format_countdown(299), "4:59");
        assert_eq!(format_countdown(65), "1:05");
        assert_eq!(format_countdown(0), "0:00");
    }

    #[test]
    fn an_expired_code_asks_for_a_new_one() {
        assert_eq!(expiry_line(0), "The code expired. Press New code.");
        assert_eq!(expiry_line(120), "Expires in 2:00");
    }

    #[test]
    fn the_paired_phone_is_the_newest_one_not_revoked() {
        let devices = [
            device("Pixel 7", DeviceKind::Phone, false),
            device("Old phone", DeviceKind::Phone, true),
            device("Desktop", DeviceKind::Desktop, false),
        ];
        assert_eq!(paired_name(&devices), Some("Pixel 7".to_string()));
        assert_eq!(paired_name(&devices[2..]), None);
    }
}
