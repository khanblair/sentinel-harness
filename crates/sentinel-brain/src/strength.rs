//! Note strength: how much a note is used and trusted. Weak notes load last.

use crate::note::Author;

pub const FLOOR: f32 = 0.05;
pub const CEILING: f32 = 1.0;
/// Strength of a note when it is first written.
pub const INITIAL: f32 = 0.5;
/// Days unused before a floor-strength note may get a retirement proposal.
pub const RETIRE_PROPOSAL_DAYS: u32 = 60;

const RISE_ON_USE: f32 = 0.05;
const RISE_ON_CONFIRM: f32 = 0.15;
const DECAY_PER_DAY: f32 = 0.01;

pub fn rise_on_use(strength: f32) -> f32 {
    clamp(strength + RISE_ON_USE)
}

pub fn rise_on_confirm(strength: f32) -> f32 {
    clamp(strength + RISE_ON_CONFIRM)
}

/// Lower strength by one step per unused day. Never goes below the floor.
pub fn decay(strength: f32, days_unused: u32) -> f32 {
    let days = u16::try_from(days_unused).unwrap_or(u16::MAX);
    clamp(strength - DECAY_PER_DAY * f32::from(days))
}

/// True when a weak, long unused note may get a retirement proposal.
/// Notes you wrote never do. Nothing is deleted here.
pub fn propose_retirement(strength: f32, days_unused: u32, by: Author) -> bool {
    strength <= FLOOR && days_unused >= RETIRE_PROPOSAL_DAYS && by != Author::You
}

fn clamp(strength: f32) -> f32 {
    strength.clamp(FLOOR, CEILING)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn use_and_confirm_raise_strength_by_their_steps() {
        assert!(close(rise_on_use(0.5), 0.55));
        assert!(close(rise_on_confirm(0.5), 0.65));
    }

    #[test]
    fn rise_stops_at_the_ceiling() {
        assert!(close(rise_on_confirm(0.95), CEILING));
        assert!(close(rise_on_use(1.0), CEILING));
    }

    #[test]
    fn decay_lowers_one_step_per_day() {
        assert!(close(decay(0.5, 10), 0.4));
    }

    #[test]
    fn decay_never_goes_below_the_floor() {
        assert!(close(decay(0.5, 10_000), FLOOR));
        assert!(close(decay(0.06, 5), FLOOR));
    }

    #[test]
    fn retirement_proposal_needs_floor_strength_and_long_disuse() {
        assert!(!propose_retirement(
            FLOOR,
            RETIRE_PROPOSAL_DAYS - 1,
            Author::Care
        ));
        assert!(!propose_retirement(0.2, 365, Author::Run));
        assert!(propose_retirement(FLOOR, RETIRE_PROPOSAL_DAYS, Author::Run));
    }

    #[test]
    fn a_note_you_wrote_never_gets_a_retirement_proposal() {
        assert!(!propose_retirement(FLOOR, 1_000, Author::You));
    }
}
