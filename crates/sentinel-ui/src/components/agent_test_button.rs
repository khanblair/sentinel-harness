//! The Test button of an agent row and the line that shows its answer. A test sends one tiny
//! prompt through `POST /api/tools/{id}/test`. The daemon can take up to 90 seconds on it.
//!
//! The state of each test lives in `Ui::agent_tests`, keyed by tool, not in the row. A snapshot
//! refresh rebuilds the rows, and the answer must survive that.

use std::collections::HashMap;

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::ToolTest;
use sentinel_core::{ToolId, ToolStatus};

use crate::api::{self, ApiFailure};
use crate::components::status_badge::shape_markup;
use crate::state::{Ui, use_ui};

/// What the line under a row shows after a test: the daemon's sentence, or a failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestOutcome {
    pub ok: bool,
    pub text: String,
}

/// Where one agent's test stands. A tool never tested is idle. A done test keeps its answer
/// until the next test of that tool or a page reload.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TestState {
    #[default]
    Idle,
    Running,
    Done(TestOutcome),
}

/// The state of one tool's test. A tool with no entry is idle.
pub fn state_of(map: &HashMap<ToolId, TestState>, id: &ToolId) -> TestState {
    map.get(id).cloned().unwrap_or_default()
}

/// True while a test of this tool runs. Every row that shows the tool reads the same answer.
pub fn is_running(map: &HashMap<ToolId, TestState>, id: &ToolId) -> bool {
    matches!(map.get(id), Some(TestState::Running))
}

/// Stores one tool's test state. The map is the shared home of the answer, so a rebuilt row
/// reads the same entry again.
pub fn set_state(map: &mut HashMap<ToolId, TestState>, id: &ToolId, state: TestState) {
    map.insert(id.clone(), state);
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

/// Marks the tool as testing, sends the test, and stores the answer under the tool's id. The
/// button stays disabled until the answer lands.
fn run_test(ui: Ui, id: ToolId) {
    ui.agent_tests
        .update(|map| set_state(map, &id, TestState::Running));
    spawn_local(async move {
        let path = format!("/api/tools/{id}/test");
        let outcome = match api::post_empty::<ToolTest>(&path).await {
            Ok(answer) => TestOutcome::from_answer(&answer),
            Err(failure) => TestOutcome::from_failure(&failure),
        };
        ui.agent_tests
            .update(|map| set_state(map, &id, TestState::Done(outcome)));
    });
}

/// The small Test button. A missing tool cannot be tested, and a phone does not show it.
#[component]
pub fn AgentTestButton(id: ToolId, status: ToolStatus) -> impl IntoView {
    let ui = use_ui();
    let missing = status == ToolStatus::Missing;
    let busy_id = id.clone();
    let busy = Memo::new(move |_| ui.agent_tests.with(|map| is_running(map, &busy_id)));
    view! {
        <Show when=move || !ui.on_phone() fallback=|| ()>
            {
                let run_id = id.clone();
                view! {
                    <button
                        class="btn sm"
                        disabled=move || missing || busy.get()
                        on:click=move |_| run_test(ui, run_id.clone())
                    >
                        {move || if busy.get() { "Testing" } else { "Test" }}
                    </button>
                }
            }
        </Show>
    }
}

/// The hint while a test runs, then the answer until the next test.
#[component]
pub fn AgentTestLine(id: ToolId, status: ToolStatus) -> impl IntoView {
    let ui = use_ui();
    let state = Memo::new(move |_| ui.agent_tests.with(|map| state_of(map, &id)));
    view! {
        <Show when=move || !ui.on_phone() fallback=|| ()>
            {move || test_line(state.get(), status)}
        </Show>
    }
}

fn test_line(state: TestState, status: ToolStatus) -> AnyView {
    if state == TestState::Running || status == ToolStatus::Missing {
        return view! { <small class="sub">{test_hint(status)}</small> }.into_any();
    }
    match state {
        TestState::Done(outcome) => outcome_line(&outcome).into_any(),
        TestState::Idle | TestState::Running => ().into_any(),
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

    fn outcome(ok: bool, text: &str) -> TestOutcome {
        TestOutcome {
            ok,
            text: text.to_owned(),
        }
    }

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

    #[test]
    fn a_result_stays_under_its_tool_after_the_rows_rebuild() {
        let mut map = HashMap::new();
        let claude = ToolId::new("claude");
        set_state(
            &mut map,
            &claude,
            TestState::Done(outcome(true, "Replied.")),
        );
        assert_eq!(
            state_of(&map, &claude),
            TestState::Done(outcome(true, "Replied."))
        );
    }

    #[test]
    fn a_new_test_replaces_the_old_result() {
        let mut map = HashMap::new();
        let claude = ToolId::new("claude");
        set_state(
            &mut map,
            &claude,
            TestState::Done(outcome(false, "Timed out.")),
        );
        set_state(&mut map, &claude, TestState::Running);
        assert!(is_running(&map, &claude));
        assert_eq!(state_of(&map, &claude), TestState::Running);
    }

    #[test]
    fn a_result_of_one_tool_leaves_the_others_alone() {
        let mut map = HashMap::new();
        let claude = ToolId::new("claude");
        let codex = ToolId::new("codex");
        set_state(
            &mut map,
            &claude,
            TestState::Done(outcome(true, "Replied.")),
        );
        assert_eq!(state_of(&map, &codex), TestState::Idle);
        assert!(!is_running(&map, &codex));
    }

    #[test]
    fn a_tool_never_tested_reads_as_idle() {
        let map = HashMap::new();
        assert_eq!(state_of(&map, &ToolId::new("aider")), TestState::Idle);
    }
}
