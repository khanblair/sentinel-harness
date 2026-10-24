//! `FontSelect`: a dropdown of the font choices in `prefs::UI_FONTS` or `prefs::MONO_FONTS`. The
//! stack for each choice is applied by the prefs code, not here.

use leptos::prelude::*;

/// One font choice: its key, its label, and its CSS stack.
pub type FontChoice = (&'static str, &'static str, &'static str);

#[component]
pub fn FontSelect(
    label: &'static str,
    options: &'static [FontChoice],
    selected: Signal<String>,
    on_pick: Callback<String>,
) -> impl IntoView {
    view! {
        <select
            aria-label=label
            prop:value=move || selected.get()
            on:change=move |ev| on_pick.run(event_target_value(&ev))
        >
            {options
                .iter()
                .map(|(key, name, _)| view! { <option value=*key>{*name}</option> })
                .collect_view()}
        </select>
    }
}
