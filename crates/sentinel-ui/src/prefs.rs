//! Appearance preferences: theme, accent, tint, fonts, size, density, radius, and color
//! overrides. Stored on each device, applied as CSS variables on the page root. This is the
//! prototype's `currentTokens`, `applyPrefs`, and `tokensCss`, with the same values as
//! `docs/ui-tokens.md`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Dark theme tokens, from `docs/ui-tokens.md`.
const DARK: &[(&str, &str)] = &[
    ("bg", "#191919"),
    ("sidebar", "#141414"),
    ("surface", "#212121"),
    ("surface2", "#2B2B2B"),
    ("border", "#303030"),
    ("text", "#ECECEA"),
    ("muted", "#9B9A96"),
    ("accent", "#E8963A"),
    ("accent-fg", "#1A1006"),
    ("ok", "#4CC38A"),
    ("warn", "#E5B93C"),
    ("danger", "#EF6A6E"),
    ("info", "#6BA8FF"),
    ("lock", "#B994FF"),
];

const LIGHT: &[(&str, &str)] = &[
    ("bg", "#FAFAF9"),
    ("sidebar", "#F2F1EF"),
    ("surface", "#FFFFFF"),
    ("surface2", "#EDECE9"),
    ("border", "#E0DED9"),
    ("text", "#1D1D1B"),
    ("muted", "#6B6A66"),
    ("accent", "#B35F09"),
    ("accent-fg", "#FFFFFF"),
    ("ok", "#1E7F55"),
    ("warn", "#8A6A00"),
    ("danger", "#B3262B"),
    ("info", "#1F5FBF"),
    ("lock", "#6B3FC4"),
];

/// `(key, bg, sidebar, surface, surface2, border)` for the warm and cool tints.
type TintRow = (&'static str, [&'static str; 5]);
const DARK_TINTS: &[TintRow] = &[
    (
        "warm",
        ["#1B1917", "#161412", "#242120", "#2E2A28", "#34302D"],
    ),
    (
        "cool",
        ["#16191D", "#12151A", "#1E2228", "#272C34", "#2C323B"],
    ),
];
const LIGHT_TINTS: &[TintRow] = &[
    (
        "warm",
        ["#FBF9F6", "#F3EFE9", "#FFFFFF", "#ECE6DD", "#E2DAD0"],
    ),
    (
        "cool",
        ["#F7F9FB", "#EEF2F6", "#FFFFFF", "#E5EBF1", "#D9E1E9"],
    ),
];
const TINT_KEYS: [&str; 5] = ["bg", "sidebar", "surface", "surface2", "border"];

/// `(key, label, CSS font stack)`.
pub const UI_FONTS: &[(&str, &str, &str)] = &[
    (
        "system",
        "System",
        "system-ui,-apple-system,\"Segoe UI\",Roboto,sans-serif",
    ),
    ("inter", "Inter", "Inter,system-ui,sans-serif"),
    (
        "plex",
        "IBM Plex Sans",
        "\"IBM Plex Sans\",system-ui,sans-serif",
    ),
    ("dm", "DM Sans", "\"DM Sans\",system-ui,sans-serif"),
    (
        "serif",
        "Source Serif 4",
        "\"Source Serif 4\",Georgia,serif",
    ),
    (
        "atk",
        "Atkinson Hyperlegible",
        "\"Atkinson Hyperlegible\",system-ui,sans-serif",
    ),
];

pub const MONO_FONTS: &[(&str, &str, &str)] = &[
    (
        "system",
        "System mono",
        "ui-monospace,SFMono-Regular,Menlo,Consolas,monospace",
    ),
    (
        "jb",
        "JetBrains Mono",
        "\"JetBrains Mono\",ui-monospace,monospace",
    ),
    (
        "plexm",
        "IBM Plex Mono",
        "\"IBM Plex Mono\",ui-monospace,monospace",
    ),
    ("fira", "Fira Code", "\"Fira Code\",ui-monospace,monospace"),
    (
        "scp",
        "Source Code Pro",
        "\"Source Code Pro\",ui-monospace,monospace",
    ),
];

