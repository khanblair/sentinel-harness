const DARK: &[(&str, &str)] = &[
    ("bg", "#191919"),
    ("sidebar", "#141414"),
    ("surface", "#212121"),
    ("surface2", "#2B2B2B"),
    ("border", "#303030"),
    ("text", "#ECECEA"),
    ("muted", "#9B9A96"),
    ("accent", "#E8963A"),
    ("accent-fg", "#888888"),
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
    // Use very dark accent-fg on light accent to achieve >= 4.5:1 contrast
    ("accent-fg", "#000000"),
    ("ok", "#1E7F55"),
    ("warn", "#8A6A00"),
    ("danger", "#B3262B"),
    ("info", "#1F5FBF"),
    ("lock", "#6B3FC4"),
];

type TintRow = (&'static str, [&'static str; 5]);
const DARK_TINTS: &[TintRow] = &[
    ("warm", ["#1B1917", "#161412", "#242120", "#2E2A28", "#34302D"]),
    ("cool", ["#16191D", "#12151A", "#1E2228", "#272C34", "#2C323B"]),
];
const LIGHT_TINTS: &[TintRow] = &[
    ("warm", ["#FBF9F6", "#F3EFE9", "#FFFFFF", "#ECE6DD", "#E2DAD0"]),
    ("cool", ["#F7F9FB", "#EEF2F6", "#FFFFFF", "#E5EBF1", "#D9E1E9"]),
];