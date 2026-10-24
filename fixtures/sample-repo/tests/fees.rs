// tests/fees.rs

use fee_ledger::fees::fee_cents;

#[test]
fn fee_is_one_percent() {
    assert_eq!(fee_cents(10_000), 100);
}

#[test]
fn fee_rounds_down() {
    assert_eq!(fee_cents(250), 2);
}
