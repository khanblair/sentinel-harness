# fee-ledger backlog

## T-01 Add a health check function
A `health()` function returns `ok`. Done when a test proves it.

## T-02 Reject negative amounts
Posting a negative amount returns an error and changes nothing. Done when a test proves it.

## T-03 Round fees half up
A fee of 2.5 cents is charged as 3 cents, not 2. Done when `fee_cents(250)` is 3 and a test proves it.

## T-04 Add a daily transfer limit
An account may not move more than 100000 cents in one day. Done when a test proves the limit.

## T-05 Add a rate limit to transfers
Refuse more than 30 transfers per minute, with a check the API layer can call. It reuses the limits module from the daily limit task.

## T-06 Add package metadata (already done)
The package name, version, and edition are already set in `Cargo.toml`. Nothing is left to do here.

## T-07 Round fees down
A fee of 2.5 cents is charged as 2 cents. Done when `fee_cents(250)` is 2.

## T-08 Add a slow audit export
Write an audit export of all ledger entries. It is expected to take a long time.

## T-09 Add a failing integration
Connect the ledger to a payment partner. This task is expected to fail in the sample run.

## T-10 Tidy the ledger tests
Rename the test helpers in the ledger tests so they read better. Nothing else changes.
