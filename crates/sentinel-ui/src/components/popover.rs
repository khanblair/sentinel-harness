//! The one popover of the page. It sits under its anchor, flips above when there is no room
//! below, and stays inside the app (`docs/ui-rules.md`, Responsive Layouts).

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::components::menus;
use crate::state::{Align, Popover, PopoverKind, Ui, use_ui};

const GAP: f64 = 6.0;
const EDGE: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

/// Where the popover goes, relative to the app root. `max_height` keeps it inside the app.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub left: f64,
    pub top: f64,
    pub max_height: f64,
}

/// Places a popover below its anchor, or above when it does not fit below, and keeps it inside
/// the viewport. All values are in the same coordinate space as `anchor`.
pub fn place(anchor: Rect, popover: Size, viewport: Size, align: Align) -> Placement {
    let anchor_bottom = anchor.top + anchor.height;
    let wanted_left = match align {
        Align::Left => anchor.left,
        Align::Right => anchor.left + anchor.width - popover.width,
    };
    let left = wanted_left.clamp(EDGE, (viewport.width - popover.width - EDGE).max(EDGE));
    let room_below = viewport.height - anchor_bottom - GAP - EDGE;
    let wanted_top = if popover.height <= room_below {
        anchor_bottom + GAP
    } else {
        anchor.top - GAP - popover.height
    };
    let max_height = (viewport.height - 2.0 * EDGE).max(0.0);
    let top = wanted_top.clamp(EDGE, (viewport.height - EDGE - popover.height).max(EDGE));
    Placement {
        left,
        top,
        max_height,
    }
}

/// Opens the menu, or closes it when the same anchor and kind are already open.
pub fn toggle_popover(ui: Ui, anchor_id: &str, align: Align, kind: PopoverKind) {
    let next = Popover {
        anchor_id: anchor_id.to_owned(),
        align,
        kind,
    };
    ui.popover.update(|current| {
        *current = if current.as_ref() == Some(&next) {
            None
        } else {
            Some(next)
        };
    });
}

/// Closes the menu when a click lands outside it and outside its anchor. A click on the anchor
/// is left to the anchor, which toggles the menu.
pub fn install_outside_click(ui: Ui) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let on_click = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        let Some(open) = ui.popover.get_untracked() else {
            return;
        };
        if !clicked_inside(&event, &open.anchor_id) {
            ui.popover.set(None);
        }
    });
    let _ = document.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref());
    on_click.forget();
}

fn clicked_inside(event: &web_sys::Event, anchor_id: &str) -> bool {
    let Some(element) = event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
    else {
        return false;
    };
    let selector = format!("#pop, #{anchor_id}");
    element.closest(&selector).ok().flatten().is_some()
}

/// The rectangle of an element, relative to the app root `#app`.
fn rect_in_app(document: &web_sys::Document, id: &str) -> Option<Rect> {
    let origin = document
        .get_element_by_id("app")?
        .get_bounding_client_rect();
    let target = document.get_element_by_id(id)?.get_bounding_client_rect();
    Some(Rect {
        left: target.left() - origin.left(),
        top: target.top() - origin.top(),
        width: target.width(),
        height: target.height(),
    })
}

/// Measures the anchor and the menu, then returns where the menu goes.
fn measure(popover: &Popover, element: &web_sys::HtmlElement) -> Option<Placement> {
    let document = web_sys::window()?.document()?;
    let anchor = rect_in_app(&document, &popover.anchor_id)?;
    let app = document
        .get_element_by_id("app")?
        .get_bounding_client_rect();
    let size = Size {
        width: f64::from(element.offset_width()),
        height: f64::from(element.offset_height()),
    };
    let viewport = Size {
        width: app.width(),
        height: app.height(),
    };
    Some(place(anchor, size, viewport, popover.align))
}

#[component]
pub fn PopoverHost() -> impl IntoView {
    let ui = use_ui();
    let node = NodeRef::<leptos::html::Div>::new();
    let placement = RwSignal::new(Placement {
        left: 0.0,
        top: 0.0,
        max_height: 0.0,
    });
    Effect::new(move |_| {
        let Some(open) = ui.popover.get() else {
            return;
        };
        let Some(element) = node.get() else {
            return;
        };
        let element: &web_sys::HtmlElement = &element;
        if let Some(next) = measure(&open, element) {
            placement.set(next);
        }
    });
    move || {
        ui.popover.get().map(|open| {
            view! {
                <div class="pop" id="pop" node_ref=node
                    style=move || {
                        let spot = placement.get();
                        format!("left:{}px;top:{}px;max-height:{}px", spot.left, spot.top, spot.max_height)
                    }>
                    {menus::body(open.kind, ui)}
                </div>
            }
        })
    }
}
