//! `TabBar`: the phone tabs Stream, Board, Reserved, Changes, More, and the swipe between them
//! (`docs/ui-rules.md`, Responsive Layouts). CSS shows the bar on phone only.

use leptos::prelude::*;
use wasm_bindgen::JsValue;

use crate::components::icon::Icon;
use crate::components::popover::toggle_popover;
use crate::layout::{Layout, PanelId};
use crate::state::{Align, PopoverKind, Ui, use_ui};

/// The tabs in bar order: id, icon, label.
const TABS: [(&str, &str, &str); 5] = [
    ("stream", "code", "Stream"),
    ("board", "tasks", "Board"),
    ("reserved", "lock", "Reserved"),
    ("changes", "diff", "Changes"),
    ("more", "more", "More"),
];

/// Tabs a swipe can reach. More opens a menu, so a swipe never lands on it.
const SWIPE_TABS: [&str; 4] = ["stream", "board", "reserved", "changes"];
/// Horizontal distance in pixels that counts as a swipe.
const SWIPE_MIN_PX: f64 = 50.0;

/// The tab that is selected: Stream when no panel is open, the panel's tab for the three panels
/// that have one, else More.
pub fn selected_tab(panel_open: bool, active: Option<PanelId>) -> &'static str {
    if !panel_open {
        return "stream";
    }
    active.and_then(tab_for_panel).unwrap_or("more")
}

fn tab_for_panel(panel: PanelId) -> Option<&'static str> {
    match panel {
        PanelId::Board => Some("board"),
        PanelId::Reserved => Some("reserved"),
        PanelId::Changes => Some("changes"),
        PanelId::Terminal
        | PanelId::Brain
        | PanelId::Remote
        | PanelId::Files
        | PanelId::Cost
        | PanelId::Report
        | PanelId::Agents
        | PanelId::Permissions
        | PanelId::Telegram
        | PanelId::Context => None,
    }
}

/// The tab a swipe moves to, or `None` when the swipe is too short or the current tab has no
/// neighbour that way. A left swipe moves forward.
pub fn swipe_target(current: &str, delta_x: f64) -> Option<&'static str> {
    if delta_x.abs() < SWIPE_MIN_PX {
        return None;
    }
    let index = SWIPE_TABS.iter().position(|tab| *tab == current)?;
    if delta_x < 0.0 {
        SWIPE_TABS.get(index + 1).copied()
    } else {
        index
            .checked_sub(1)
            .and_then(|prev| SWIPE_TABS.get(prev).copied())
    }
}

/// Selects a tab. The More tab opens its menu from the button with id `tabMore`.
pub fn pick_tab(ui: Ui, tab: &str) {
    match tab {
        "stream" => ui.panel_open.set(false),
        "board" => ui.open_panel(PanelId::Board),
        "reserved" => ui.open_panel(PanelId::Reserved),
        "changes" => ui.open_panel(PanelId::Changes),
        "more" => toggle_popover(ui, "tabMore", Align::Right, PopoverKind::More),
        _ => {}
    }
}

/// Reads one touch coordinate. `list` is `touches` or `changedTouches`. The read goes through
/// `js_sys`, so this file does not need the `TouchList` web-sys features.
pub fn touch_x(event: &web_sys::Event, list: &str) -> Option<f64> {
    let target: &JsValue = event.as_ref();
    let touches = js_sys::Reflect::get(target, &JsValue::from_str(list)).ok()?;
    let first = js_sys::Reflect::get(&touches, &JsValue::from_f64(0.0)).ok()?;
    js_sys::Reflect::get(&first, &JsValue::from_str("clientX"))
        .ok()?
        .as_f64()
}

/// Records where a touch began on the main area. Only the phone layout swipes.
pub fn touch_started(ui: Ui, start: RwSignal<Option<f64>>, event: &web_sys::Event) {
    if ui.layout.get_untracked() == Layout::Phone {
        start.set(touch_x(event, "touches"));
    }
}

/// Ends a touch and switches tab when it was a swipe.
pub fn touch_ended(ui: Ui, start: RwSignal<Option<f64>>, event: &web_sys::Event) {
    let Some(from) = start.get_untracked() else {
        return;
    };
    start.set(None);
    let Some(to) = touch_x(event, "changedTouches") else {
        return;
    };
    let current = selected_tab(
        ui.panel_open.get_untracked(),
        ui.active_panel.get_untracked(),
    );
    if let Some(next) = swipe_target(current, to - from) {
        pick_tab(ui, next);
    }
}

#[component]
pub fn TabBar() -> impl IntoView {
    let ui = use_ui();
    let selected = move || selected_tab(ui.panel_open.get(), ui.active_panel.get());
    view! {
        <nav class="tabbar" id="tabbar" aria-label="Sections">
            {TABS
                .into_iter()
                .map(|(id, icon, label)| view! {
                    <button role="tab" id=(id == "more").then_some("tabMore")
                        aria-selected=move || selected() == id
                        on:click=move |_| pick_tab(ui, id)>
                        <Icon name=icon size=20 />
                        <span>{label}</span>
                    </button>
                })
                .collect_view()}
        </nav>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_is_selected_when_no_panel_is_open() {
        assert_eq!(selected_tab(false, Some(PanelId::Board)), "stream");
    }

    #[test]
    fn the_three_panel_tabs_select_their_own_tab() {
        assert_eq!(selected_tab(true, Some(PanelId::Board)), "board");
        assert_eq!(selected_tab(true, Some(PanelId::Reserved)), "reserved");
        assert_eq!(selected_tab(true, Some(PanelId::Changes)), "changes");
    }

    #[test]
    fn other_panels_select_more() {
        assert_eq!(selected_tab(true, Some(PanelId::Cost)), "more");
        assert_eq!(selected_tab(true, None), "more");
    }

    #[test]
    fn a_left_swipe_moves_forward_and_a_right_swipe_moves_back() {
        assert_eq!(swipe_target("stream", -120.0), Some("board"));
        assert_eq!(swipe_target("board", 120.0), Some("stream"));
    }

    #[test]
    fn a_short_swipe_or_an_edge_does_nothing() {
        assert_eq!(swipe_target("stream", 20.0), None);
        assert_eq!(swipe_target("stream", 120.0), None);
        assert_eq!(swipe_target("changes", -120.0), None);
    }

    #[test]
    fn a_swipe_never_reaches_more() {
        assert_eq!(swipe_target("changes", -200.0), None);
        assert_eq!(swipe_target("more", -200.0), None);
    }
}
