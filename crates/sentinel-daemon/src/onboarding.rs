//! Onboarding progress in the store's settings table. It survives a restart.

use sentinel_core::OnboardingState;
use sentinel_store::{Store, StoreError};

const STEP_KEY: &str = "onboarding.step";
const SKIPPED_KEY: &str = "onboarding.skipped";
const COMPLETE_KEY: &str = "onboarding.complete";
const LAST_STEP: u8 = 5;
const SKIPPABLE: [u8; 2] = [4, 5];

#[derive(Debug, thiserror::Error)]
pub enum OnboardingError {
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("onboarding step {0} is out of range")]
    InvalidStep(u8),
    #[error("onboarding step {0} cannot be skipped")]
    NotSkippable(u8),
    #[error("onboarding state is corrupt")]
    Corrupt,
}

pub type Result<T> = std::result::Result<T, OnboardingError>;

pub fn load(store: &Store) -> Result<OnboardingState> {
    let step = match store.get_setting(STEP_KEY)? {
        None => 0,
        Some(text) => text.parse().map_err(|_| OnboardingError::Corrupt)?,
    };
    let skipped = match store.get_setting(SKIPPED_KEY)? {
        None => Vec::new(),
        Some(text) => parse_skipped(&text)?,
    };
    let complete = store.get_setting(COMPLETE_KEY)?.as_deref() == Some("true");
    Ok(OnboardingState {
        step,
        complete,
        skipped,
    })
}

pub fn set_step(store: &Store, step: u8) -> Result<OnboardingState> {
    check_step(step)?;
    store.set_setting(STEP_KEY, &step.to_string())?;
    load(store)
}

pub fn skip_step(store: &Store, step: u8) -> Result<OnboardingState> {
    check_step(step)?;
    if !SKIPPABLE.contains(&step) {
        return Err(OnboardingError::NotSkippable(step));
    }
    let mut state = load(store)?;
    if !state.skipped.contains(&step) {
        state.skipped.push(step);
        state.skipped.sort_unstable();
    }
    store.set_setting(SKIPPED_KEY, &join_steps(&state.skipped))?;
    let next = (step + 1).min(LAST_STEP);
    store.set_setting(STEP_KEY, &next.to_string())?;
    load(store)
}

pub fn complete(store: &Store) -> Result<OnboardingState> {
    store.set_setting(STEP_KEY, &LAST_STEP.to_string())?;
    store.set_setting(COMPLETE_KEY, "true")?;
    load(store)
}

pub fn reset(store: &Store) -> Result<OnboardingState> {
    store.set_setting(STEP_KEY, "0")?;
    store.set_setting(SKIPPED_KEY, "")?;
    store.set_setting(COMPLETE_KEY, "false")?;
    load(store)
}

fn check_step(step: u8) -> Result<()> {
    if step > LAST_STEP {
        return Err(OnboardingError::InvalidStep(step));
    }
    Ok(())
}

fn parse_skipped(text: &str) -> Result<Vec<u8>> {
    let mut steps = Vec::new();
    for part in text.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let step: u8 = part.parse().map_err(|_| OnboardingError::Corrupt)?;
        steps.push(step);
    }
    steps.sort_unstable();
    steps.dedup();
    Ok(steps)
}

fn join_steps(steps: &[u8]) -> String {
    steps
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_moves_and_rejects_values_above_five() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(load(&store).unwrap().step, 0);
        assert_eq!(set_step(&store, 3).unwrap().step, 3);
        assert!(set_step(&store, 6).is_err());
    }

    #[test]
    fn only_the_phone_and_dry_run_steps_can_be_skipped() {
        let store = Store::open_in_memory().unwrap();
        for step in [1, 2, 3] {
            assert!(skip_step(&store, step).is_err(), "step {step}");
        }
        let state = skip_step(&store, 4).unwrap();
        assert_eq!(state.step, 5);
        assert_eq!(state.skipped, vec![4]);
        let state = skip_step(&store, 5).unwrap();
        assert_eq!(state.skipped, vec![4, 5]);
    }

    #[test]
    fn complete_marks_the_last_step_and_reset_clears_it() {
        let store = Store::open_in_memory().unwrap();
        let state = complete(&store).unwrap();
        assert_eq!(state.step, 5);
        assert!(state.complete);
        let state = reset(&store).unwrap();
        assert_eq!(state.step, 0);
        assert!(!state.complete);
        assert_eq!(state.skipped, Vec::<u8>::new());
    }

    #[test]
    fn state_survives_closing_and_reopening_the_store() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("state.db");
        {
            let store = Store::open(&path).unwrap();
            set_step(&store, 2).unwrap();
            skip_step(&store, 4).unwrap();
        }
        let store = Store::open(&path).unwrap();
        let state = load(&store).unwrap();
        assert_eq!(state.step, 5);
        assert_eq!(state.skipped, vec![4]);
        assert!(!state.complete);
    }

    #[test]
    fn corrupt_skipped_text_is_an_error() {
        let store = Store::open_in_memory().unwrap();
        store.set_setting(SKIPPED_KEY, "one,two").unwrap();
        assert!(matches!(load(&store), Err(OnboardingError::Corrupt)));
    }
}
