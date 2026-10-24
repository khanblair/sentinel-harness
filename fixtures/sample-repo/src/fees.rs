//! Fee ledger calculations.

/// Returns the fee in cents, rounded down.
pub fn fee_cents(amount_cents: i64) -> i64 {
    amount_cents / 100
}
