//! `RangeField`: a slider with its value in pixels, used for text size, stream width, and corner
//! radius (`docs/ui-tokens.md`, Settings-Driven Tokens).

use leptos::prelude::*;

/// The limits and step of one slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeSpec {
    pub label: &'static str,
    pub min: u32,
    pub max: u32,
    pub step: u32,
}

/// The value a slider reports, kept inside its limits. Text that is not a number keeps the
/// minimum, so a bad read never breaks the page.
pub fn range_value(text: &str, spec: RangeSpec) -> u32 {
    text.parse::<u32>()
        .unwrap_or(spec.min)
        .clamp(spec.min, spec.max)
}

#[component]
pub fn RangeField(spec: RangeSpec, value: Signal<u32>, on_change: Callback<u32>) -> impl IntoView {
    view! {
        <span class="row">
            <input
                type="range"
                min=spec.min
                max=spec.max
                step=spec.step
                aria-label=spec.label
                prop:value=move || value.get().to_string()
                on:input=move |ev| on_change.run(range_value(&event_target_value(&ev), spec))
            />
            <span class="mono">{move || format!("{}px", value.get())}</span>
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: RangeSpec = RangeSpec {
        label: "Text size",
        min: 12,
        max: 18,
        step: 1,
    };

    #[test]
    fn a_slider_value_inside_the_range_is_kept() {
        assert_eq!(range_value("15", SPEC), 15);
    }

    #[test]
    fn a_slider_value_outside_the_range_is_pulled_back() {
        assert_eq!(range_value("40", SPEC), 18);
        assert_eq!(range_value("2", SPEC), 12);
    }

    #[test]
    fn text_that_is_not_a_number_keeps_the_minimum() {
        assert_eq!(range_value("", SPEC), 12);
        assert_eq!(range_value("-3", SPEC), 12);
        assert_eq!(range_value("14.5", SPEC), 12);
    }
}
