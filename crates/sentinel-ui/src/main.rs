//! Browser entry point. Trunk compiles this to WASM.

// TODO(P6-07): remove this allow once the shell uses every module.
#![allow(dead_code)]

mod api;
mod app;
mod components;
mod guide;
mod icons;
mod layout;
mod palette;
mod prefs;
mod slash;
mod state;
mod theme;
mod tour;
mod tour_state;
mod views;
mod ws;

fn main() {
    console_error_panic_hook::set_once();
    api::capture_token_from_url();
    if on_pair_page() {
        leptos::mount::mount_to_body(views::pair_page::PairPage);
    } else {
        leptos::mount::mount_to_body(app::App);
    }
}

/// A phone that scans the pairing QR code lands on `/pair`.
fn on_pair_page() -> bool {
    web_sys::window()
        .and_then(|window| window.location().pathname().ok())
        .is_some_and(|path| path.trim_end_matches('/') == "/pair")
}
