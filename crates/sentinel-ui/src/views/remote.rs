//! The Remote panel: pairing a phone, paired devices, and what a phone can do (`docs/mobile.md`,
//! Pairing and Security). The pairing card lives in `components/pair_phone_card.rs`.

use leptos::prelude::*;

use crate::components::device_list::{DeviceList, active_devices};
use crate::components::icon::Icon;
use crate::components::pair_phone_card::PairPhoneCard;
use crate::state::use_ui;

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

#[component]
pub fn RemotePanel() -> impl IntoView {
    let ui = use_ui();
    let devices = Signal::derive(move || ui.snapshot.with(|s| active_devices(&s.devices)));
    view! {
        <PairPhoneCard />
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
    fn phone_lists_cover_both_sides() {
        assert!(PHONE_CANNOT.contains(&"Raise the cap"));
        assert!(PHONE_CAN.contains(&"Lower the cap"));
    }
}