/// The six accent swatches in Settings.
pub const SWATCHES: [&str; 6] = [
    "#E8963A", "#5B8DEF", "#3DAA7D", "#9B7BEA", "#E5668B", "#A8A8A4",
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// The theme after "system" is resolved against the device setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolved {
    Light,
    Dark,
}

impl Resolved {
    pub fn name(self) -> &'static str {
        match self {
            Resolved::Light => "light",
            Resolved::Dark => "dark",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tint {
    #[default]
    Neutral,
    Warm,
    Cool,
}

impl Tint {
    fn key(self) -> Option<&'static str> {
        match self {
            Tint::Neutral => None,
            Tint::Warm => Some("warm"),
            Tint::Cool => Some("cool"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Overrides {
    pub dark: BTreeMap<String, String>,
    pub light: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub theme: Theme,
    pub accent: Option<String>,
    pub tint: Tint,
    pub font_ui: String,
    pub font_mono: String,
    /// Base text size in pixels, 12 to 18.
    pub fs: u32,
    /// Spacing multiplier: 0.8, 1.0, or 1.25.
    pub density: f64,
    /// Corner radius in pixels, 2 to 16.
    pub radius: u32,
    /// Stream width in pixels, 560 to 980.
    pub mw: u32,
    pub overrides: Overrides,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            accent: None,
            tint: Tint::Neutral,
            font_ui: "inter".into(),
            font_mono: "jb".into(),
            fs: 14,
            density: 1.0,
            radius: 8,
            mw: 760,
            overrides: Overrides::default(),
        }
    }
}

impl Prefs {
    /// Pull every value into its allowed range, so stored or edited prefs cannot break the page.
    #[must_use]
    pub fn clamped(mut self) -> Self {
        self.fs = self.fs.clamp(12, 18);
        self.radius = self.radius.clamp(2, 16);
        self.mw = self.mw.clamp(560, 980);
        self.density = self.density.clamp(0.8, 1.25);
        if !UI_FONTS.iter().any(|(k, _, _)| *k == self.font_ui) {
            self.font_ui = "inter".into();
        }
        if !MONO_FONTS.iter().any(|(k, _, _)| *k == self.font_mono) {
            self.font_mono = "jb".into();
        }
        self
    }

    pub fn resolve(&self, system_is_dark: bool) -> Resolved {
        let dark = match self.theme {
            Theme::Light => false,
            Theme::Dark => true,
            Theme::System => system_is_dark,
        };
        if dark {
            Resolved::Dark
        } else {
            Resolved::Light
        }
    }
}

/// Relative luminance of `#RRGGBB`, or `None` for anything else.
pub fn luminance(hex: &str) -> Option<f64> {
    let digits = hex.strip_prefix('#')?;
    if digits.len() != 6 {
        return None;
    }
    let channel = |i: usize| -> Option<f64> {
        let v = f64::from(u8::from_str_radix(&digits[i..i + 2], 16).ok()?) / 255.0;
        Some(if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        })
    };
    Some(0.2126 * channel(0)? + 0.7152 * channel(2)? + 0.0722 * channel(4)?)
}

/// Whichever of dark text or white gives the better contrast on this accent. (The prototype used a
/// fixed luminance cut that chose white text on the default amber; this picks by contrast.)
pub fn foreground_for(hex: &str) -> &'static str {
    const DARK_TEXT: &str = "#17120A";
    match (contrast(hex, DARK_TEXT), contrast(hex, "#FFFFFF")) {
        (Some(dark), Some(white)) if dark >= white => DARK_TEXT,
        _ => "#FFFFFF",
    }
}

/// WCAG contrast ratio of two colors.
pub fn contrast(a: &str, b: &str) -> Option<f64> {
    let (x, y) = (luminance(a)?, luminance(b)?);
    Some((x.max(y) + 0.05) / (x.min(y) + 0.05))
}

/// Every color token for the resolved theme: base, then tint, then accent, then overrides.
pub fn tokens(prefs: &Prefs, resolved: Resolved) -> BTreeMap<String, String> {
    let (base, tints, overrides) = match resolved {
        Resolved::Dark => (DARK, DARK_TINTS, &prefs.overrides.dark),
        Resolved::Light => (LIGHT, LIGHT_TINTS, &prefs.overrides.light),
    };
    let mut out: BTreeMap<String, String> = base
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    if let Some((_, row)) = prefs
        .tint
        .key()
        .and_then(|k| tints.iter().find(|(n, _)| *n == k))
    {
        for (key, value) in TINT_KEYS.iter().zip(row) {
            out.insert((*key).to_owned(), (*value).to_owned());
        }
    }
    if let Some(accent) = &prefs.accent {
        out.insert("accent-fg".into(), foreground_for(accent).into());
        out.insert("accent".into(), accent.clone());
    }
    for (key, value) in overrides {
        out.insert(key.clone(), value.clone());
    }
    if let Some(accent) = overrides
        .get("accent")
        .filter(|_| !overrides.contains_key("accent-fg"))
    {
        out.insert("accent-fg".into(), foreground_for(accent).into());
    }
    out
}

fn font_stack(table: &[(&str, &str, &str)], key: &str) -> String {
    table
        .iter()
        .find(|(k, _, _)| *k == key)
        .map_or_else(String::new, |(_, _, s)| (*s).to_owned())
}

/// Every CSS variable this page sets on its root, as `(name, value)` with the leading `--`.
pub fn css_vars(prefs: &Prefs, resolved: Resolved) -> Vec<(String, String)> {
    let mut vars: Vec<(String, String)> = tokens(prefs, resolved)
        .into_iter()
        .map(|(k, v)| (format!("--{k}"), v))
        .collect();
    vars.push(("--font-ui".into(), font_stack(UI_FONTS, &prefs.font_ui)));
    vars.push((
        "--font-mono".into(),
        font_stack(MONO_FONTS, &prefs.font_mono),
    ));
    vars.push(("--fs".into(), format!("{}px", prefs.fs)));
    vars.push(("--d".into(), format!("{}", prefs.density)));
    vars.push(("--r".into(), format!("{}px", prefs.radius)));
    vars.push(("--mw".into(), format!("{}px", prefs.mw)));
    vars
}

/// The "Copy as CSS" text in Settings, Appearance.
pub fn tokens_css(prefs: &Prefs, resolved: Resolved) -> String {
    let lines: Vec<String> = css_vars(prefs, resolved)
        .iter()
        .map(|(k, v)| format!("  {k}: {v};"))
        .collect();
    format!(
        ":root[data-theme=\"{}\"] {{\n{}\n}}",
        resolved.name(),
        lines.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_prefs_use_the_documented_values() {
        let p = Prefs::default();
        assert_eq!((p.fs, p.radius, p.mw), (14, 8, 760));
        assert!((p.density - 1.0).abs() < f64::EPSILON);
        assert_eq!(p.theme, Theme::System);
    }

    #[test]
    fn dark_tokens_match_the_token_doc() {
        let t = tokens(&Prefs::default(), Resolved::Dark);
        assert_eq!(t["bg"], "#191919");
        assert_eq!(t["accent"], "#E8963A");
        assert_eq!(t["lock"], "#B994FF");
    }

    #[test]
    fn light_tokens_match_the_token_doc() {
        let t = tokens(&Prefs::default(), Resolved::Light);
        assert_eq!(t["bg"], "#FAFAF9");
        assert_eq!(t["accent"], "#B35F09");
    }

    #[test]
    fn warm_tint_replaces_the_five_surface_tokens_only() {
        let prefs = Prefs {
            tint: Tint::Warm,
            ..Prefs::default()
        };
        let t = tokens(&prefs, Resolved::Dark);
        assert_eq!(t["bg"], "#1B1917");
        assert_eq!(t["border"], "#34302D");
        assert_eq!(t["text"], "#ECECEA");
    }

    #[test]
    fn accent_choice_sets_foreground_by_luminance() {
        let light_accent = Prefs {
            accent: Some("#E8963A".into()),
            ..Prefs::default()
        };
        assert_eq!(
            tokens(&light_accent, Resolved::Dark)["accent-fg"],
            "#17120A"
        );
        let dark_accent = Prefs {
            accent: Some("#3D2A8A".into()),
            ..Prefs::default()
        };
        assert_eq!(tokens(&dark_accent, Resolved::Dark)["accent-fg"], "#FFFFFF");
    }

    #[test]
    fn overrides_win_and_an_accent_override_picks_its_own_foreground() {
        let mut prefs = Prefs::default();
        prefs.overrides.dark.insert("bg".into(), "#000000".into());
        prefs
            .overrides
            .dark
            .insert("accent".into(), "#FFFFFF".into());
        let t = tokens(&prefs, Resolved::Dark);
        assert_eq!(t["bg"], "#000000");
        assert_eq!(t["accent-fg"], "#17120A");
        assert_eq!(tokens(&prefs, Resolved::Light)["bg"], "#FAFAF9");
    }

    #[test]
    fn contrast_of_black_on_white_is_21() {
        let ratio = contrast("#000000", "#FFFFFF").unwrap();
        assert!((ratio - 21.0).abs() < 0.01, "{ratio}");
        assert!(contrast("red", "#FFFFFF").is_none());
    }

    #[test]
    fn body_text_meets_the_contrast_rule_in_both_themes() {
        for resolved in [Resolved::Dark, Resolved::Light] {
            let t = tokens(&Prefs::default(), resolved);
            assert!(
                contrast(&t["text"], &t["bg"]).unwrap() >= 4.5,
                "{resolved:?}"
            );
            assert!(
                contrast(&t["muted"], &t["bg"]).unwrap() >= 4.5,
                "{resolved:?}"
            );
        }
    }

    #[test]
    fn clamped_pulls_values_into_range() {
        let prefs = Prefs {
            fs: 40,
            radius: 0,
            mw: 100,
            density: 3.0,
            font_ui: "nope".into(),
            ..Prefs::default()
        };
        let c = prefs.clamped();
        assert_eq!((c.fs, c.radius, c.mw), (18, 2, 560));
        assert!((c.density - 1.25).abs() < f64::EPSILON);
        assert_eq!(c.font_ui, "inter");
    }

    #[test]
    fn system_theme_follows_the_device() {
        let prefs = Prefs::default();
        assert_eq!(prefs.resolve(true), Resolved::Dark);
        assert_eq!(prefs.resolve(false), Resolved::Light);
        let fixed = Prefs {
            theme: Theme::Light,
            ..Prefs::default()
        };
        assert_eq!(fixed.resolve(true), Resolved::Light);
    }

    #[test]
    fn css_export_lists_tokens_and_settings() {
        let css = tokens_css(&Prefs::default(), Resolved::Dark);
        assert!(css.starts_with(":root[data-theme=\"dark\"] {"), "{css}");
        for needle in [
            "--bg: #191919;",
            "--fs: 14px;",
            "--r: 8px;",
            "--mw: 760px;",
            "--d: 1;",
        ] {
            assert!(css.contains(needle), "{needle} missing in {css}");
        }
    }

    #[test]
    fn prefs_round_trip_and_tolerate_missing_fields() {
        let json = serde_json::to_string(&Prefs::default()).unwrap();
        assert_eq!(
            serde_json::from_str::<Prefs>(&json).unwrap(),
            Prefs::default()
        );
        let partial: Prefs = serde_json::from_str("{\"fs\":16}").unwrap();
        assert_eq!(partial.fs, 16);
        assert_eq!(partial.radius, 8);
    }
}
