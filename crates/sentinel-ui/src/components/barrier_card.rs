//! The barrier card: the full test suite on a clean tree after a wave.

use leptos::prelude::*;

use crate::components::icon::Icon;
use crate::views::stream_model::BarrierState;

fn barrier_body(state: BarrierState) -> &'static str {
    match state {
        BarrierState::Running => "Tree is clean. Running the full test suite.",
        BarrierState::Pass => "Full suite passed.",
        BarrierState::Fail => "Suite failed. Work stops until it passes.",
    }
}

fn barrier_mark(state: BarrierState) -> AnyView {
    match state {
        BarrierState::Running => view! { <span class="spin"></span> }.into_any(),
        BarrierState::Pass => view! {
            <span style="color:var(--ok)"><Icon name="check" size=16 /></span>
        }
        .into_any(),
        BarrierState::Fail => view! {
            <span style="color:var(--danger)"><Icon name="x" size=16 /></span>
        }
        .into_any(),
    }
}

#[component]
pub fn BarrierCard(wave: u32, state: BarrierState) -> impl IntoView {
    view! {
        <div class="card">
            <h4>{barrier_mark(state)}{format!(" Barrier after wave {wave}")}</h4>
            <div class="sub">{barrier_body(state)}</div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_barrier_state_has_its_own_sentence() {
        assert_eq!(barrier_body(BarrierState::Pass), "Full suite passed.");
        assert!(barrier_body(BarrierState::Fail).starts_with("Suite failed"));
        assert!(barrier_body(BarrierState::Running).contains("Running"));
    }
}
