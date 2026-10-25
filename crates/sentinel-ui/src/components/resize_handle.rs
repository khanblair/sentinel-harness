//! Drag handles on the desktop layout: one on the sidebar edge, one on the panel edge. The widths
//! are kept on this device (`docs/ui-rules.md`, Layout). `ResizeHandles` mounts both handles and
//! writes the width variables on the shell element.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::layout::{
    Layout, PANEL_DEFAULT, PANEL_MAX, PANEL_MIN, SIDEBAR_DEFAULT, SIDEBAR_MAX, SIDEBAR_MIN,
    clamp_widths,
};
use crate::prefs::Prefs;
use crate::state::{Ui, use_ui};

/// Pixels one arrow key press moves a handle.
const KEY_STEP: i32 = 16;
/// Pixels one arrow key press moves a handle with Shift held.
const KEY_STEP_LARGE: i32 = 64;
/// Body class while a drag runs. It turns off text selection and sets the column cursor.
const DRAG_CLASS: &str = "col-resize";
/// Id of the shell element. The width variables are set on it.
const SHELL_ID: &str = "app";
/// Used when the page width cannot be read. Desktop is the widest layout.
const FALLBACK_VIEWPORT: u32 = 1280;
/// `MouseEvent.button` value of the main button.
const PRIMARY_BUTTON: i16 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleSide {
    Sidebar,
    Panel,
}

impl HandleSide {
    fn class(self) -> &'static str {
        match self {
            HandleSide::Sidebar => "rz rz-sb",
            HandleSide::Panel => "rz rz-rp",
        }
    }

    fn label(self) -> &'static str {
        match self {
            HandleSide::Sidebar => "Resize sidebar",
            HandleSide::Panel => "Resize panel",
        }
    }

    fn min(self) -> u32 {
        match self {
            HandleSide::Sidebar => SIDEBAR_MIN,
            HandleSide::Panel => PANEL_MIN,
        }
    }

    fn max(self) -> u32 {
        match self {
            HandleSide::Sidebar => SIDEBAR_MAX,
            HandleSide::Panel => PANEL_MAX,
        }
    }

    fn default_px(self) -> u32 {
        match self {
            HandleSide::Sidebar => SIDEBAR_DEFAULT,
            HandleSide::Panel => PANEL_DEFAULT,
        }
    }

    fn clamp(self, px: u32) -> u32 {
        px.clamp(self.min(), self.max())
    }

    fn width(self, ui: Ui) -> RwSignal<u32> {
        match self {
            HandleSide::Sidebar => ui.sidebar_width,
            HandleSide::Panel => ui.panel_width,
        }
    }

    fn store(self, prefs: &mut Prefs, px: u32) {
        match self {
            HandleSide::Sidebar => prefs.sidebar_w = px,
            HandleSide::Panel => prefs.panel_w = px,
        }
    }
}

/// The width a pointer drag ends on, before clamping. The sidebar grows as the pointer moves right.
/// The panel grows as the pointer moves left.
pub fn width_from_pointer(start_width: u32, start_x: i32, x: i32, side: HandleSide) -> i32 {
    let moved = x - start_x;
    let start = i32::try_from(start_width).unwrap_or(i32::MAX);
    match side {
        HandleSide::Sidebar => start + moved,
        HandleSide::Panel => start - moved,
    }
}

/// The change one arrow key makes, or `None` for any other key. Shift makes the step larger.
pub fn key_delta(key: &str, shift: bool, side: HandleSide) -> Option<i32> {
    let step = if shift { KEY_STEP_LARGE } else { KEY_STEP };
    let grows_right = match key {
        "ArrowRight" => true,
        "ArrowLeft" => false,
        _ => return None,
    };
    // Pressing right grows the sidebar. Pressing left grows the panel.
    let outward = grows_right == (side == HandleSide::Sidebar);
    Some(if outward { step } else { -step })
}

/// Adds a signed change to a width. Never goes below zero.
fn shifted(width: u32, delta: i32) -> u32 {
    let total = i32::try_from(width)
        .unwrap_or(i32::MAX)
        .saturating_add(delta);
    px_from(total)
}

