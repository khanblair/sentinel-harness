//! `PairChecklist`: Part 1 of pairing. Tailscale must be installed, running, and signed in, with
//! a private address, before the phone steps show (`docs/architecture.md`, Pairing).

use leptos::prelude::*;
use sentinel_core::TailscaleState;
use sentinel_core::api::TailscaleStatus;

use crate::components::icon::Icon;
use crate::components::repo_checks::CheckShape;

/// One line of the checklist: its title, what was found, and the color and word to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckLine {
    pub title: &'static str,
    pub detail: String,
    pub class: &'static str,
    pub word: &'static str,
}

fn line(title: &'static str, detail: &str, class: &'static str, word: &'static str) -> CheckLine {
    CheckLine {
        title,
        detail: detail.to_string(),
        class,
        word,
    }
}

/// The three checks in order: installed, running and signed in, and a private address.
pub fn check_lines(status: &TailscaleStatus) -> [CheckLine; 3] {
    [
        install_line(status.state),
        running_line(status),
        address_line(status),
    ]
}

fn install_line(state: TailscaleState) -> CheckLine {
    let installed = state != TailscaleState::NotInstalled;
    let detail = if installed {
        "The tailscale command was found on this computer."
    } else {
        "The tailscale command was not found."
    };
    let (class, word) = if installed {
        ("ok", "OK")
    } else {
        ("danger", "Missing")
    };
    line("Tailscale is installed", detail, class, word)
}

fn running_line(status: &TailscaleStatus) -> CheckLine {
    let host = status.machine_name.as_deref().unwrap_or("this computer");
    let (detail, class, word) = match status.state {
        TailscaleState::Running => (format!("Signed in. This computer is {host}."), "ok", "OK"),
        TailscaleState::SignedOut => (
            "Installed, but signed out.".to_string(),
            "warn",
            "Needs action",
        ),
        TailscaleState::NotInstalled => ("Install it first.".to_string(), "danger", "Missing"),
    };
    CheckLine {
        title: "Tailscale is running and signed in",
        detail,
        class,
        word,
    }
}

fn address_line(status: &TailscaleStatus) -> CheckLine {
    let running = status.state == TailscaleState::Running;
    let address = status.address.as_deref().filter(|_| running);
    let (detail, class, word) = match address {
        Some(ip) => (
            format!("{ip}. Only your own devices can reach it."),
            "ok",
            "OK",
        ),
        None => ("No address yet.".to_string(), "danger", "Missing"),
    };
    CheckLine {
        title: "This computer has a private address",
        detail,
        class,
        word,
    }
}

/// The steps a user follows before the phone steps. Running needs no fix, so it gets the
/// phone steps instead.
pub fn phone_steps(state: TailscaleState) -> Vec<&'static str> {
    match state {
        TailscaleState::NotInstalled => vec![
            "macOS or Windows: download the app from tailscale.com/download, open it, and sign in.",
            "Linux: run curl -fsSL https://tailscale.com/install.sh | sh, then sudo tailscale up.",
            "Come back here and press Check again.",
        ],
        TailscaleState::SignedOut => vec![
            "Tailscale is installed but signed out. Signing in opens your browser.",
            "Press Sign in to Tailscale, then come back here and press Check again.",
        ],
        TailscaleState::Running => vec![
            "Install Tailscale from the App Store or Google Play.",
            "Open it and sign in with the same account as this computer.",
            "Scan the code below with the phone camera, or type the link into the phone's browser.",
            "Enter the one-time code. This computer then asks you to approve the new device.",
        ],
    }
}

#[component]
pub fn PairChecklist(
    #[prop(into)] status: Signal<Option<TailscaleStatus>>,
    #[prop(into)] checking: Signal<bool>,
    on_check: Callback<()>,
    on_signin: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="card">
            <div class="row sp wrap">
                <b>"Part 1. This computer"</b>
                <button class="btn sm" disabled=move || checking.get() on:click=move |_| on_check.run(())>
                    <Icon name="refresh" size=13 />
                    " Check again"
                </button>
            </div>
            {move || status.get().map(|s| view! { <CheckRows lines=check_lines(&s) /> })}
            {move || {
                status
                    .get()
                    .map(|s| s.state)
                    .filter(|state| *state != TailscaleState::Running)
                    .map(|state| view! { <FixSteps state=state on_signin=on_signin /> })
            }}
        </div>
    }
}

#[component]
fn CheckRows(lines: [CheckLine; 3]) -> impl IntoView {
    lines
        .into_iter()
        .map(|line| {
            view! {
                <div class="chk">
                    <CheckShape class=line.class />
                    <div class="n">
                        <b style="font-weight:500">{line.title}</b>
                        <small>{line.detail}</small>
                    </div>
                    <span class="sub">{line.word}</span>
                </div>
            }
        })
        .collect_view()
}

/// The install or sign-in steps, with a sign-in button when the tool is installed but signed out.
#[component]
fn FixSteps(state: TailscaleState, on_signin: Callback<()>) -> impl IntoView {
    let signed_out = state == TailscaleState::SignedOut;
    view! {
        <div style="margin-top:.7em">
            <ol class="ol">
                {phone_steps(state)
                    .into_iter()
                    .map(|step| view! { <li><span>{step}</span></li> })
                    .collect_view()}
            </ol>
            <Show when=move || signed_out fallback=|| ()>
                <button class="btn pri" on:click=move |_| on_signin.run(())>"Sign in to Tailscale"</button>
            </Show>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(state: TailscaleState, address: Option<&str>) -> TailscaleStatus {
        TailscaleStatus {
            state,
            address: address.map(str::to_string),
            machine_name: Some("wiz-pc".to_string()),
        }
    }

    fn classes(lines: &[CheckLine; 3]) -> [&'static str; 3] {
        [lines[0].class, lines[1].class, lines[2].class]
    }

    #[test]
    fn a_missing_install_fails_every_check() {
        let lines = check_lines(&status(TailscaleState::NotInstalled, None));
        assert_eq!(classes(&lines), ["danger", "danger", "danger"]);
    }

    #[test]
    fn a_signed_out_install_needs_action_on_the_second_check() {
        let lines = check_lines(&status(TailscaleState::SignedOut, None));
        assert_eq!(classes(&lines), ["ok", "warn", "danger"]);
        assert_eq!(lines[1].word, "Needs action");
    }

    #[test]
    fn a_running_tailscale_with_an_address_passes_every_check() {
        let lines = check_lines(&status(TailscaleState::Running, Some("100.101.102.103")));
        assert_eq!(classes(&lines), ["ok", "ok", "ok"]);
        assert_eq!(lines[1].detail, "Signed in. This computer is wiz-pc.");
        assert_eq!(
            lines[2].detail,
            "100.101.102.103. Only your own devices can reach it."
        );
    }

    #[test]
    fn the_install_steps_name_both_platforms() {
        let steps = phone_steps(TailscaleState::NotInstalled);
        assert_eq!(steps.len(), 3);
        assert!(steps[1].starts_with("Linux:"));
    }

    #[test]
    fn the_phone_steps_are_the_four_pairing_steps() {
        assert_eq!(phone_steps(TailscaleState::Running).len(), 4);
    }
}
