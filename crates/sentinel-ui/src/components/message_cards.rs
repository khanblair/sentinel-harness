//! Plain stream lines: the user's instruction, a Sentinel line, and a muted note with an icon.

use leptos::prelude::*;
use sentinel_core::ToolId;

use crate::components::icon::Icon;

#[component]
pub fn UserMessage(to: Option<ToolId>, #[prop(into)] text: String) -> impl IntoView {
    let target = to.map(|agent| view! { <small>{format!("to {agent}")}</small> });
    view! { <div class="msg-user">{target}{text}</div> }
}

#[component]
pub fn SystemMessage(#[prop(into)] text: String) -> impl IntoView {
    view! { <div class="msg-sys">{text}</div> }
}

#[component]
pub fn NoteLine(#[prop(into)] text: String, icon: &'static str) -> impl IntoView {
    view! {
        <div class="note">
            <Icon name=icon size=14 />
            <span>{text}</span>
        </div>
    }
}
