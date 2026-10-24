//! `TokenEditor`: one color input per theme token in Settings, Appearance. Each edit is kept in
//! `prefs.overrides` for the theme that is showing, so light and dark keep their own values
//! (`docs/ui-tokens.md`, Settings-Driven Tokens).

use leptos::prelude::*;

use crate::prefs::{self, Prefs, Resolved};
use crate::state::{Ui, use_ui};

/// The color tokens the editor shows, with their labels.
pub const TOKEN_KEYS: [(&str, &str); 10] = [
    ("bg", "Background"),
    ("sidebar", "Sidebar"),
    ("surface", "Surface"),
    ("text", "Text"),
    ("muted", "Muted text"),
    ("accent", "Accent"),
    ("ok", "Verified"),
    ("warn", "Warning"),
    ("danger", "Danger"),
    ("info", "Running"),
];

/// `#RRGGBB` in upper case, or `None`. Color inputs send lower case, so case is accepted.
pub fn normalize_hex(value: &str) -> Option<String> {
    let digits = value.strip_prefix('#')?;
    if digits.len() == 6 && digits.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(format!("#{}", digits.to_ascii_uppercase()))
    } else {
        None
    }
}

/// Whether a device-wide `dark` setting applies now. Only the theme and the system setting
/// decide it, so this is safe to call from a closure.
pub fn resolved_now(prefs: &Prefs) -> Resolved {
    prefs.resolve(crate::theme::system_is_dark())
}

/// Sets one token for one theme. An unknown key or a value that is not `#RRGGBB` leaves the
/// prefs as they are.
pub fn token_edit(prefs: &Prefs, resolved: Resolved, key: &str, value: &str) -> Prefs {
    let known = TOKEN_KEYS.iter().any(|(name, _)| *name == key);
    let Some(hex) = normalize_hex(value).filter(|_| known) else {
        return prefs.clone();
    };
    let mut next = prefs.clone();
    let overrides = match resolved {
        Resolved::Dark => &mut next.overrides.dark,
        Resolved::Light => &mut next.overrides.light,
    };
    overrides.insert(key.to_owned(), hex);
    next
}

/// The value the editor shows for one token in the theme that is on screen.
pub fn token_value(prefs: &Prefs, resolved: Resolved, key: &str) -> String {
    prefs::tokens(prefs, resolved)
        .get(key)
        .cloned()
        .unwrap_or_default()
}

#[component]
pub fn TokenEditor() -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="tokgrid" style="margin-top:.5em">
            {TOKEN_KEYS
                .into_iter()
                .map(|(key, label)| token_input(ui, key, label))
                .collect_view()}
        </div>
    }
}

fn token_input(ui: Ui, key: &'static str, label: &'static str) -> impl IntoView {
    let value = move || {
        let prefs = ui.prefs.get();
        token_value(&prefs, resolved_now(&prefs), key)
    };
    view! {
        <label>
            <input
                type="color"
                aria-label=label
                prop:value=value
                on:input=move |ev| {
                    let next = event_target_value(&ev);
                    ui.prefs.update(|p| *p = token_edit(p, resolved_now(p), key, &next));
                }
            />
            {label}
        </label>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_edit_stores_an_upper_case_hex_for_the_shown_theme_only() {
        let prefs = Prefs::default();
        let next = token_edit(&prefs, Resolved::Dark, "bg", "#00ff7f");
        assert_eq!(
            next.overrides.dark.get("bg").map(String::as_str),
            Some("#00FF7F")
        );
        assert!(next.overrides.light.is_empty());
        assert_eq!(prefs::tokens(&next, Resolved::Light)["bg"], "#FAFAF9");
    }

    #[test]
    fn token_edit_rejects_a_value_that_is_not_a_hex_color() {
        let prefs = Prefs::default();
        for bad in ["red", "#12345", "#GGGGGG", "123456", ""] {
            assert_eq!(
                token_edit(&prefs, Resolved::Dark, "bg", bad),
                prefs,
                "{bad}"
            );
        }
    }

    #[test]
    fn token_edit_rejects_a_key_the_editor_does_not_show() {
        let prefs = Prefs::default();
        let next = token_edit(&prefs, Resolved::Dark, "lock", "#000000");
        assert_eq!(next, prefs);
    }

    #[test]
    fn token_value_reads_the_edited_color_back() {
        let prefs = token_edit(&Prefs::default(), Resolved::Light, "text", "#101010");
        assert_eq!(token_value(&prefs, Resolved::Light, "text"), "#101010");
        assert_eq!(token_value(&prefs, Resolved::Dark, "text"), "#ECECEA");
    }
}
