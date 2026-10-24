//! Reservation queries.

use rusqlite::{OptionalExtension, Row, params, params_from_iter};
use sentinel_core::{CoreError, Reservation, ReservationId, ReservationState, TaskId};
use time::OffsetDateTime;

use crate::{
    Result, Store, StoreError, enum_from_text, enum_text, state_filter, time_from_text, time_text,
};

const RESERVATION_COLUMNS: &str = "id, task_id, path, state, reserved_at, released_at";

type ReservationRow = (i64, String, String, String, String, Option<String>);

impl Store {
    // The public API fixes these four arguments, and clippy counts `self`.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_reservation(
        &self,
        task: &TaskId,
        path: &str,
        state: ReservationState,
        at: OffsetDateTime,
    ) -> Result<Reservation> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO reservations (task_id, path, state, reserved_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![task.as_str(), path, enum_text(&state)?, time_text(at)?],
        )?;
        Ok(Reservation {
            id: ReservationId::new(connection.last_insert_rowid()),
            task_id: task.clone(),
            path: path.to_owned(),
            state,
            reserved_at: at,
            released_at: None,
        })
    }

    pub fn set_reservation_state(
        &self,
        id: ReservationId,
        next: ReservationState,
        at: OffsetDateTime,
    ) -> Result<()> {
        let connection = self.connection()?;
        let stored = connection
            .query_row(
                "SELECT state FROM reservations WHERE id = ?1",
                params![id.get()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| StoreError::NotFound(format!("reservation {}", id.get())))?;
        let current: ReservationState = enum_from_text(&stored)?;
        if !current.can_move_to(next) {
            return Err(StoreError::Core(CoreError::InvalidTransition {
                from: state_name(current),
                to: state_name(next),
            }));
        }
        let released_at = if releases_hold(next) {
            Some(time_text(at)?)
        } else {
            None
        };
        connection.execute(
            "UPDATE reservations SET state = ?1, released_at = COALESCE(?2, released_at)
             WHERE id = ?3",
            params![enum_text(&next)?, released_at, id.get()],
        )?;
        Ok(())
    }

    /// An empty filter means every reservation.
    pub fn list_reservations(&self, states: &[ReservationState]) -> Result<Vec<Reservation>> {
        let connection = self.connection()?;
        let (clause, filters) = state_filter("state", states)?;
        let sql = format!("SELECT {RESERVATION_COLUMNS} FROM reservations{clause} ORDER BY id");
        let rows = {
            let mut statement = connection.prepare(&sql)?;
            let mapped =
                statement.query_map(params_from_iter(filters.iter()), read_reservation_row)?;
            mapped.collect::<rusqlite::Result<Vec<ReservationRow>>>()?
        };
        rows.into_iter().map(reservation_from_row).collect()
    }

    pub fn reservations_of_task(&self, task: &TaskId) -> Result<Vec<Reservation>> {
        let connection = self.connection()?;
        let rows = {
            let mut statement = connection.prepare(&format!(
                "SELECT {RESERVATION_COLUMNS} FROM reservations WHERE task_id = ?1 ORDER BY id"
            ))?;
            let mapped = statement.query_map(params![task.as_str()], read_reservation_row)?;
            mapped.collect::<rusqlite::Result<Vec<ReservationRow>>>()?
        };
        rows.into_iter().map(reservation_from_row).collect()
    }
}

fn releases_hold(state: ReservationState) -> bool {
    matches!(
        state,
        ReservationState::Released | ReservationState::Revoked | ReservationState::Abandoned
    )
}

fn state_name(state: ReservationState) -> &'static str {
    match state {
        ReservationState::Requested => "requested",
        ReservationState::Waiting => "waiting",
        ReservationState::Granted => "granted",
        ReservationState::Released => "released",
        ReservationState::Revoked => "revoked",
        ReservationState::Abandoned => "abandoned",
    }
}

fn read_reservation_row(row: &Row<'_>) -> rusqlite::Result<ReservationRow> {
    columns!(row; 0, 1, 2, 3, 4, 5)
}

fn reservation_from_row(row: ReservationRow) -> Result<Reservation> {
    let (id, task_id, path, state, reserved_at, released_at) = row;
    Ok(Reservation {
        id: ReservationId::new(id),
        task_id: TaskId::new(task_id),
        path,
        state: enum_from_text(&state)?,
        reserved_at: time_from_text(&reserved_at)?,
        released_at: released_at.as_deref().map(time_from_text).transpose()?,
    })
}