fn px_from(value: i32) -> u32 {
    u32::try_from(value).unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Drag {
    start_width: u32,
    start_x: i32,
}

/// One drag handle. It takes the side it sits on.
#[component]
pub fn ResizeHandle(side: HandleSide) -> impl IntoView {
    let ui = use_ui();
    let drag = RwSignal::new(None::<Drag>);
    // A handle that unmounts mid-drag never gets its pointerup. Clear the body class here.
    on_cleanup(|| mark_body(false));
    let finish = move |_: web_sys::PointerEvent| end_drag(ui, drag, side);
    view! {
        <div class=side.class() role="separator" aria-orientation="vertical"
            aria-label=side.label()
            aria-valuenow=move || side.width(ui).get().to_string()
            aria-valuemin=side.min().to_string()
            aria-valuemax=side.max().to_string()
            tabindex="0"
            on:pointerdown=move |event: web_sys::PointerEvent| start_drag(ui, drag, side, &event)
            on:pointermove=move |event: web_sys::PointerEvent| move_drag(ui, drag, side, &event)
            on:pointerup=finish
            on:pointercancel=finish
            on:dblclick=move |_: web_sys::MouseEvent| reset_width(ui, side)
            on:keydown=move |event: web_sys::KeyboardEvent| press_key(ui, side, &event)
        ></div>
    }
}

/// Both handles, shown only where they can be dragged, and the width variables on the shell.
/// Mount it inside `.app`.
#[component]
pub fn ResizeHandles() -> impl IntoView {
    let ui = use_ui();
    // No tracked reads, so this runs once and the resize listener is added once.
    Effect::new(move |_| install_viewport(ui));
    Effect::new(move |_| apply_shell_widths(ui));
    view! {
        <Show when=move || ui.sidebar_resizable()>
            <ResizeHandle side=HandleSide::Sidebar />
        </Show>
        <Show when=move || ui.panel_resizable()>
            <ResizeHandle side=HandleSide::Panel />
        </Show>
    }
}

fn start_drag(
    ui: Ui,
    drag: RwSignal<Option<Drag>>,
    side: HandleSide,
    event: &web_sys::PointerEvent,
) {
    if event.button() != PRIMARY_BUTTON {
        return;
    }
    // Stops the browser from starting a text selection on the way down.
    event.prevent_default();
    capture_pointer(event);
    drag.set(Some(Drag {
        start_width: side.width(ui).get_untracked(),
        start_x: event.client_x(),
    }));
    mark_body(true);
}

fn move_drag(
    ui: Ui,
    drag: RwSignal<Option<Drag>>,
    side: HandleSide,
    event: &web_sys::PointerEvent,
) {
    let Some(start) = drag.get_untracked() else {
        return;
    };
    let raw = width_from_pointer(start.start_width, start.start_x, event.client_x(), side);
    side.width(ui).set(side.clamp(px_from(raw)));
}

fn end_drag(ui: Ui, drag: RwSignal<Option<Drag>>, side: HandleSide) {
    if drag.get_untracked().is_none() {
        return;
    }
    drag.set(None);
    mark_body(false);
    save_width(ui, side);
}

fn press_key(ui: Ui, side: HandleSide, event: &web_sys::KeyboardEvent) {
    let key = event.key();
    if key == "Home" {
        event.prevent_default();
        reset_width(ui, side);
        return;
    }
    let Some(delta) = key_delta(&key, event.shift_key(), side) else {
        return;
    };
    event.prevent_default();
    let next = shifted(side.width(ui).get_untracked(), delta);
    side.width(ui).set(side.clamp(next));
    save_width(ui, side);
}

fn reset_width(ui: Ui, side: HandleSide) {
    side.width(ui).set(side.default_px());
    save_width(ui, side);
}

/// Writes the width into the prefs. The prefs effect in `app.rs` keeps them on this device.
fn save_width(ui: Ui, side: HandleSide) {
    let px = side.width(ui).get_untracked();
    ui.prefs.update(|prefs| side.store(prefs, px));
}

/// Keeps the pointer's moves coming when it leaves the thin handle.
fn capture_pointer(event: &web_sys::PointerEvent) {
    let Some(handle) = event
        .current_target()
        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
    else {
        return;
    };
    let _ = handle.set_pointer_capture(event.pointer_id());
}

fn mark_body(dragging: bool) {
    let Some(body) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.body())
    else {
        return;
    };
    let classes = body.class_list();
    let _ = if dragging {
        classes.add_1(DRAG_CLASS)
    } else {
        classes.remove_1(DRAG_CLASS)
    };
}

