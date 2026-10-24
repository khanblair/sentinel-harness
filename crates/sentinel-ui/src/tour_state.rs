//! The tour's state: which stop shows, where its highlight and coach mark go, and the seen flag.
//! Settings and the palette start the tour from click handlers, so `start` reads the state from
//! a thread-local set by `provide_tour` instead of a context lookup.

use std::cell::RefCell;

use gloo_storage::{LocalStorage, Storage};
use gloo_timers::callback::Timeout;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Element, KeyboardEvent};

use crate::layout::Layout;
use crate::state::{ToastKind, Ui};
use crate::tour::{
    Move, Placement, Rect, STOPS, Size, TourStop, effective_pin, move_for_key, next_stop, place,
    previous_stop,
};

pub const SEEN_KEY: &str = "sentinel.tour.seen";
const FINISHED_TEXT: &str = "Tour finished. Replay it from Settings.";
/// Phone animates the drawer, so the anchor is measured after it settles.
const PHONE_SETTLE_MS: u32 = 320;
const SETTLE_MS: u32 = 30;
/// Padding of the highlight ring around the anchor, in px.
const RING_PAD: f64 = 4.0;

/// Where the highlight ring and the coach mark go, relative to the app.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spot {
    pub ring: Rect,
    pub tip: Placement,
}

#[derive(Clone, Copy)]
pub struct Tour {
    /// The stop on screen, or `None` when no tour runs.
    pub step: RwSignal<Option<usize>>,
    /// `None` until the current stop is measured.
    pub spot: RwSignal<Option<Spot>>,
}

thread_local! {
    static CURRENT: RefCell<Option<Tour>> = const { RefCell::new(None) };
}

/// Creates the tour state once, at the app root.
pub fn provide_tour() {
    let tour = Tour {
        step: RwSignal::new(None),
        spot: RwSignal::new(None),
    };
    provide_context(tour);
    CURRENT.with(|current| *current.borrow_mut() = Some(tour));
}

pub fn use_tour() -> Tour {
    expect_context::<Tour>()
}

fn current() -> Option<Tour> {
    CURRENT.with(|current| *current.borrow())
}

/// Starts the tour at the first stop and marks it seen on this device.
pub fn start(ui: Ui) {
    let Some(tour) = current() else {
        return;
    };
    mark_seen();
    ui.popover.set(None);
    go_to(ui, tour, 0);
}

/// Ends the tour from Skip, Escape, or the replay menu.
pub fn end(ui: Ui) {
    let Some(tour) = current() else {
        return;
    };
    finish(ui, tour);
}

pub fn seen() -> bool {
    LocalStorage::get::<bool>(SEEN_KEY).unwrap_or(false)
}

fn mark_seen() {
    let _ = LocalStorage::set(SEEN_KEY, true);
}

/// Moves the tour by a key or swipe. Back on the first stop does nothing.
pub fn apply_move(ui: Ui, tour: Tour, step: Move) {
    let Some(index) = tour.step.get_untracked() else {
        return;
    };
    match step {
        Move::Next => go_to(
            ui,
            tour,
            next_stop(index, STOPS.len()).unwrap_or(STOPS.len()),
        ),
        Move::Back if index > 0 => go_to(ui, tour, previous_stop(index)),
        Move::Back => {}
        Move::End => finish(ui, tour),
    }
}

/// Shows a stop. The anchor is measured after a short wait, so the layout change has landed.
fn go_to(ui: Ui, tour: Tour, index: usize) {
    if index >= STOPS.len() {
        finish(ui, tour);
        ui.show_toast(FINISHED_TEXT, ToastKind::Info);
        return;
    }
    tour.step.set(Some(index));
    tour.spot.set(None);
    if let Some(stop) = STOPS.get(index) {
        apply_stop_layout(ui, stop);
    }
    let delay = match ui.layout.get_untracked() {
        Layout::Phone => PHONE_SETTLE_MS,
        Layout::Tablet | Layout::Desktop => SETTLE_MS,
    };
    Timeout::new(delay, move || measure(ui, tour, index)).forget();
}

/// Phone opens the drawer only for the stops that need it. Tablet and phone close the panels.
fn apply_stop_layout(ui: Ui, stop: &TourStop) {
    match ui.layout.get_untracked() {
        Layout::Phone => {
            ui.sidebar_open.set(stop.opens_sidebar);
            ui.panel_open.set(false);
        }
        Layout::Tablet => ui.panel_open.set(false),
        Layout::Desktop => {}
    }
}

fn finish(ui: Ui, tour: Tour) {
    tour.step.set(None);
    tour.spot.set(None);
    if ui.layout.get_untracked() == Layout::Phone {
        ui.sidebar_open.set(false);
    }
}

/// Runs after the wait. A stop with no anchor on screen is skipped.
fn measure(ui: Ui, tour: Tour, index: usize) {
    if tour.step.get_untracked() != Some(index) {
        return;
    }
    let Some(stop) = STOPS.get(index) else {
        return;
    };
    let Some(app) = element_rect("app") else {
        return;
    };
    let Some(ring) = anchor_rect(stop.anchors, app) else {
        go_to(ui, tour, index + 1);
        return;
    };
    let Some(card) = element_size("tip") else {
        return;
    };
    let viewport = Size { w: app.w, h: app.h };
    let pin = effective_pin(stop.pin, ui.layout.get_untracked() == Layout::Desktop);
    let tip = place(pin, ring, card, viewport);
    tour.spot.set(Some(Spot { ring, tip }));
}

fn document() -> Option<web_sys::Document> {
    web_sys::window()?.document()
}

/// An element's box, in viewport pixels.
fn element_rect(id: &str) -> Option<Rect> {
    let element = document()?.get_element_by_id(id)?;
    Some(to_rect(&element))
}

fn element_size(id: &str) -> Option<Size> {
    element_rect(id).map(|rect| Size {
        w: rect.w,
        h: rect.h,
    })
}

fn to_rect(element: &Element) -> Rect {
    let bounds = element.get_bounding_client_rect();
    Rect {
        x: bounds.left(),
        y: bounds.top(),
        w: bounds.width(),
        h: bounds.height(),
    }
}

/// The first anchor that is on screen, as a ring around it, relative to the app.
fn anchor_rect(selectors: &[&str], app: Rect) -> Option<Rect> {
    let document = document()?;
    selectors.iter().find_map(|selector| {
        let element = document.query_selector(selector).ok()??;
        let bounds = to_rect(&element);
        (bounds.w > 0.0).then_some(Rect {
            x: bounds.x - app.x - RING_PAD,
            y: bounds.y - app.y - RING_PAD,
            w: bounds.w + 2.0 * RING_PAD,
            h: bounds.h + 2.0 * RING_PAD,
        })
    })
}

/// Arrows go back and forward, Escape ends. Typing in a field keeps its keys.
pub fn listen_for_keys(ui: Ui, tour: Tour) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let on_key = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
        handle_key(ui, tour, &event);
    });
    let _ = window.add_event_listener_with_callback("keydown", on_key.as_ref().unchecked_ref());
    on_key.forget();
}

fn handle_key(ui: Ui, tour: Tour, event: &KeyboardEvent) {
    if tour.step.get_untracked().is_none() || is_typing(event) {
        return;
    }
    if let Some(step) = move_for_key(&event.key()) {
        event.prevent_default();
        apply_move(ui, tour, step);
    }
}

fn is_typing(event: &KeyboardEvent) -> bool {
    event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok())
        .is_some_and(|element| matches!(element.tag_name().as_str(), "TEXTAREA" | "INPUT"))
}
