//! Brain modes: what the Brain may read and write, and who may move it.

use sentinel_core::BrainMode;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ModeError {
    #[error("a phone cannot move the Brain from {from:?} to {to:?}")]
    PhoneLoosens { from: BrainMode, to: BrainMode },
}

/// Only `learn` lets runs write notes.
pub fn may_write(mode: BrainMode) -> bool {
    mode == BrainMode::Learn
}

/// Agents may read notes in every mode except `off`.
pub fn may_read(mode: BrainMode) -> bool {
    mode != BrainMode::Off
}

/// Move the mode. A phone may only tighten it, never loosen it.
pub fn set_mode(
    current: BrainMode,
    target: BrainMode,
    from_phone: bool,
) -> Result<BrainMode, ModeError> {
    if from_phone && !current.phone_may_set(target) {
        return Err(ModeError::PhoneLoosens {
            from: current,
            to: target,
        });
    }
    Ok(target)
}

/// Live runs read the Brain but write nothing.
pub fn live_run_default() -> BrainMode {
    BrainMode::ReadOnly
}

/// A hackathon run never writes, whatever the mode says.
pub fn hackathon_mode_cap(mode: BrainMode, hackathon: bool) -> BrainMode {
    if hackathon {
        mode.min(BrainMode::ReadOnly)
    } else {
        mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_learn_may_write() {
        assert!(may_write(BrainMode::Learn));
        assert!(!may_write(BrainMode::ReadOnly));
        assert!(!may_write(BrainMode::Off));
    }

    #[test]
    fn read_is_allowed_unless_off() {
        assert!(may_read(BrainMode::ReadOnly));
        assert!(may_read(BrainMode::Learn));
        assert!(!may_read(BrainMode::Off));
    }

    #[test]
    fn phone_can_tighten_the_mode() {
        let mode = set_mode(BrainMode::Learn, BrainMode::ReadOnly, true);
        assert_eq!(mode, Ok(BrainMode::ReadOnly));
    }

    #[test]
    fn phone_cannot_loosen_the_mode() {
        let mode = set_mode(BrainMode::ReadOnly, BrainMode::Learn, true);
        assert_eq!(
            mode,
            Err(ModeError::PhoneLoosens {
                from: BrainMode::ReadOnly,
                to: BrainMode::Learn,
            })
        );
    }

    #[test]
    fn pc_can_loosen_the_mode() {
        assert_eq!(
            set_mode(BrainMode::ReadOnly, BrainMode::Learn, false),
            Ok(BrainMode::Learn)
        );
    }

    #[test]
    fn live_runs_default_to_read_only() {
        assert_eq!(live_run_default(), BrainMode::ReadOnly);
    }

    #[test]
    fn hackathon_caps_learn_to_read_only() {
        assert_eq!(
            hackathon_mode_cap(BrainMode::Learn, true),
            BrainMode::ReadOnly
        );
        assert_eq!(hackathon_mode_cap(BrainMode::Off, true), BrainMode::Off);
        assert_eq!(
            hackathon_mode_cap(BrainMode::Learn, false),
            BrainMode::Learn
        );
    }
}
