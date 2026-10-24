//! `AccentPicker`: the six accent swatches, a custom color, and Reset to default
//! (`docs/ui-tokens.md`, Accent Options). The text on the accent is picked by luminance in
//! `prefs::foreground_for`, so this file only stores the accent.

use leptos::prelude::*;

use crate::components::token_editor::{normalize_hex, resolved_now};
use crate::prefs::{self, Prefs, Resolved, SWATCHES};
use crate::state::{Ui, use_ui};

/// Sets the accent to a `#RRGGBB` value. Any other value leaves the prefs as they are.
pub fn pick_accent(prefs: &Prefs, value: &str) -> Prefs {
    let Some(hex) = normalize_hex(value) else {
        return prefs.clone();
    };
    Prefs {
        accent: Some(hex),
        ..prefs.clone()
    }
}

/// Puts the accent back to the theme default. A per-theme accent edit made in the token editor
/// is cleared too, so the reset is complete.
pub fn reset_accent(prefs: &Prefs) -> Prefs {
    let mut next = prefs.clone();
    next.accent = None;
    next.overrides.dark.remove("accent");
    next.overrides.dark.remove("accent-fg");
    next.overrides.light.remove("accent");
    next.overrides.light.remove("accent-fg");
    next
}

/// The accent the page shows now, whether it came from a swatch, the custom color, or a default.
pub fn shown_accent(prefs: &Prefs, resolved: Resolved) -> String {
    prefs::tokens(prefs, resolved)
        .get("accent")
        .cloned()
        .unwrap_or_default()
}

#[component]
pub fn AccentPicker() -> impl IntoView {
    let ui = use_ui();
    let custom = move || {
        let prefs = ui.prefs.get();
        shown_accent(&prefs, resolved_now(&prefs))
    };
    view! {
        <div class="sw">
            {SWATCHES.into_iter().map(|hex| swatch(ui, hex)).collect_view()}
            <label class="swl">
                <span>"Custom"</span>
                <input
                    type="color"
                    aria-label="Custom accent"
                    prop:value=custom
                    on:input=move |ev| ui.prefs.update(|p| *p = pick_accent(p, &event_target_value(&ev)))
                />
            </label>
            <button class="btn sm" on:click=move |_| ui.prefs.update(|p| *p = reset_accent(p))>
                "Reset to default"
            </button>
        </div>
    }
}

fn swatch(ui: Ui, hex: &'static str) -> impl IntoView {
    let pressed = move || (ui.prefs.get().accent.as_deref() == Some(hex)).to_string();
    view! {
        <button
            class="swc"
            style=format!("background:{hex}")
            aria-label=format!("Accent {hex}")
            aria-pressed=pressed
            on:click=move |_| ui.prefs.update(|p| *p = pick_accent(p, hex))
        ></button>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_accent_stores_a_valid_swatch() {
        let next = pick_accent(&Prefs::default(), "#5b8def");
        assert_eq!(next.accent.as_deref(), Some("#5B8DEF"));
    }

    #[test]
    fn pick_accent_ignores_a_value_that_is_not_a_hex_color() {
        let prefs = Prefs::default();
        assert_eq!(pick_accent(&prefs, "blue"), prefs);
    }

    #[test]
    fn reset_accent_returns_to_the_default_and_clears_accent_overrides() {
        let mut prefs = pick_accent(&Prefs::default(), "#5B8DEF");
        prefs
            .overrides
            .dark
            .insert("accent".into(), "#111111".into());
        prefs
            .overrides
            .light
            .insert("accent-fg".into(), "#222222".into());
        prefs.overrides.dark.insert("bg".into(), "#000000".into());
        let reset = reset_accent(&prefs);
        assert_eq!(reset.accent, None);
        assert!(!reset.overrides.dark.contains_key("accent"));
        assert!(!reset.overrides.light.contains_key("accent-fg"));
        assert_eq!(
            reset.overrides.dark.get("bg").map(String::as_str),
            Some("#000000")
        );
        assert_eq!(prefs::tokens(&reset, Resolved::Light)["accent"], "#B35F09");
    }

    #[test]
    fn shown_accent_follows_the_chosen_swatch() {
        let prefs = pick_accent(&Prefs::default(), "#3DAA7D");
        assert_eq!(shown_accent(&prefs, Resolved::Dark), "#3DAA7D");
    }
}
