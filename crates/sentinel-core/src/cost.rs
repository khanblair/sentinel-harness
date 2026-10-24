//! Money. Stored as integer micro-dollars, never floats.

use std::fmt;
use std::ops::Add;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

const MICRO_PER_USD: u64 = 1_000_000;

/// An amount of money. `estimated` is true when any part of it was guessed because a tool
/// reported no dollar figure.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Cost {
    micro_usd: u64,
    estimated: bool,
}

impl Cost {
    pub const ZERO: Cost = Cost {
        micro_usd: 0,
        estimated: false,
    };

    pub fn from_micro_usd(micro_usd: u64, estimated: bool) -> Self {
        Self {
            micro_usd,
            estimated,
        }
    }

    /// Parse a dollar amount such as `1.50` or `0.009`. Up to 6 decimals, no signs, no floats.
    pub fn parse_usd(text: &str) -> Result<Self, CoreError> {
        let bad = || CoreError::InvalidMoney(text.to_owned());
        let text = text.trim().trim_start_matches('$');
        let (whole, frac) = text.split_once('.').unwrap_or((text, ""));
        let digits_only = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
        if whole.is_empty() || frac.len() > 6 || !digits_only(whole) || !digits_only(frac) {
            return Err(bad());
        }
        let whole: u64 = whole.parse().map_err(|_| bad())?;
        let frac_micro: u64 = format!("{frac:0<6}").parse().map_err(|_| bad())?;
        let micro = whole
            .checked_mul(MICRO_PER_USD)
            .and_then(|w| w.checked_add(frac_micro));
        micro
            .map(|m| Self::from_micro_usd(m, false))
            .ok_or_else(bad)
    }

    pub fn micro_usd(self) -> u64 {
        self.micro_usd
    }

    pub fn is_estimated(self) -> bool {
        self.estimated
    }

    /// Whole percent of `cap` that this cost is, rounded down and capped at 255. A zero cap
    /// counts as fully spent.
    pub fn percent_of(self, cap: Cost) -> u8 {
        if cap.micro_usd == 0 {
            return u8::MAX;
        }
        let percent = u128::from(self.micro_usd) * 100 / u128::from(cap.micro_usd);
        u8::try_from(percent).unwrap_or(u8::MAX)
    }

    #[must_use]
    pub fn saturating_sub(self, other: Cost) -> Cost {
        Cost {
            micro_usd: self.micro_usd.saturating_sub(other.micro_usd),
            estimated: self.estimated,
        }
    }
}

impl Add for Cost {
    type Output = Cost;

    fn add(self, rhs: Cost) -> Cost {
        Cost {
            micro_usd: self.micro_usd.saturating_add(rhs.micro_usd),
            estimated: self.estimated || rhs.estimated,
        }
    }
}

impl fmt::Display for Cost {
    /// `$0.04`. Amounts under one cent keep three decimals so `$0.009` does not read as `$0.01`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.micro_usd != 0 && self.micro_usd < 10_000 {
            let whole = self.micro_usd / MICRO_PER_USD;
            let frac = self.micro_usd % MICRO_PER_USD;
            return write!(f, "${whole}.{:03}", frac / 1_000);
        }
        let cents = (self.micro_usd + 5_000) / 10_000;
        write!(f, "${}.{:02}", cents / 100, cents % 100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reads_dollars_and_cents() {
        assert_eq!(Cost::parse_usd("1.50").unwrap().micro_usd(), 1_500_000);
        assert_eq!(Cost::parse_usd("$2").unwrap().micro_usd(), 2_000_000);
        assert_eq!(Cost::parse_usd("0.009").unwrap().micro_usd(), 9_000);
    }

    #[test]
    fn parse_rejects_bad_input() {
        for bad in ["", "-1", "1.2.3", "abc", "1.1234567", "."] {
            assert!(Cost::parse_usd(bad).is_err(), "{bad} should fail");
        }
    }

    #[test]
    fn display_formats_like_the_ui_copy() {
        assert_eq!(Cost::from_micro_usd(40_000, false).to_string(), "$0.04");
        assert_eq!(Cost::from_micro_usd(9_000, false).to_string(), "$0.009");
        assert_eq!(Cost::from_micro_usd(0, false).to_string(), "$0.00");
        assert_eq!(Cost::from_micro_usd(1_995_000, false).to_string(), "$2.00");
    }

    #[test]
    fn adding_keeps_the_estimate_flag() {
        let sum = Cost::from_micro_usd(1, false) + Cost::from_micro_usd(2, true);
        assert_eq!(sum.micro_usd(), 3);
        assert!(sum.is_estimated());
    }

    #[test]
    fn percent_of_rounds_down_and_handles_zero_cap() {
        let cap = Cost::from_micro_usd(2_000_000, false);
        assert_eq!(Cost::from_micro_usd(1_400_000, false).percent_of(cap), 70);
        assert_eq!(Cost::from_micro_usd(1_999_999, false).percent_of(cap), 99);
        assert_eq!(Cost::ZERO.percent_of(Cost::ZERO), 255);
    }
}
