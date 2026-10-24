//! The budget banner: a warn notice at 70 percent of the cap, a danger notice at 95 percent.

use leptos::prelude::*;

use crate::components::icon::Icon;

/// The card class and the sentence for a budget level.
pub fn budget_copy(percent: u8) -> (&'static str, String) {
    if percent >= 95 {
        (
            "card dangc",
            format!("Workers stopped. Spend reached {percent}% of the cap."),
        )
    } else {
        (
            "card warnc",
            format!(
                "Spend reached {percent}% of the cap. Lower the cap with /cap, or stop the run."
            ),
        )
    }
}

#[component]
pub fn BudgetBanner(percent: u8) -> impl IntoView {
    let (class, text) = budget_copy(percent);
    view! {
        <div class=class>
            <h4><Icon name="dollar" size=16 />{format!(" {percent}% of the cap used")}</h4>
            <div>{text}</div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seventy_percent_is_a_warning_and_ninety_five_is_danger() {
        assert_eq!(budget_copy(70).0, "card warnc");
        assert_eq!(budget_copy(95).0, "card dangc");
        assert!(budget_copy(95).1.starts_with("Workers stopped"));
    }
}
