use fee_ledger::ledger::Ledger;

#[test]
fn post_then_balance_adds_up() {
    let mut ledger = Ledger::new();

    ledger.post("a", 500);
    ledger.post("a", 250);

    assert_eq!(ledger.balance("a"), 750);
}

#[test]
fn accounts_stay_separate() {
    let mut ledger = Ledger::new();

    ledger.post("a", 100);
    ledger.post("b", 40);

    assert_eq!(ledger.balance("a"), 100);
    assert_eq!(ledger.balance("b"), 40);
}
