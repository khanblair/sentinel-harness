//! `StrengthBar`: how strong a note is, as a thin bar with the prototype's `sbar` classes.

use leptos::prelude::*;

use crate::brain::strength_class;

/// The fill width for a strength from 0 to 1, as a CSS style.
pub fn strength_style(strength: f32) -> String {
    format!("width:{:.0}%", strength.clamp(0.0, 1.0) * 100.0)
}

/// The tooltip text, such as `Strength 80%`.
pub fn strength_title(strength: f32) -> String {
    format!("Strength {:.0}%", strength.clamp(0.0, 1.0) * 100.0)
}

#[component]
pub fn StrengthBar(strength: f32, #[prop(default = false)] wide: bool) -> impl IntoView {
    let class = if wide { "sbar wide" } else { "sbar" };
    view! {
        <span class=class title=strength_title(strength)>
            <i class=strength_class(strength) style=strength_style(strength)></i>
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fill_follows_the_strength_and_stops_at_full() {
        assert_eq!(strength_style(0.5), "width:50%");
        assert_eq!(strength_style(1.7), "width:100%");
        assert_eq!(strength_title(0.8), "Strength 80%");
    }
}
