//! `DeviceList`: paired devices, each with a revoke action that asks first.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::{DeviceInfo, DeviceKind};

use crate::api;
use crate::components::icon::Icon;
use crate::components::top_bar::settle;
use crate::state::{Ui, use_ui};

/// The icon for a device kind.
pub fn device_icon(kind: DeviceKind) -> &'static str {
    match kind {
        DeviceKind::Phone => "phone",
        DeviceKind::Desktop => "tablet",
        DeviceKind::Telegram => "send",
    }
}

/// Devices that still have access. Revoked ones are not listed.
pub fn active_devices(devices: &[DeviceInfo]) -> Vec<DeviceInfo> {
    devices
        .iter()
        .filter(|device| !device.revoked)
        .cloned()
        .collect()
}

/// The confirm sentence before a revoke.
pub fn revoke_text(name: &str) -> String {
    format!("Revoke {name}? It loses access at once. Pair it again to bring it back.")
}

fn revoke(ui: Ui, device: &DeviceInfo) {
    let path = format!("/api/devices/{}", device.id);
    spawn_local(async move {
        let body = serde_json::json!({});
        let result = api::send::<_, serde_json::Value>("DELETE", &path, &body).await;
        settle(ui, result);
    });
}

#[component]
fn DeviceRow(device: DeviceInfo, asking: RwSignal<Option<String>>) -> impl IntoView {
    let ui = use_ui();
    let id = device.id.to_string();
    let name = device.name.clone();
    let icon = device_icon(device.kind);
    let question = revoke_text(&name);
    let is_asking = {
        let id = id.clone();
        move || asking.get().as_deref() == Some(id.as_str())
    };
    let ask_id = id.clone();
    let confirm = device.clone();
    view! {
        <div class="tcard">
            <div class="row sp wrap">
                <span class="row"><Icon name=icon size=16 /><b>{name}</b></span>
                <Show
                    when=is_asking.clone()
                    fallback=move || {
                        let id = ask_id.clone();
                        view! {
                            <button class="btn sm dng" on:click=move |_| asking.set(Some(id.clone()))>"Revoke"</button>
                        }
                    }
                >
                    <span class="row" style="gap:.4em">
                        <button class="btn sm dng" on:click={
                            let confirm = confirm.clone();
                            move |_| {
                                asking.set(None);
                                revoke(ui, &confirm);
                            }
                        }>"Revoke"</button>
                        <button class="btn sm" on:click=move |_| asking.set(None)>"Cancel"</button>
                    </span>
                </Show>
            </div>
            <Show when=is_asking fallback=|| ()>
                <div class="sub" style="margin-top:.3em">{question.clone()}</div>
            </Show>
        </div>
    }
}

#[component]
pub fn DeviceList(#[prop(into)] devices: Signal<Vec<DeviceInfo>>) -> impl IntoView {
    let asking: RwSignal<Option<String>> = RwSignal::new(None);
    view! {
        {move || {
            let list = active_devices(&devices.get());
            if list.is_empty() {
                view! { <div class="note">"No devices paired."</div> }.into_any()
            } else {
                list.into_iter()
                    .map(|device| view! { <DeviceRow device=device asking=asking /> })
                    .collect_view()
                    .into_any()
            }
        }}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::DeviceId;
    use time::OffsetDateTime;

    fn device(name: &str, revoked: bool) -> DeviceInfo {
        DeviceInfo {
            id: DeviceId::new(name),
            name: name.to_owned(),
            kind: DeviceKind::Phone,
            created_at: OffsetDateTime::UNIX_EPOCH,
            revoked,
        }
    }

    #[test]
    fn revoked_devices_are_not_listed() {
        let list = active_devices(&[device("pixel", false), device("old", true)]);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "pixel");
    }

    #[test]
    fn each_kind_has_an_icon() {
        assert_eq!(device_icon(DeviceKind::Phone), "phone");
        assert_eq!(device_icon(DeviceKind::Desktop), "tablet");
        assert_eq!(device_icon(DeviceKind::Telegram), "send");
    }

    #[test]
    fn revoke_question_names_the_device() {
        assert!(revoke_text("pixel").starts_with("Revoke pixel?"));
    }
}
