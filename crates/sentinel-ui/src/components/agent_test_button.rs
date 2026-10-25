//! The Test button of an agent row and the line that shows its answer. A test sends one tiny
//! prompt through `POST /api/tools/{id}/test`. The daemon can take up to 90 seconds on it.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::{ToolInfo, ToolStatus};
use serde::Deserialize;

use crate::api::{self, ApiFailure};
use crate::components::status_badge::shape_markup;
use crate::state::use_ui;

/// The daemon's answer to a test. The core type is not shared yet, so the page keeps the two
/// fields it shows. Serde skips the rest of the answer.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolTest {
    pub ok: bool,
    pub message: String,
}

/// What the line under a row shows after a test: the daemon's sentence, or a failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestOutcome {
    pub ok: bool,
    pub text: String,
}

/// The running flag and the last outcome of one row. The button and the line share it.
#[derive(Debug, Clone, Copy)]
pub struct TestSlot {
    pub running: RwSignal<bool>,
    pub outcome: RwSignal<Option<TestOutcome>>,
}

/// The line under a row while it runs, or the reason a tool cannot be tested.
pub fn test_hint(status: ToolStatus) -> &'static str {
    match status {
        ToolStatus::Missing => "Install this agent before you test it.",
        ToolStatus::Ready
        | ToolStatus::NeedsLogin
        | ToolStatus::NeedsCommand
        | ToolStatus::Failed => "Sends one tiny prompt. It can take up to a minute.",
    }
}

/// Color token and shape name for a test answer. A pass is a green dot, a failure a red ring.
pub fn test_look(ok: bool) -> (&'static str, &'static str) {
    if ok { ("ok", "dot") } else { ("danger", "oct") }
}

/// The words for a failed test. A network failure gets a plain sentence. Any answer from the
/// daemon keeps its own message, such as the 409 for a test that is already running.
pub fn failure_text(failure: &ApiFailure) -> String {
    match failure {
        ApiFailure::Network(_) => "Could not reach the daemon.".to_owned(),
        ApiFailure::Status { message, .. } => message.clone(),
        ApiFailure::Decode(_) => failure.to_string(),
    }
}

impl TestOutcome {
    fn from_answer(answer: &ToolTest) -> Self {
        Self {
            ok: answer.ok,
            text: answer.message.clone(),
        }
    }

    fn from_failure(failure: &ApiFailure) -> Self {
        Self {
            ok: false,
            text: failure_text(failure),
        }
    }
}

/// Sends the test and stores the answer. The row stays disabled until the answer lands.
fn run_test(slot: TestSlot, id: String) {
    slot.running.set(true);
    spawn_local(async move {
        let path = format!("/api/tools/{id}/test");
        let outcome = match api::post_empty::<ToolTest>(&path).await {
            Ok(answer) => TestOutcome::from_answer(&answer),
            Err(failure) => TestOutcome::from_failure(&failure),
        };
        slot.outcome.set(Some(outcome));
        slot.running.set(false);
    });
}

/// The small Test button. A missing tool cannot be tested, and a phone does not show it.
#[component]
pub fn AgentTestButton(tool: ToolInfo, test: TestSlot) -> impl IntoView {
    let ui = use_ui();
    let missing = tool.status == ToolStatus::Missing;
    let running = test.running;
    view! {
        <Show when=move || !ui.on_phone() fallback=|| ()>
            {
                let id = tool.id.as_str().to_owned();
                view! {
                    <button
                        class="btn sm"
                        disabled=move || missing || running.get()
                        on:click=move |_| run_test(test, id.clone())
                    >
                        {move || if running.get() { "Testing" } else { "Test" }}
                    </button>
                }
            }
        </Show>
    }
}

/// The hint while a test runs, then the answer until the next test.
#[component]
pub fn AgentTestLine(test: TestSlot, status: ToolStatus) -> impl IntoView {
    let ui = use_ui();
    view! {
        <Show when=move || !ui.on_phone() fallback=|| ()>
            {move || test_line(test, status)}
        </Show>
    }
}

fn test_line(slot: TestSlot, status: ToolStatus) -> AnyView {
    if slot.running.get() || status == ToolStatus::Missing {
        return view! { <small class="sub">{test_hint(status)}</small> }.into_any();
    }
    match slot.outcome.get() {
        Some(outcome) => outcome_line(&outcome).into_any(),
        None => ().into_any(),
    }
}

fn outcome_line(outcome: &TestOutcome) -> impl IntoView {
    let (color, shape) = test_look(outcome.ok);
    view! {
        <span class="stb" style=format!("color:var(--{color})")>
            <svg class="shp" viewBox="0 0 12 12" aria-hidden="true" inner_html=shape_markup(shape, color)></svg>
            <span>{outcome.text.clone()}</span>
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_tool_is_told_to_install_before_testing() {
        assert_eq!(
            test_hint(ToolStatus::Missing),
            "Install this agent before you test it."
        );
    }

    #[test]
    fn a_found_tool_is_told_the_test_is_small_and_may_be_slow() {
        assert_eq!(
            test_hint(ToolStatus::Ready),
            "Sends one tiny prompt. It can take up to a minute."
        );
    }

    #[test]
    fn a_passing_test_is_a_green_dot() {
        assert_eq!(test_look(true), ("ok", "dot"));
    }

    #[test]
    fn a_failing_test_is_a_red_ring() {
        assert_eq!(test_look(false), ("danger", "oct"));
    }

    #[test]
    fn a_network_failure_gets_the_plain_daemon_sentence() {
        let failure = ApiFailure::Network("connection refused".to_owned());
        assert_eq!(failure_text(&failure), "Could not reach the daemon.");
    }

    #[test]
    fn a_conflict_shows_the_daemon_message() {
        let failure = ApiFailure::Status {
            code: 409,
            message: "A test of this agent is already running.".to_owned(),
        };
        assert_eq!(
            failure_text(&failure),
            "A test of this agent is already running."
        );
    }

    #[test]
    fn the_daemon_answer_reads_into_a_test() {
        let json = r#"{"tool":"claude","ok":true,"message":"Replied in 2 s.","millis":2000}"#;
        let answer: ToolTest = serde_json::from_str(json).expect("answer json is valid");
        let outcome = TestOutcome::from_answer(&answer);
        assert_eq!(
            outcome,
            TestOutcome {
                ok: true,
                text: "Replied in 2 s.".to_owned(),
            }
        );
    }
}
