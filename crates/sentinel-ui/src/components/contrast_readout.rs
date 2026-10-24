//! `ContrastReadout`: the live contrast check in Settings, Appearance. Each pair shows its ratio and
//! whether it meets its limit (`docs/ui-tokens.md`, Contrast Rules). The words and the check or
//! cross icon carry the result, so color is not the only signal.

use std::collections::BTreeMap;

use leptos::prelude::*;

use crate::components::icon::Icon;
use crate::prefs::{self, Prefs, Resolved};

/// Body text and button text need 4.5:1. Accent on the background only needs 3:1.
const TEXT_LIMIT: f64 = 4.5;
const LARGE_LIMIT: f64 = 3.0;

/// One checked pair in the readout.
#[derive(Debug, Clone, PartialEq)]
pub struct ContrastRow {
    pub label: &'static str,
    pub ratio: f64,
    pub limit: f64,
    pub pass: bool,
}

/// A token pair to check, by token name.
struct Pair {
    label: &'static str,
    fg: &'static str,
    bg: &'static str,
    limit: f64,
}

const PAIRS: [Pair; 4] = [
    Pair {
        label: "Text",
        fg: "text",
        bg: "bg",
        limit: TEXT_LIMIT,
    },
    Pair {
        label: "Muted",
        fg: "muted",
        bg: "bg",
        limit: TEXT_LIMIT,
    },
    Pair {
        label: "Accent on bg",
        fg: "accent",
        bg: "bg",
        limit: LARGE_LIMIT,
    },
    Pair {
        label: "Button text",
        fg: "accent-fg",
        bg: "accent",
        limit: TEXT_LIMIT,
    },
];

/// Every pair for one theme, checked against the tokens that theme shows.
pub fn contrast_rows(prefs: &Prefs, resolved: Resolved) -> Vec<ContrastRow> {
    let tokens = prefs::tokens(prefs, resolved);
    PAIRS.iter().map(|pair| check_pair(&tokens, pair)).collect()
}

fn check_pair(tokens: &BTreeMap<String, String>, pair: &Pair) -> ContrastRow {
    let color = |key: &str| tokens.get(key).map_or("", String::as_str);
    // A value that is not `#RRGGBB` cannot be measured, so it counts as a fail.
    let ratio = prefs::contrast(color(pair.fg), color(pair.bg)).unwrap_or(0.0);
    ContrastRow {
        label: pair.label,
        ratio,
        limit: pair.limit,
        pass: ratio >= pair.limit,
    }
}

/// The readout text for one row, such as `Text 14.2:1`.
pub fn row_text(row: &ContrastRow) -> String {
    format!("{} {:.1}:1", row.label, row.ratio)
}

#[component]
pub fn ContrastReadout(#[prop(into)] rows: Signal<Vec<ContrastRow>>) -> impl IntoView {
    view! {
        <div class="row wrap" style="gap:.4em" id="contrastRow">
            {move || rows.get().into_iter().map(|row| chip(&row)).collect_view()}
        </div>
    }
}

fn chip(row: &ContrastRow) -> AnyView {
    let (color, icon) = if row.pass {
        ("ok", "check")
    } else {
        ("danger", "x")
    };
    let text = row_text(row);
    view! {
        <span class="chip" style=format!("color:var(--{color})")>
            <Icon name=icon size=11 />
            " "
            {text}
        </span>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_named(rows: &[ContrastRow], label: &str) -> ContrastRow {
        rows.iter()
            .find(|row| row.label == label)
            .cloned()
            .unwrap_or_else(|| panic!("no row named {label}"))
    }

    #[test]
    fn default_colors_pass_every_pair_in_both_themes() {
        for resolved in [Resolved::Dark, Resolved::Light] {
            for row in contrast_rows(&Prefs::default(), resolved) {
                assert!(row.pass, "{resolved:?} {}", row_text(&row));
            }
        }
    }

    #[test]
    fn accent_on_background_uses_the_three_to_one_limit() {
        let rows = contrast_rows(&Prefs::default(), Resolved::Light);
        let accent = row_named(&rows, "Accent on bg");
        assert!((accent.limit - 3.0).abs() < f64::EPSILON);
        assert!((row_named(&rows, "Text").limit - 4.5).abs() < f64::EPSILON);
    }

    #[test]
    fn a_low_contrast_text_color_fails_its_row() {
        let mut prefs = Prefs::default();
        prefs.overrides.dark.insert("bg".into(), "#777777".into());
        prefs.overrides.dark.insert("text".into(), "#808080".into());
        let text = row_named(&contrast_rows(&prefs, Resolved::Dark), "Text");
        assert!(!text.pass, "{}", row_text(&text));
        assert!(text.ratio < 4.5);
    }

    #[test]
    fn row_text_shows_one_decimal_and_the_ratio_unit() {
        let row = ContrastRow {
            label: "Text",
            ratio: 14.26,
            limit: 4.5,
            pass: true,
        };
        assert_eq!(row_text(&row), "Text 14.3:1");
    }
}
