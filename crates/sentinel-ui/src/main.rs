//! Browser entry point. Trunk compiles this to WASM.

// TODO(P6-07): remove this allow once the shell uses every module.
#![allow(dead_code)]

mod api;
mod app;
mod components;
mod icons;
mod layout;
mod prefs;
mod slash;
mod state;
mod theme;
mod views;
mod ws;

fn main() {
    console_error_panic_hook::set_once();
    api::capture_token_from_url();
    leptos::mount::mount_to_body(app::App);
}
