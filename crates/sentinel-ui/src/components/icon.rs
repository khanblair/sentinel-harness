//! `Icon`: one inline SVG from the outline set, sized in `em` so it scales with the text size.

use leptos::prelude::*;

use crate::icons;

#[component]
pub fn Icon(
    /// A name from `icons::ICONS`.
    name: &'static str,
    /// Pixel size at the default 14 px text size.
    #[prop(default = 18)]
    size: u32,
) -> impl IntoView {
    let em = format!("{:.2}", f64::from(size) / 14.0);
    let filled = icons::is_filled(name);
    view! {
        <svg
            class="ic"
            style=format!("width:{em}em;height:{em}em")
            viewBox="0 0 24 24"
            fill=if filled { "currentColor" } else { "none" }
            stroke=if filled { "none" } else { "currentColor" }
            stroke-width="1.7"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
            inner_html=icons::markup(name)
        ></svg>
    }
}
