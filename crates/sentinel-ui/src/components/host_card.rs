//! The "Where Sentinel runs" card in Settings, Integrations. It shows what the daemon reports about
//! the host and reloads it when a host event arrives. The text and the calls live in `host`.

use leptos::prelude::*;
use sentinel_core::api::HostStatus;

use crate::components::icon::Icon;
use crate::components::status_badge::shape_markup;
use crate::host::{self, CheckRow};
use crate::state::use_ui;

#[component]
pub fn HostCard() -> impl IntoView {
    let ui = use_ui();
    let status = RwSignal::new(None::<HostStatus>);
    let host_event = Memo::new(move |_| ui.snapshot.with(|s| host::latest_host_event(&s.events)));
    Effect::new(move |_| {
        let _ = host_event.get();
        host::load(status, ui);
    });
    view! {
        <div class="card" style="margin-bottom:.8em">
            <div class="row sp wrap">
                <b class="row"><Icon name="desktop" size=16 /> " Where Sentinel runs"</b>
                {move || status.get().map(|host| view! {
                    <span class="chip">{host::location_label(host.headless)}</span>
                })}
            </div>
            {move || status.get().map(|host| view! { <HostBody host status /> })}
        </div>
        <BackOnlineNote />
    }
}

#[component]
fn HostBody(host: HostStatus, status: RwSignal<Option<HostStatus>>) -> impl IntoView {
    let rows = host::check_rows(&host, host::browser_now());
    view! {
        <div class="sub" style="margin:.4em 0">{host::location_sub(host.headless)}</div>
        {rows.into_iter().map(|row| view! { <CheckLine row /> }).collect_view()}
        <SetupSteps />
        <CheckButton status />
    }
}

#[component]
fn CheckLine(row: CheckRow) -> impl IntoView {
    let CheckRow { title, detail, ok } = row;
    let (color, shape) = if ok { ("ok", "dot") } else { ("warn", "ring") };
    let word = if ok { "OK" } else { "Needs action" };
    view! {
        <div class="chk">
            <span><svg class="shp" viewBox="0 0 12 12" aria-hidden="true" inner_html=shape_markup(shape, color)></svg></span>
            <div class="n"><b style="font-weight:500">{title}</b><small>{detail}</small></div>
            <span class="sub">{word}</span>
        </div>
    }
}

#[component]
fn SetupSteps() -> impl IntoView {
    view! {
        <details style="margin-top:.6em">
            <summary>"Set up the always-on machine"</summary>
            <ol class="ol">
                <li><span>"Install Sentinel on the machine."</span></li>
                <li><span>"Run " <span class="k">"sentinel service install"</span>". It writes a launchd job on macOS or a systemd unit on Linux. It starts at boot and restarts if it stops."</span></li>
                <li><span>"Sign the machine in to Tailscale."</span></li>
                <li><span>"Log in to your agent CLIs on that machine and clone the repository there."</span></li>
                <li><span>"Open this app at the machine's Tailscale address."</span></li>
            </ol>
        </details>
    }
}

#[component]
fn CheckButton(status: RwSignal<Option<HostStatus>>) -> impl IntoView {
    let ui = use_ui();
    let phone = move || ui.on_phone();
    view! {
        <div class="row wrap" style="margin-top:.6em">
            <button class="btn" disabled=phone on:click=move |_| host::check(status, ui)>
                <Icon name="refresh" size=13 /> " Check the host"
            </button>
            <Show when=phone fallback=|| ()>
                <span class="sub">"Needs the computer"</span>
            </Show>
        </div>
    }
}

#[component]
fn BackOnlineNote() -> impl IntoView {
    let ui = use_ui();
    let gap = move || ui.snapshot.with(|s| host::latest_back_online(&s.events));
    view! {
        {move || gap().map(|seconds| view! {
            <div class="note"><Icon name="info" size=14 /><span>{host::back_online_text(seconds)}</span></div>
        })}
    }
}
