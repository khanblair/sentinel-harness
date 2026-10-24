//! Applies appearance prefs to the page root: the `data-theme` attribute and the CSS variables
//! that the stylesheet reads (`docs/ui-tokens.md`, Themes as CSS Variables).

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::prefs::{self, Prefs};
use crate::state::Ui;

const DARK_QUERY: &str = "(prefers-color-scheme: dark)";

/// True when the device is set to dark.
pub fn system_is_dark() -> bool {
    web_sys::window()
        .and_then(|window| window.match_media(DARK_QUERY).ok().flatten())
        .is_some_and(|list| list.matches())
}

/// Sets `data-theme` and every token variable on the root element.
pub fn apply(prefs: &Prefs) {
    let Some(root) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    else {
        return;
    };
    let resolved = prefs.resolve(system_is_dark());
    // A failed attribute write only leaves the old theme on screen until the next change.
    let _ = root.set_attribute("data-theme", resolved.name());
    let Ok(root) = root.dyn_into::<web_sys::HtmlElement>() else {
        return;
    };
    let style = root.style();
    for (name, value) in prefs::css_vars(prefs, resolved) {
        let _ = style.set_property(&name, &value);
    }
}

/// Re-applies the prefs when the device switches between light and dark. Only matters while the
/// theme is set to System.
pub fn follow_system(ui: Ui) {
    let Some(list) = web_sys::window().and_then(|window| window.match_media(DARK_QUERY).ok().flatten())
    else {
        return;
    };
    let on_change = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
        apply(&ui.prefs.get_untracked());
    });
    let _ = list.add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref());
    on_change.forget();
}
