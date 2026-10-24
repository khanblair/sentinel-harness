//! The density choices of Settings, Appearance. Settings only write the three values in
//! `DENSITIES`. A stored value that drifted is shown as the nearest of them.

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
}