fn viewport_width() -> u32 {
    web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
        .and_then(|root| u32::try_from(root.client_width()).ok())
        .unwrap_or(FALLBACK_VIEWPORT)
}

fn install_viewport(ui: Ui) {
    ui.viewport.set(viewport_width());
    let Some(window) = web_sys::window() else {
        return;
    };
    let on_resize = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
        ui.viewport.set(viewport_width());
    });
    let _ = window.add_event_listener_with_callback("resize", on_resize.as_ref().unchecked_ref());
    on_resize.forget();
}

fn shell_style() -> Option<web_sys::CssStyleDeclaration> {
    let shell = web_sys::window()?
        .document()?
        .get_element_by_id(SHELL_ID)?
        .dyn_into::<web_sys::HtmlElement>()
        .ok()?;
    Some(shell.style())
}

/// Sets `--sb-w` only while the sidebar is open on desktop. Without it the stylesheet's closed
/// width (0 px) still applies. `--panel-w` is set on desktop, and the stylesheet uses it only
/// while the panel is open and not expanded.
fn apply_shell_widths(ui: Ui) {
    let Some(style) = shell_style() else {
        return;
    };
    let desktop = ui.layout.get() == Layout::Desktop;
    let (sidebar, panel) = clamp_widths(
        ui.viewport.get(),
        ui.sidebar_width.get(),
        ui.panel_width.get(),
    );
    set_shell_var(&style, "--sb-w", ui.sidebar_resizable().then_some(sidebar));
    set_shell_var(&style, "--panel-w", desktop.then_some(panel));
}

fn set_shell_var(style: &web_sys::CssStyleDeclaration, name: &str, px: Option<u32>) {
    let _ = match px {
        Some(px) => style.set_property(name, &format!("{px}px")),
        None => style.remove_property(name).map(|_| ()),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragging_the_sidebar_edge_right_widens_the_sidebar() {
        assert_eq!(width_from_pointer(264, 100, 130, HandleSide::Sidebar), 294);
    }

    #[test]
    fn dragging_the_panel_edge_left_widens_the_panel() {
        assert_eq!(width_from_pointer(440, 800, 760, HandleSide::Panel), 480);
    }

    #[test]
    fn arrow_right_widens_the_sidebar_and_arrow_left_narrows_it() {
        assert_eq!(
            key_delta("ArrowRight", false, HandleSide::Sidebar),
            Some(16)
        );
        assert_eq!(
            key_delta("ArrowLeft", false, HandleSide::Sidebar),
            Some(-16)
        );
    }

    #[test]
    fn arrow_left_widens_the_panel_because_its_edge_is_on_the_left() {
        assert_eq!(key_delta("ArrowLeft", false, HandleSide::Panel), Some(16));
        assert_eq!(key_delta("ArrowRight", false, HandleSide::Panel), Some(-16));
    }

    #[test]
    fn shift_with_an_arrow_moves_sixty_four_pixels() {
        assert_eq!(key_delta("ArrowRight", true, HandleSide::Sidebar), Some(64));
    }

    #[test]
    fn keys_other_than_the_arrows_do_not_resize() {
        assert_eq!(key_delta("a", false, HandleSide::Sidebar), None);
    }

    #[test]
    fn a_key_step_never_takes_a_width_below_zero() {
        assert_eq!(shifted(10, -64), 0);
        assert_eq!(shifted(264, 16), 280);
    }

    #[test]
    fn saving_a_width_writes_only_its_own_side() {
        let mut prefs = Prefs::default();
        HandleSide::Panel.store(&mut prefs, 500);
        assert_eq!((prefs.sidebar_w, prefs.panel_w), (SIDEBAR_DEFAULT, 500));
    }

    #[test]
    fn a_reset_goes_back_to_the_default_width() {
        assert_eq!(HandleSide::Sidebar.default_px(), 264);
        assert_eq!(HandleSide::Panel.default_px(), 440);
    }
}
