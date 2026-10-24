//! `CssExport`: the current tokens as one `:root` block, with a Copy button (`prefs::tokens_css`).
//! The copy runs inside the click handler, so the browser allows the clipboard write.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen_futures::JsFuture;

use crate::components::icon::Icon;
use crate::components::token_editor::resolved_now;
use crate::prefs::tokens_css;
use crate::state::{ToastKind, Ui, use_ui};

/// Writes `text` to the clipboard and shows a toast with the result.
pub fn copy_text(ui: Ui, text: String) {
    spawn_local(async move {
        let Some(window) = web_sys::window() else {
            ui.show_toast(
                "Could not copy. Select the text instead.",
                ToastKind::Danger,
            );
            return;
        };
        let clipboard = window.navigator().clipboard();
        match JsFuture::from(clipboard.write_text(&text)).await {
            Ok(_) => ui.show_toast("Copied.", ToastKind::Ok),
            Err(_) => ui.show_toast(
                "Could not copy. Select the text instead.",
                ToastKind::Danger,
            ),
        }
    });
}

#[component]
pub fn CssExport() -> impl IntoView {
    let ui = use_ui();
    let css = move || {
        let prefs = ui.prefs.get();
        tokens_css(&prefs, resolved_now(&prefs))
    };
    view! {
        <details>
            <summary>"View tokens as CSS"</summary>
            <pre class="term" id="cssOut" style="min-height:0">{css}</pre>
            <button class="btn sm" style="margin-top:.5em" on:click=move |_| copy_text(ui, css())>
                <Icon name="copy" size=13 />
                " Copy CSS"
            </button>
        </details>
    }
}
