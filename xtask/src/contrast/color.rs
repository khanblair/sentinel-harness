//! Color math: relative luminance and the WCAG contrast ratio.

/// Relative luminance of an sRGB color `#RRGGBB`.
pub(super) fn luminance(hex: &str) -> Option<f64> {
    let digits = hex.strip_prefix('#')?;
    if digits.len() != 6 {
        return None;
    }
    let channel = |i: usize| -> Option<f64> {
        let v = f64::from(u8::from_str_radix(digits.get(i..i + 2)?, 16).ok()?) / 255.0;
        Some(if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        })
    };
    Some(0.2126 * channel(0)? + 0.7152 * channel(2)? + 0.0722 * channel(4)?)
}

/// WCAG contrast ratio of two colors. A color that does not parse gives 0, which fails every rule.
pub(super) fn contrast(a: &str, b: &str) -> f64 {
    let (Some(x), Some(y)) = (luminance(a), luminance(b)) else {
        return 0.0;
    };
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}
