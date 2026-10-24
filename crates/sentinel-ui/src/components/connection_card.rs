//! The Connections page and the Devices page of Settings (`docs/product-scope.md`, F18). Each
//! connection row shows the daemon's state and a Test button that asks the daemon to check it
//! again. The `OpenRouter` key goes to the daemon's keychain route and is never shown back.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::{ConnectionState, DeviceInfo, DeviceKind};
use serde::Deserialize;

use crate::api;
use crate::components::icon::Icon;
use crate::components::status_badge::shape_markup;
use crate::components::top_bar::settle;
use crate::state::{ToastKind, Ui, use_ui};

/// One row of `GET /api/connections`. The daemon owns the type; this copy reads the same fields.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ConnectionRow {
    pub name: String,
    pub state: ConnectionState,
    pub detail: String,
}

/// The color, shape, and words for a connection state. Color is never the only signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateLook {
    pub label: &'static str,
    pub color: &'static str,
    pub shape: &'static str,
}

pub fn state_look(state: ConnectionState) -> StateLook {
    let (label, color, shape) = match state {
        ConnectionState::Connected => ("Connected", "ok", "dot"),
        ConnectionState::NeedsLogin => ("Needs login", "warn", "ring"),
        ConnectionState::Missing => ("Not installed", "warn", "ring"),
        ConnectionState::Failed => ("Failed", "danger", "oct"),
    };
    StateLook {
        label,
        color,
        shape,
    }
}

/// The name the prototype shows for a connection. An unknown name shows as the daemon sent it.
pub fn connection_label(name: &str) -> String {
    match name.to_ascii_lowercase().as_str() {
        "tailscale" => "Tailscale".to_owned(),
        "openrouter" => "OpenRouter".to_owned(),
        "mcp" => "MCP file server".to_owned(),
        "git" => "Git".to_owned(),
        "keychain" => "OS keychain".to_owned(),
        _ => name.to_owned(),
    }
}

fn connection_icon(name: &str) -> &'static str {
    match name.to_ascii_lowercase().as_str() {
        "openrouter" => "key",
        "mcp" => "lock",
        "git" => "branch",
        "keychain" => "shield",
        _ => "net",
    }
}

/// Icon for a paired device. Telegram is a chat, not a screen.
pub fn device_icon(kind: DeviceKind) -> &'static str {
    match kind {
        DeviceKind::Phone => "phone",
        DeviceKind::Desktop => "desktop",
        DeviceKind::Telegram => "send",
    }
}

/// Devices the user has not revoked. Revoked ones stay in the daemon for the log only.
pub fn active_devices(devices: &[DeviceInfo]) -> Vec<DeviceInfo> {
    devices
        .iter()
        .filter(|device| !device.revoked)
        .cloned()
        .collect()
}

