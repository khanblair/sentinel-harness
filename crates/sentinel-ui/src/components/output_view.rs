//! `OutputView`: monospace agent output. It follows the newest line while the reader is at the
//! bottom, and stops following when the reader scrolls up.

use leptos::prelude::*;

use crate::views::terminal::line_class;

/// The id of the scrolling `.term` box. One output view is on screen at a time.
const TERMINAL_ID: &str = "termBox";
/// Within this many pixels of the end, the view counts as at the bottom.
const BOTTOM_SLACK_PX: i32 = 24;

/// True when the box is scrolled to (or within a few pixels of) its end.
pub fn near_end(element: &web_sys::Element) -> bool {
    element.scroll_height() - element.scroll_top() - element.client_height() < BOTTOM_SLACK_PX
}

fn scroll_to_end(id: &str) {
    let element = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(id));
    if let Some(element) = element {
        element.set_scroll_top(element.scroll_height());
    }
}

#[component]
pub fn OutputView(#[prop(into)] lines: Signal<Vec<String>>) -> impl IntoView {
    let pinned = RwSignal::new(true);
    Effect::new(move |_| {
        lines.track();
        if pinned.get_untracked() {
            scroll_to_end(TERMINAL_ID);
        }
    });
    view! {
        <div class="term" id=TERMINAL_ID
            on:scroll=move |event| {
                let at_end = event_target::<web_sys::Element>(&event);
                pinned.set(near_end(&at_end));
            }>
            {move || {
                let current = lines.get();
                if current.is_empty() {
                    view! { <span class="m">"No output yet. Start a run to see the raw agent output here."</span> }
                        .into_any()
                } else {
                    current
                        .into_iter()
                        .map(|line| {
                            let class = line_class(&line).class_name();
                            view! { <span class=class>{line}</span>"\n" }
                        })
                        .collect_view()
                        .into_any()
                }
            }}
        </div>
    }
}
