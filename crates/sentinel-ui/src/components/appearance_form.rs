//! The controls of Settings, Appearance, from theme down to corner radius. The density choices
//! are here too: Settings only write the three values in `DENSITIES`, and a stored value that
//! drifted is shown as the nearest of them.

use leptos::prelude::*;

use crate::components::accent_picker::AccentPicker;
use crate::components::font_select::FontSelect;
use crate::components::range_field::{RangeField, RangeSpec};
use crate::components::toggle::{SegOption, Segmented};
use crate::prefs::{MONO_FONTS, Theme, Tint, UI_FONTS};
use crate::state::use_ui;
use crate::views::settings::Field;

const FS_SPEC: RangeSpec = RangeSpec {
    label: "Text size",
    min: 12,
    max: 18,
    step: 1,
};
const MW_SPEC: RangeSpec = RangeSpec {
    label: "Stream width",
    min: 560,
    max: 980,
    step: 20,
};
const RADIUS_SPEC: RangeSpec = RangeSpec {
    label: "Corner radius",
    min: 2,
    max: 16,
    step: 1,
};

pub const THEME_OPTIONS: &[SegOption] = &[
    SegOption {
        value: "system",
        label: "System",
        icon: "desktop",
    },
    SegOption {
        value: "light",
        label: "Light",
        icon: "sun",
    },
    SegOption {
        value: "dark",
        label: "Dark",
        icon: "moon",
    },
];

pub const TINT_OPTIONS: &[SegOption] = &[
    SegOption {
        value: "neutral",
        label: "Neutral",
        icon: "",
    },
    SegOption {
        value: "warm",
        label: "Warm",
        icon: "",
    },
    SegOption {
        value: "cool",
        label: "Cool",
        icon: "",
    },
];

pub const DENSITY_OPTIONS: &[SegOption] = &[
    SegOption {
        value: "0.8",
        label: "Compact",
        icon: "",
    },
    SegOption {
        value: "1",
        label: "Default",
        icon: "",
    },
    SegOption {
        value: "1.25",
        label: "Spacious",
        icon: "",
    },
];

pub fn theme_key(theme: Theme) -> &'static str {
    match theme {
        Theme::System => "system",
        Theme::Light => "light",
        Theme::Dark => "dark",
    }
}

/// Unknown keys mean System, the default.
pub fn theme_from_key(key: &str) -> Theme {
    match key {
        "light" => Theme::Light,
        "dark" => Theme::Dark,
        _ => Theme::System,
    }
}

pub fn tint_key(tint: Tint) -> &'static str {
    match tint {
        Tint::Neutral => "neutral",
        Tint::Warm => "warm",
        Tint::Cool => "cool",
    }
}

/// Unknown keys mean Neutral, the default.
pub fn tint_from_key(key: &str) -> Tint {
    match key {
        "warm" => Tint::Warm,
        "cool" => Tint::Cool,
        _ => Tint::Neutral,
    }
}

#[component]
pub fn AppearanceControls() -> impl IntoView {
    view! {
        <ThemeFields />
        <TypeFields />
        <SpacingFields />
    }
}

#[component]
fn ThemeFields() -> impl IntoView {
    let ui = use_ui();
    let theme = Signal::derive(move || theme_key(ui.prefs.get().theme).to_owned());
    let tint = Signal::derive(move || tint_key(ui.prefs.get().tint).to_owned());
    view! {
        <Field label="Theme" hint="System follows your device.">
            <Segmented
                options=THEME_OPTIONS
                selected=theme
                on_pick=Callback::new(move |key: String| ui.prefs.update(|p| p.theme = theme_from_key(&key)))
            />
        </Field>
        <Field label="Accent color" hint="Used for the main button, focus and the active tab.">
            <AccentPicker />
        </Field>
        <Field label="Surface tint" hint="Warm, cool or neutral greys.">
            <Segmented
                options=TINT_OPTIONS
                selected=tint
                on_pick=Callback::new(move |key: String| ui.prefs.update(|p| p.tint = tint_from_key(&key)))
            />
        </Field>
    }
}

