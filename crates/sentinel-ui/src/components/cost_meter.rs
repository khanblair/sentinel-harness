//! `CostMeter`: the spend bar with marks at 70 and 95 percent of the cap.

use leptos::prelude::*;
use sentinel_core::Cost;

use crate::views::cost::{bar_tone, percent_marks};

/// Width and color of the fill, for a whole percent between 0 and 100.
pub fn fill_style(percent: u8) -> String {
    let tone = bar_tone(percent);
    if tone.is_empty() {
        format!("width:{percent}%")
    } else {
        format!("width:{percent}%;background:{tone}")
    }
}

/// Spend as a whole percent of the cap, clamped to 0 to 100. A zero cap reads as full.
pub fn spend_percent(total: Cost, cap: Cost) -> u8 {
    total.percent_of(cap).min(100)
}

#[component]
pub fn CostMeter(
    #[prop(into)] total: Signal<Cost>,
    #[prop(into)] cap: Signal<Cost>,
) -> impl IntoView {
    let percent = move || spend_percent(total.get(), cap.get());
    let [warn_mark, danger_mark] = percent_marks();
    view! {
        <div class="meter" style="background:var(--surface2)">
            <i style=move || fill_style(percent())></i>
            <u style=format!("left:{warn_mark}%")></u>
            <u style=format!("left:{danger_mark}%")></u>
        </div>
        <div class="row sp sub">
            <span>{format!("{warn_mark}%: no new tasks start")}</span>
            <span>{format!("{danger_mark}%: workers stop")}</span>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_colors_follow_the_thresholds() {
        assert_eq!(fill_style(40), "width:40%");
        assert_eq!(fill_style(80), "width:80%;background:var(--warn)");
        assert_eq!(fill_style(100), "width:100%;background:var(--danger)");
    }

    #[test]
    fn spend_percent_is_clamped_and_handles_zero_cap() {
        let cap = Cost::from_micro_usd(2_000_000, false);
        let over = Cost::from_micro_usd(3_000_000, false);
        assert_eq!(spend_percent(over, cap), 100);
        assert_eq!(spend_percent(Cost::ZERO, Cost::ZERO), 100);
    }
}
