//! Browser entry point. Trunk compiles this to WASM.

mod app;
mod components;
mod icons;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
