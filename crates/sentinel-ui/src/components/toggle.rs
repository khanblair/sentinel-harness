//! `Toggle` and `Segmented`: the two small choice controls of Settings. Both use the prototype's
//! classes (`tog`, `seg`), so the stylesheet draws them with no change.

use leptos::prelude::*;

use crate::components::icon::Icon;

/// One segment of a `Segmented` control. An empty `icon` shows the label alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegOption {
    pub value: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
}

/// An on or off switch. It reports the value it should move to.
#[component]
pub fn Toggle(checked: Signal<bool>, on_change: Callback<bool>) -> impl IntoView {
    view! {
        <button
            class="tog"
            role="switch"
            aria-checked=move || checked.get().to_string()
            on:click=move |_| on_change.run(!checked.get_untracked())
        ></button>
    }
}

/// A row of connected choices. One is pressed, and a press reports the segment's value.
#[component]
pub fn Segmented(
    options: &'static [SegOption],
    selected: Signal<String>,
    on_pick: Callback<String>,
) -> impl IntoView {
    view! {
        <span class="seg">
            {options
                .iter()
                .map(|option| segment(*option, selected, on_pick))
                .collect_view()}
        </span>
    }
}

fn segment(
    option: SegOption,
    selected: Signal<String>,
    on_pick: Callback<String>,
) -> impl IntoView {
    let pressed = move || (selected.get() == option.value).to_string();
    view! {
        <button aria-pressed=pressed on:click=move |_| on_pick.run(option.value.to_owned())>
            {(!option.icon.is_empty()).then(|| view! { <Icon name=option.icon size=15 /> })}
            {option.label}
        </button>
    }
}
