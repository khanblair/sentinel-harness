//! `TourCoach`: the highlight ring and the coach mark for the current stop. It draws nothing
//! unless a tour runs. Where it sits is worked out in `tour_state.rs`.

use leptos::prelude::*;

use crate::components::tab_bar::touch_x;
use crate::state::{Ui, use_ui};
use crate::tour::{Move, Rect, STOPS, TourStop, move_for_swipe};
use crate::tour_state::{Spot, Tour, apply_move, listen_for_keys, use_tour};

#[component]
pub fn TourCoach() -> impl IntoView {
    let ui = use_ui();
    let tour = use_tour();
    listen_for_keys(ui, tour);
    move || tour.step.get().map(|index| view! { <TourCard ui index /> })
}

#[component]
fn TourCard(ui: Ui, index: usize) -> impl IntoView {
    let tour = use_tour();
    let swipe_from = RwSignal::new(None::<f64>);
    view! {
        <div class="tour" id="tour"
            on:touchstart=move |event| swipe_from.set(touch_x(&event, "touches"))
            on:touchend=move |event| finish_swipe(ui, tour, swipe_from, &event)>
            {move || tour.spot.get().map(|spot| highlight_ring(spot.ring))}
            <div class="tour-tip" id="tip" style=move || tip_style(tour.spot.get())>
                {STOPS.get(index).copied().map(|stop| tip_body(ui, tour, index, stop))}
            </div>
        </div>
    }
}

fn highlight_ring(ring: Rect) -> impl IntoView {
    view! { <div class="tour-hl" style=ring_style(ring)></div> }
}

fn tip_body(ui: Ui, tour: Tour, index: usize, stop: TourStop) -> impl IntoView {
    let total = STOPS.len();
    let label = if index + 1 == total { "Done" } else { "Next" };
    view! {
        <h4>{stop.title}</h4>
        <p>{stop.body}</p>
        <div class="row sp">
            <span class="sub">{format!("{} of {}", index + 1, total)}</span>
            <span class="row">
                <button class="btn sm" on:click=move |_| apply_move(ui, tour, Move::End)>"Skip"</button>
                <button class="btn sm" disabled=index == 0
                    on:click=move |_| apply_move(ui, tour, Move::Back)>"Back"</button>
                <button class="btn sm pri" on:click=move |_| apply_move(ui, tour, Move::Next)>{label}</button>
            </span>
        </div>
    }
}

/// A horizontal swipe moves the tour. The start point comes from `touchstart`.
fn finish_swipe(ui: Ui, tour: Tour, start: RwSignal<Option<f64>>, event: &web_sys::Event) {
    let Some(from) = start.get_untracked() else {
        return;
    };
    start.set(None);
    let Some(to) = touch_x(event, "changedTouches") else {
        return;
    };
    if let Some(step) = move_for_swipe(to - from) {
        apply_move(ui, tour, step);
    }
}

fn ring_style(ring: Rect) -> String {
    format!(
        "left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px",
        ring.x, ring.y, ring.w, ring.h
    )
}

/// Before the stop is measured the card is hidden, but still laid out so its size can be read.
fn tip_style(spot: Option<Spot>) -> String {
    match spot {
        Some(spot) => format!("left:{:.1}px;top:{:.1}px", spot.tip.x, spot.tip.y),
        None => "left:0px;top:0px;visibility:hidden".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tour::{Placement, Side};

    #[test]
    fn the_ring_sits_on_the_anchor_box() {
        let ring = Rect {
            x: 10.0,
            y: 20.0,
            w: 100.0,
            h: 30.0,
        };
        assert_eq!(
            ring_style(ring),
            "left:10.0px;top:20.0px;width:100.0px;height:30.0px"
        );
    }

    #[test]
    fn the_card_is_hidden_until_its_stop_is_measured() {
        assert_eq!(tip_style(None), "left:0px;top:0px;visibility:hidden");
    }

    #[test]
    fn a_measured_card_moves_to_its_placement() {
        let spot = Spot {
            ring: Rect {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
            },
            tip: Placement {
                x: 12.0,
                y: 40.5,
                side: Side::Below,
            },
        };
        assert_eq!(tip_style(Some(spot)), "left:12.0px;top:40.5px");
    }
}
