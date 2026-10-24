//! A reservation: one task's exclusive claim on one file path.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::ids::{ReservationId, TaskId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReservationState {
    Requested,
    Waiting,
    Granted,
    Released,
    /// Taken back after a crash, a kill, or a violation.
    Revoked,
    /// The waiter gave up.
    Abandoned,
}

impl ReservationState {
    /// See the Reservation state machine in `docs/architecture.md`.
    pub fn can_move_to(self, next: ReservationState) -> bool {
        use ReservationState as S;
        matches!(
            (self, next),
            (S::Requested, S::Granted | S::Waiting)
                | (S::Waiting, S::Granted | S::Abandoned)
                | (S::Granted, S::Released | S::Revoked)
        )
    }

    /// A held path blocks other tasks.
    pub fn is_held(self) -> bool {
        self == ReservationState::Granted
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reservation {
    pub id: ReservationId,
    pub task_id: TaskId,
    pub path: String,
    pub state: ReservationState,
    #[serde(with = "time::serde::rfc3339")]
    pub reserved_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub released_at: Option<OffsetDateTime>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn granted_moves_to_released_or_revoked_only() {
        assert!(ReservationState::Granted.can_move_to(ReservationState::Released));
        assert!(ReservationState::Granted.can_move_to(ReservationState::Revoked));
        assert!(!ReservationState::Granted.can_move_to(ReservationState::Waiting));
    }

    #[test]
    fn waiting_can_be_granted_or_abandoned() {
        assert!(ReservationState::Waiting.can_move_to(ReservationState::Granted));
        assert!(ReservationState::Waiting.can_move_to(ReservationState::Abandoned));
        assert!(!ReservationState::Released.can_move_to(ReservationState::Granted));
    }

    #[test]
    fn only_granted_is_held() {
        assert!(ReservationState::Granted.is_held());
        assert!(!ReservationState::Waiting.is_held());
    }

    #[test]
    fn reservation_round_trips_through_json() {
        let r = Reservation {
            id: ReservationId::new(7),
            task_id: TaskId::new("T-01"),
            path: "src/a.rs".into(),
            state: ReservationState::Granted,
            reserved_at: OffsetDateTime::UNIX_EPOCH,
            released_at: None,
        };
        let back: Reservation = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(back, r);
    }
}
