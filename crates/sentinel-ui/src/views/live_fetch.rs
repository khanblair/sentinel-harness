//! Keeps a panel's data fresh while the panel is open. It loads on mount, again when the event id
//! moves (at most once a second, so a busy run does not flood the daemon), and every five seconds.
//! The timer stops when the panel closes.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use leptos::prelude::*;

use crate::state::Ui;

const EVERY: Duration = Duration::from_secs(5);
const MIN_GAP_MS: f64 = 1_000.0;

/// Calls `load` now, and keeps calling it while the panel is mounted.
pub fn keep_live(ui: Ui, load: impl Fn() + Clone + 'static) {
    load();
    let last_load = Rc::new(Cell::new(js_sys::Date::now()));
    let event_id = Memo::new(move |_| ui.snapshot.with(|snapshot| snapshot.last_event_id));
    let on_event = load.clone();
    Effect::new(move |seen: Option<i64>| {
        let now = event_id.get();
        if seen.is_some_and(|previous| previous != now) {
            throttled(&last_load, &on_event);
        }
        now
    });
    let timer = set_interval_with_handle(load, EVERY);
    if let Ok(handle) = timer {
        on_cleanup(move || handle.clear());
    }
}

/// Runs `load` unless it ran less than a second ago.
fn throttled(last_load: &Cell<f64>, load: &impl Fn()) {
    let now = js_sys::Date::now();
    if now - last_load.get() >= MIN_GAP_MS {
        last_load.set(now);
        load();
    }
}
