//! Icon paths, ported from the prototype's `ICONS` table. One outline set, 1.7 px stroke.

/// `(name, svg inner markup)`.
pub const ICONS: &[(&str, &str)] = &[
    (
        "sidebar",
        r#"<rect x="3" y="4" width="18" height="16" rx="3"/><path d="M9 4v16"/>"#,
    ),
    (
        "panel",
        r#"<rect x="3" y="4" width="18" height="16" rx="3"/><path d="M15 4v16"/>"#,
    ),
    ("back", r#"<path d="M15 6l-6 6 6 6"/>"#),
    ("fwd", r#"<path d="M9 6l6 6-6 6"/>"#),
    ("down", r#"<path d="M6 9l6 6 6-6"/>"#),
    ("right", r#"<path d="M9 6l6 6-6 6"/>"#),
    (
        "search",
        r#"<circle cx="11" cy="11" r="6"/><path d="M20 20l-4-4"/>"#,
    ),
    (
        "code",
        r#"<path d="M8 8l-4 4 4 4M16 8l4 4-4 4M14 5l-4 14"/>"#,
    ),
    (
        "tasks",
        r#"<rect x="3" y="4" width="18" height="16" rx="3"/><path d="M7 9h2M12 9h5M7 12h2M12 12h5M7 15h2M12 15h5"/>"#,
    ),
    ("terminal", r#"<path d="M4 7l5 5-5 5M12 18h8"/>"#),
    (
        "diff",
        r#"<rect x="4" y="3" width="16" height="18" rx="3"/><path d="M12 8v6M9 11h6M9 17h6"/>"#,
    ),
    (
        "lock",
        r#"<rect x="5" y="11" width="14" height="9" rx="2"/><path d="M8 11V8a4 4 0 018 0v3"/>"#,
    ),
    (
        "phone",
        r#"<rect x="7" y="3" width="10" height="18" rx="2.5"/><path d="M11 18h2"/>"#,
    ),
    (
        "tablet",
        r#"<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M11 18h2"/>"#,
    ),
    (
        "desktop",
        r#"<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>"#,
    ),
    (
        "folder",
        r#"<path d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z"/>"#,
    ),
    (
        "more",
        r#"<circle cx="5" cy="12" r="1.3"/><circle cx="12" cy="12" r="1.3"/><circle cx="19" cy="12" r="1.3"/>"#,
    ),
    ("plus", r#"<path d="M12 5v14M5 12h14"/>"#),
    ("x", r#"<path d="M6 6l12 12M18 6L6 18"/>"#),
    (
        "mic",
        r#"<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5 11a7 7 0 0014 0M12 18v3"/>"#,
    ),
    ("send", r#"<path d="M12 19V5M6 11l6-6 6 6"/>"#),
    ("check", r#"<path d="M5 12l5 5 9-10"/>"#),
    (
        "shield",
        r#"<path d="M12 3l7 3v5c0 5-3 8-7 10-4-2-7-5-7-10V6z"/>"#,
    ),
    (
        "agent",
        r#"<rect x="5" y="8" width="14" height="10" rx="3"/><path d="M12 8V5M9 13h.01M15 13h.01"/>"#,
    ),
    (
        "branch",
        r#"<circle cx="6" cy="5" r="2"/><circle cx="6" cy="19" r="2"/><circle cx="18" cy="9" r="2"/><path d="M6 7v10M18 11c0 4-6 3-12 6"/>"#,
    ),
    (
        "sun",
        r#"<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.5 1.5M17.5 17.5L19 19M5 19l1.5-1.5M17.5 6.5L19 5"/>"#,
    ),
    (
        "moon",
        r#"<path d="M20 14.5A8 8 0 019.5 4 8 8 0 1020 14.5z"/>"#,
    ),
    (
        "gear",
        r#"<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06a1.65 1.65 0 001.82.33H9a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06a1.65 1.65 0 00-.33 1.82V9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z"/>"#,
    ),
    ("arrow", r#"<path d="M5 12h14M13 6l6 6-6 6"/>"#),
    ("play", r#"<path d="M7 5l12 7-12 7z"/>"#),
    ("pause", r#"<path d="M8 5v14M16 5v14"/>"#),
    (
        "stop",
        r#"<rect x="6" y="6" width="12" height="12" rx="2"/>"#,
    ),
    (
        "alert",
        r#"<path d="M12 4l9 16H3z"/><path d="M12 10v4M12 17h.01"/>"#,
    ),
    (
        "info",
        r#"<circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/>"#,
    ),
    (
        "copy",
        r#"<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V6a2 2 0 00-2-2H6a2 2 0 00-2 2v8a2 2 0 002 2h2"/>"#,
    ),
    ("refresh", r#"<path d="M20 11a8 8 0 10-2 6M20 5v6h-6"/>"#),
    ("undo", r#"<path d="M9 8L4 13l5 5M4 13h11a5 5 0 000-10"/>"#),
    (
        "key",
        r#"<circle cx="8" cy="15" r="4"/><path d="M11 12l9-9M16 7l3 3"/>"#,
    ),
    (
        "net",
        r#"<circle cx="12" cy="5" r="2"/><circle cx="5" cy="18" r="2"/><circle cx="19" cy="18" r="2"/><path d="M12 7v5M12 12l-6 5M12 12l6 5"/>"#,
    ),
    ("download", r#"<path d="M12 4v11M7 11l5 5 5-5M5 20h14"/>"#),
    (
        "book",
        r#"<path d="M5 4h10a4 4 0 014 4v12H9a4 4 0 01-4-4z"/><path d="M5 16a4 4 0 014-4h10"/>"#,
    ),
    ("spark", r#"<path d="M12 3l2 6 6 2-6 2-2 6-2-6-6-2 6-2z"/>"#),
    (
        "dollar",
        r#"<path d="M12 3v18M16 7.5c0-1.7-2-3-4-3s-4 1.1-4 3 1.8 2.5 4 3 4 1.2 4 3-2 3-4 3-4-1.3-4-3"/>"#,
    ),
    (
        "file",
        r#"<path d="M6 3h8l5 5v13H6z"/><path d="M14 3v5h5"/>"#,
    ),
    (
        "expand",
        r#"<path d="M15 4h5v5M9 20H4v-5M20 4l-6 6M4 20l6-6"/>"#,
    ),
    (
        "eye",
        r#"<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>"#,
    ),
    (
        "list",
        r#"<path d="M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01"/>"#,
    ),
    (
        "keyboard",
        r#"<rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M7 14h10"/>"#,
    ),
    (
        "palette",
        r#"<path d="M12 3a9 9 0 100 18c1.5 0 2-1 1.5-2s0-2 1.5-2h2a3 3 0 003-3c0-6-4-11-8-11z"/><circle cx="8" cy="10" r="1"/><circle cx="12" cy="7" r="1"/><circle cx="16" cy="10" r="1"/>"#,
    ),
    (
        "link",
        r#"<path d="M10 14a4 4 0 005.7 0l3-3a4 4 0 00-5.7-5.7l-1 1M14 10a4 4 0 00-5.7 0l-3 3a4 4 0 005.7 5.7l1-1"/>"#,
    ),
    (
        "sliders",
        r#"<path d="M4 7h10M18 7h2M4 17h2M10 17h10M14 5v4M6 15v4"/>"#,
    ),
    (
        "play2",
        r#"<circle cx="12" cy="12" r="9"/><path d="M10 8l6 4-6 4z"/>"#,
    ),
    (
        "brain",
        r#"<path d="M9 4a3 3 0 00-3 3 3 3 0 00-2 5 3 3 0 002 5 3 3 0 005 1V4z"/><path d="M15 4a3 3 0 013 3 3 3 0 012 5 3 3 0 01-2 5 3 3 0 01-5 1V4z"/>"#,
    ),
    (
        "clock",
        r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>"#,
    ),
];

/// Icons drawn filled instead of outlined.
pub const FILLED: &[&str] = &["more", "play"];

/// The inner markup of an icon, or an empty string for an unknown name.
pub fn markup(name: &str) -> &'static str {
    ICONS
        .iter()
        .find(|(n, _)| *n == name)
        .map_or("", |(_, m)| m)
}

pub fn is_filled(name: &str) -> bool {
    FILLED.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_icons_have_markup() {
        for name in ["sidebar", "lock", "brain", "check"] {
            assert!(!markup(name).is_empty(), "{name}");
        }
    }

    #[test]
    fn unknown_icon_is_empty() {
        assert_eq!(markup("nope"), "");
    }

    #[test]
    fn only_more_and_play_are_filled() {
        assert!(is_filled("more") && is_filled("play") && !is_filled("lock"));
    }
}
