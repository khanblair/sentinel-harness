//! The budget rules of Settings, Budget, and the routing choices of Settings, Agents and tools.
//! The cap can only go down, so the lower-cap check here is the only path to a new cap.

use sentinel_core::Cost;

/// True when `text` is a dollar amount above zero and strictly below the current cap. The daemon
/// refuses a raise too (`POST /api/cap/lower`), so this check only keeps the form honest.
pub fn lower_cap_valid(text: &str, current: Cost) -> bool {
    match Cost::parse_usd(text) {
        Ok(next) => next.micro_usd() > 0 && next.micro_usd() < current.micro_usd(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_dollars() -> Cost {
        Cost::from_micro_usd(2_000_000, false)
    }

    #[test]
    fn a_lower_cap_is_accepted() {
        assert!(lower_cap_valid("1.50", two_dollars()));
    }

    #[test]
    fn a_cap_equal_to_or_above_the_current_one_is_refused() {
        assert!(!lower_cap_valid("2.00", two_dollars()));
        assert!(!lower_cap_valid("9.00", two_dollars()));
    }

    #[test]
    fn zero_and_text_that_is_not_money_are_refused() {
        assert!(!lower_cap_valid("0", two_dollars()));
        assert!(!lower_cap_valid("abc", two_dollars()));
        assert!(!lower_cap_valid("", two_dollars()));
    }
}
