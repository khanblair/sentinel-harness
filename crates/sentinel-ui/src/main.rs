//! Browser entry point. Trunk compiles this to WASM.

// TODO(P6-07): remove this allow once the shell uses every module.
#![allow(dead_code)]

mod app;
mod components;
mod icons;
mod prefs;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
