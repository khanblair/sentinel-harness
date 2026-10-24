//! The contrast rules: which token goes on which, and the least ratio each pair needs.

use std::path::Path;

use super::color::contrast;
use super::{CHECK, Variant};
use crate::finding::Finding;

const TEXT_MIN: f64 = 4.5;
const GRAPHIC_MIN: f64 = 3.0;

struct Rule {
    /// The token drawn on top.
    fg: &'static str,
    /// The tokens it is drawn on.
    on: &'static [&'static str],
    min: f64,
}

const RULES: [Rule; 9] = [
    Rule {
        fg: "text",
        on: &["bg", "sidebar", "surface", "surface2"],
        min: TEXT_MIN,
    },
    Rule {
        fg: "muted",
        on: &["bg", "sidebar", "surface"],
        min: TEXT_MIN,
    },
    Rule {
        fg: "accent-fg",
        on: &["accent"],
        min: TEXT_MIN,
    },
    Rule {
        fg: "accent",
        on: &["bg"],
        min: GRAPHIC_MIN,
    },
    Rule {
        fg: "ok",
        on: &["bg", "surface"],
        min: GRAPHIC_MIN,
    },
    Rule {
        fg: "warn",
        on: &["bg", "surface"],
        min: GRAPHIC_MIN,
    },
    Rule {
        fg: "danger",
        on: &["bg", "surface"],
        min: GRAPHIC_MIN,
    },
    Rule {
        fg: "info",
        on: &["bg", "surface"],
        min: GRAPHIC_MIN,
    },
    Rule {
        fg: "lock",
        on: &["bg", "surface"],
        min: GRAPHIC_MIN,
    },
];

pub(super) fn check_variant(path: &Path, variant: &Variant) -> Vec<Finding> {
    let mut findings = Vec::new();
    for rule in &RULES {
        for background in rule.on {
            if let Some(message) = check_pair(variant, rule, background) {
                findings.push(Finding::new(CHECK, path, 0, message));
            }
        }
    }
    findings
}

/// The finding text for one pair, or `None` when the pair passes or a token is missing.
fn check_pair(variant: &Variant, rule: &Rule, background: &str) -> Option<String> {
    let fg = variant.colors.get(rule.fg)?;
    let bg = variant.colors.get(background)?;
    let ratio = contrast(fg, bg);
    (ratio < rule.min).then(|| {
        format!(
            "{}: {} on {background} is {ratio:.1}:1, needs {:.1}:1 ({fg} on {bg})",
            variant.name, rule.fg, rule.min
        )
    })
}