/// The pairing date as `YYYY-MM-DD` in UTC.
pub fn paired_on(device: &DeviceInfo) -> String {
    let date = device.created_at.date();
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

#[component]
pub fn ConnectionsPage() -> impl IntoView {
    let ui = use_ui();
    let rows = RwSignal::new(Vec::<ConnectionRow>::new());
    Effect::new(move |_| load_connections(ui, rows));
    view! {
        <h2>"Connections"</h2>
        <p class="lead">"What Sentinel needs to reach, and its state right now."</p>
        {move || {
            rows.get()
                .into_iter()
                .map(|row| connection_card(ui, rows, row))
                .collect_view()
        }}
    }
}

fn load_connections(ui: Ui, rows: RwSignal<Vec<ConnectionRow>>) {
    spawn_local(async move {
        match api::get::<Vec<ConnectionRow>>("/api/connections").await {
            Ok(list) => rows.set(list),
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

fn connection_card(
    ui: Ui,
    rows: RwSignal<Vec<ConnectionRow>>,
    row: ConnectionRow,
) -> impl IntoView {
    let look = state_look(row.state);
    let label = connection_label(&row.name);
    let icon = connection_icon(&row.name);
    let is_openrouter = row.name.eq_ignore_ascii_case("openrouter");
    let test_path = format!("/api/connections/{}/test", row.name.to_ascii_lowercase());
    let status_style = format!("color:var(--{})", look.color);
    view! {
        <div class="card" style="margin-bottom:.6em">
            <div class="row sp wrap">
                <span class="row"><Icon name=icon size=17 /><b>{label}</b></span>
                <span class="stb" style=status_style>
                    <svg class="shp" viewBox="0 0 12 12" aria-hidden="true" inner_html=shape_markup(look.shape, look.color)></svg>
                    <span>{look.label}</span>
                </span>
            </div>
            <div class="sub" style="margin:.35em 0">{row.detail}</div>
            {is_openrouter.then(|| view! { <OpenRouterKey /> })}
            <div class="row">
                <button class="btn" on:click=move |_| test_connection(ui, rows, test_path.clone())>
                    "Test"
                </button>
            </div>
        </div>
    }
}

fn test_connection(ui: Ui, rows: RwSignal<Vec<ConnectionRow>>, path: String) {
    spawn_local(async move {
        match api::post_empty::<serde_json::Value>(&path).await {
            Ok(_) => {
                ui.show_toast("Test finished.", ToastKind::Info);
                load_connections(ui, rows);
            }
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

/// The OpenRouter key field. The text is cleared after a save and never sent back to the page.
#[component]
fn OpenRouterKey() -> impl IntoView {
    let ui = use_ui();
    let key = RwSignal::new(String::new());
    view! {
        <div class="row wrap">
            <input
                class="field"
                type="password"
                placeholder="Paste API key"
                aria-label="OpenRouter API key"
                autocomplete="off"
                style="max-width:16em"
                prop:value=key
                on:input=move |ev| key.set(event_target_value(&ev))
            />
            <button class="btn" on:click=move |_| save_key(ui, key)>"Save key"</button>
        </div>
        <div class="sub">"The key goes to your keychain."</div>
    }
}

fn save_key(ui: Ui, key: RwSignal<String>) {
    let value = key.get_untracked();
    if value.trim().is_empty() {
        ui.show_toast("Paste a key first.", ToastKind::Warn);
        return;
    }
    spawn_local(async move {
        let body = serde_json::json!({ "key": value });
        let result =
            api::send::<_, serde_json::Value>("PUT", "/api/secrets/openrouter", &body).await;
        if result.is_ok() {
            key.set(String::new());
            ui.show_toast("Key saved.", ToastKind::Ok);
        }
        settle(ui, result);
    });
}

#[component]
pub fn DevicesPage() -> impl IntoView {
    let ui = use_ui();
    view! {
        <h2>"Devices"</h2>
        <p class="lead">"Phones and tablets paired over Tailscale."</p>
        {move || device_section(ui)}
        <PhoneAccess />
    }
}

fn device_section(ui: Ui) -> AnyView {
    let devices = active_devices(&ui.snapshot.get().devices);
    let count = devices.len();
    let list = if devices.is_empty() {
        view! { <div class="note">"No devices paired."</div> }.into_any()
    } else {
        devices
            .into_iter()
            .map(|device| device_card(ui, device))
            .collect_view()
            .into_any()
    };
    view! {
        <div class="psec" style="margin-top:1em">
            <h5>{format!("Paired devices · {count}")}</h5>
            {list}
        </div>
    }
    .into_any()
}

fn device_card(ui: Ui, device: DeviceInfo) -> impl IntoView {
    let id = device.id.as_str().to_owned();
    let icon = device_icon(device.kind);
    let paired = format!("Paired {}", paired_on(&device));
    view! {
        <div class="tcard">
            <div class="row sp">
                <span class="row"><Icon name=icon size=16 /><b>{device.name}</b></span>
                <button class="btn sm dng" on:click=move |_| revoke_device(ui, id.clone())>"Revoke"</button>
            </div>
            <div class="meta">{paired}</div>
        </div>
    }
}

fn revoke_device(ui: Ui, id: String) {
    let confirmed = web_sys::window()
        .and_then(|window| {
            window
                .confirm_with_message("Revoke this device? It will need to pair again.")
                .ok()
        })
        .unwrap_or(false);
    if !confirmed {
        return;
    }
    spawn_local(async move {
        let path = format!("/api/devices/{id}");
        let result =
            api::send::<_, serde_json::Value>("DELETE", &path, &serde_json::json!({})).await;
        if result.is_ok() {
            ui.show_toast("Device revoked.", ToastKind::Ok);
        }
        settle(ui, result);
    });
}

const PHONE_CAN: [&str; 5] = [
    "See everything: stream, board, files, changes, cost",
    "Pause, resume, kill an agent",
    "Answer approvals and decisions",
    "Lower the cap, tighten lessons mode",
    "Review the Brain",
];

const PHONE_CANNOT: [&str; 3] = ["Raise the cap", "Edit files or tests", "Run shell commands"];

#[component]
fn PhoneAccess() -> impl IntoView {
    view! {
        <div class="psec">
            <h5>"What a phone can do"</h5>
            <div class="tree" style="font-family:var(--font-ui)">
                {PHONE_CAN.into_iter().map(|text| access_row(text, true)).collect_view()}
                {PHONE_CANNOT.into_iter().map(|text| access_row(text, false)).collect_view()}
            </div>
        </div>
    }
}

fn access_row(text: &'static str, allowed: bool) -> impl IntoView {
    let (icon, color, text_color) = if allowed {
        ("check", "ok", "text")
    } else {
        ("x", "danger", "muted")
    };
    view! {
        <div class="row" style=format!("padding:.2em 0;color:var(--{color})")>
            <Icon name=icon size=14 />
            <span style=format!("color:var(--{text_color})")>{text}</span>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    fn device(name: &str, revoked: bool) -> DeviceInfo {
        DeviceInfo {
            id: sentinel_core::DeviceId::new(name),
            name: name.to_owned(),
            kind: DeviceKind::Phone,
            created_at: OffsetDateTime::UNIX_EPOCH,
            revoked,
        }
    }

    #[test]
    fn a_connected_state_is_ok_and_a_failure_is_danger() {
        assert_eq!(state_look(ConnectionState::Connected).color, "ok");
        assert_eq!(state_look(ConnectionState::Failed).color, "danger");
        assert_eq!(state_look(ConnectionState::Missing).label, "Not installed");
    }

    #[test]
    fn known_connection_names_get_the_prototype_labels() {
        assert_eq!(connection_label("openrouter"), "OpenRouter");
        assert_eq!(connection_label("keychain"), "OS keychain");
        assert_eq!(connection_label("Unknown Thing"), "Unknown Thing");
    }

    #[test]
    fn revoked_devices_are_left_out_of_the_list() {
        let devices = [device("Pixel 7", false), device("Old tablet", true)];
        let active = active_devices(&devices);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "Pixel 7");
    }

    #[test]
    fn a_pairing_date_is_shown_in_utc_as_a_plain_date() {
        assert_eq!(paired_on(&device("Pixel 7", false)), "1970-01-01");
    }

    #[test]
    fn each_device_kind_has_an_icon() {
        assert_eq!(device_icon(DeviceKind::Phone), "phone");
        assert_eq!(device_icon(DeviceKind::Desktop), "desktop");
        assert_eq!(device_icon(DeviceKind::Telegram), "send");
    }
}
