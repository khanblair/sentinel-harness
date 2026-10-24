//! Fee ledger library for tracking account balances.

/// Ledger that records a sequence of account entries.
#[derive(Debug, Default)]
pub struct Ledger {
    entries: Vec<(String, i64)>,
}

impl Ledger {
    /// Create a new empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Post an entry for an account with the given amount in cents.
    pub fn post(&mut self, account: &str, amount_cents: i64) -> Result<(), String> {
        self.entries.push((account.to_string(), amount_cents));
        Ok(())
    }

    /// Return the sum of all amounts for the given account.
    pub fn balance(&self, account: &str) -> i64 {
        self.entries
            .iter()
            .filter(|(name, _)| name == account)
            .fold(0i64, |acc, (_, amount)| acc + amount)
    }
}

impl Default for Ledger {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}
