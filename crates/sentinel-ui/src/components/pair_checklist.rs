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

/// The phone steps, in the order the phone does them. Each one matches what `pair_page.rs`
/// and the daemon do: a code works once, and this computer approves the request.
const PHONE_STEPS: [&str; 6] = [
    "Install the Tailscale app on the phone from the App Store or Google Play.",
    "Open Tailscale on the phone, sign in with the same account as this computer, and turn it on.",
    "Scan the QR code with the phone camera, or type the link into the phone's browser.",
    "On the phone, type the one-time code and press Pair. Do it before the countdown ends. A code works once.",
    "The phone then waits. This computer shows a request with the phone's name. Press Approve here.",
    "After you approve, the phone opens Sentinel. Keep its page open until then.",
];

/// The numbered steps for one Tailscale state. Missing or signed-out Tailscale gets the fix on
/// this computer. Once Tailscale runs, the steps move to the phone.
pub fn pair_steps(state: TailscaleState) -> Vec<&'static str> {
    match state {
        TailscaleState::NotInstalled => vec![
            "Install Tailscale on this computer. On macOS or Windows, download it from tailscale.com/download.",
            "On Linux, follow the install steps at tailscale.com, then run sudo tailscale up.",
            "Open Tailscale and sign in with your account.",
            "Press Check again.",
        ],
        TailscaleState::SignedOut => vec![
            "Tailscale is installed but not signed in. Press Sign in to Tailscale. Your browser opens.",
            "Sign in to your account in the browser.",
            "Press Check again.",
        ],
        TailscaleState::Running => PHONE_STEPS.to_vec(),
    }
}

/// An ordered list of steps, in the `ol` style the prototype uses.
#[component]
pub fn StepList(steps: Vec<&'static str>) -> impl IntoView {
    view! {
        <ol class="ol">
            {steps
                .into_iter()
                .map(|step| view! { <li><span>{step}</span></li> })
                .collect_view()}
        </ol>
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
            <StepList steps=pair_steps(state) />
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
    fn a_missing_install_gets_the_install_steps_for_this_computer() {
        let steps = pair_steps(TailscaleState::NotInstalled);
        assert_eq!(steps.len(), 4);
        assert!(steps[0].contains("tailscale.com/download"));
        assert!(steps[1].starts_with("On Linux"));
        assert_eq!(steps.last(), Some(&"Press Check again."));
    }

    #[test]
    fn a_signed_out_install_gets_the_sign_in_steps() {
        let steps = pair_steps(TailscaleState::SignedOut);
        assert_eq!(steps.len(), 3);
        assert!(steps[0].contains("Sign in to Tailscale"));
    }

    #[test]
    fn a_running_tailscale_gets_the_six_phone_steps() {
        let steps = pair_steps(TailscaleState::Running);
        assert_eq!(steps.len(), 6);
        assert!(steps[3].contains("A code works once"));
        assert!(steps[4].ends_with("Press Approve here."));
    }
}
