//! The page. The workspace shell (sidebar, top bar, stream, panels) lands in P6-07.

use leptos::prelude::*;

use crate::components::icon::Icon;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <main class="empty-boot">
            <Icon name="lock" size=22 />
            <span>"Sentinel"</span>
        </main>
    }
}