#[component]
fn TypeFields() -> impl IntoView {
    let ui = use_ui();
    let ui_font = Signal::derive(move || ui.prefs.get().font_ui);
    let mono_font = Signal::derive(move || ui.prefs.get().font_mono);
    let text_size = Signal::derive(move || ui.prefs.get().fs);
    let stream = Signal::derive(move || ui.prefs.get().mw);
    view! {
        <Field label="Interface font">
            <FontSelect
                label="Interface font"
                options=UI_FONTS
                selected=ui_font
                on_pick=Callback::new(move |key: String| ui.prefs.update(|p| p.font_ui = key))
            />
        </Field>
        <Field label="Code font" hint="Agent output, paths and diffs.">
            <FontSelect
                label="Code font"
                options=MONO_FONTS
                selected=mono_font
                on_pick=Callback::new(move |key: String| ui.prefs.update(|p| p.font_mono = key))
            />
        </Field>
        <Field label="Text size" hint="Scales the whole interface.">
            <RangeField
                spec=FS_SPEC
                value=text_size
                on_change=Callback::new(move |px: u32| ui.prefs.update(|p| p.fs = px))
            />
        </Field>
        <Field label="Stream width" hint="Width of the conversation column.">
            <RangeField
                spec=MW_SPEC
                value=stream
                on_change=Callback::new(move |px: u32| ui.prefs.update(|p| p.mw = px))
            />
        </Field>
    }
}

#[component]
fn SpacingFields() -> impl IntoView {
    let ui = use_ui();
    let density = Signal::derive(move || density_option(ui.prefs.get().density).key.to_owned());
    let radius = Signal::derive(move || ui.prefs.get().radius);
    view! {
        <Field label="Density" hint="Space between items.">
            <Segmented
                options=DENSITY_OPTIONS
                selected=density
                on_pick=Callback::new(move |key: String| ui.prefs.update(|p| p.density = density_from_key(&key)))
            />
        </Field>
        <Field label="Corner radius">
            <RangeField
                spec=RADIUS_SPEC
                value=radius
                on_change=Callback::new(move |px: u32| ui.prefs.update(|p| p.radius = px))
            />
        </Field>
    }
}

/// One density choice: the spacing multiplier, its segment key, and its label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Density {
    pub value: f64,
    pub key: &'static str,
    pub label: &'static str,
}

pub const DENSITIES: [Density; 3] = [
    Density {
        value: 0.8,
        key: "0.8",
        label: "Compact",
    },
    Density {
        value: 1.0,
        key: "1",
        label: "Default",
    },
    Density {
        value: 1.25,
        key: "1.25",
        label: "Spacious",
    },
];

/// The choice nearest to `value`. Ties keep the earlier choice.
pub fn density_option(value: f64) -> Density {
    DENSITIES
        .iter()
        .copied()
        .fold(DENSITIES[1], |best, option| {
            if (option.value - value).abs() < (best.value - value).abs() {
                option
            } else {
                best
            }
        })
}

pub fn density_label(value: f64) -> &'static str {
    density_option(value).label
}

/// The multiplier for a segment key. An unknown key gives the default spacing.
pub fn density_from_key(key: &str) -> f64 {
    DENSITIES
        .iter()
        .find(|option| option.key == key)
        .map_or(1.0, |option| option.value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_documented_density_has_its_label() {
        assert_eq!(density_label(0.8), "Compact");
        assert_eq!(density_label(1.0), "Default");
        assert_eq!(density_label(1.25), "Spacious");
    }

    #[test]
    fn a_drifted_density_shows_the_nearest_choice() {
        assert_eq!(density_label(1.2), "Spacious");
        assert_eq!(density_label(0.7), "Compact");
    }

    #[test]
    fn a_segment_key_maps_back_to_its_multiplier() {
        for option in DENSITIES {
            assert!((density_from_key(option.key) - option.value).abs() < f64::EPSILON);
        }
        assert!((density_from_key("nope") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn density_segments_match_the_density_table() {
        for (segment, option) in DENSITY_OPTIONS.iter().zip(DENSITIES) {
            assert_eq!(segment.value, option.key);
            assert_eq!(segment.label, option.label);
        }
    }

    #[test]
    fn theme_and_tint_keys_round_trip() {
        for theme in [Theme::System, Theme::Light, Theme::Dark] {
            assert_eq!(theme_from_key(theme_key(theme)), theme);
        }
        for tint in [Tint::Neutral, Tint::Warm, Tint::Cool] {
            assert_eq!(tint_from_key(tint_key(tint)), tint);
        }
        assert_eq!(theme_from_key("nope"), Theme::System);
        assert_eq!(tint_from_key("nope"), Tint::Neutral);
    }
}
