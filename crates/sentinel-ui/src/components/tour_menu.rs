//! `TourMenu`: shows how far the tour got, and replays or skips it. Settings uses it.

use leptos::prelude::*;

use crate::state::use_ui;
use crate::tour::STOPS;
use crate::tour_state::{end, start, use_tour};

#[component]
pub fn TourMenu() -> impl IntoView {
    let ui = use_ui();
    let tour = use_tour();
    view! {
        <div class="row sp wrap">
            <span class="sub">{move || progress_text(tour.step.get())}</span>
            <span class="row">
                <button class="btn sm" disabled=move || tour.step.get().is_none()
                    on:click=move |_| end(ui)>"Skip tour"</button>
                <button class="btn sm pri" on:click=move |_| start(ui)>"Replay tour"</button>
            </span>
        </div>
    }
}

/// `Stop 3 of 7` while the tour runs, `Not running` otherwise.
pub fn progress_text(step: Option<usize>) -> String {
    match step {
        Some(index) => format!("Stop {} of {}", index + 1, STOPS.len()),
        None => "Not running".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_counts_stops_from_one() {
        assert_eq!(progress_text(Some(2)), "Stop 3 of 7");
    }

    #[test]
    fn progress_says_when_no_tour_runs() {
        assert_eq!(progress_text(None), "Not running");
    }
}
