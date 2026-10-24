//! The Reserved panel: which task holds each file, and who waits (`docs/ui-rules.md`, Views).

use std::collections::BTreeMap;

use leptos::prelude::*;
use sentinel_core::{Reservation, ReservationState};

use crate::components::icon::Icon;
use crate::components::reserved_row::{ReservedRow, ReservedRowModel};
use crate::state::use_ui;

/// One row per path, sorted by path. Only granted holds and waiting requests count.
pub fn group_reservations(reservations: &[Reservation]) -> Vec<ReservedRowModel> {
    let mut rows: BTreeMap<&str, ReservedRowModel> = BTreeMap::new();
    for reservation in reservations {
        if !matches!(
            reservation.state,
            ReservationState::Granted | ReservationState::Waiting
        ) {
            continue;
        }
        let row = rows
            .entry(reservation.path.as_str())
            .or_insert_with(|| ReservedRowModel {
                path: reservation.path.clone(),
                holder: None,
                waiting: Vec::new(),
            });
        let task = reservation.task_id.to_string();
        if reservation.state == ReservationState::Granted {
            row.holder = Some(task);
        } else {
            row.waiting.push(task);
        }
    }
    rows.into_values().collect()
}

/// How many tasks are queued behind a held file, summed over all paths.
fn waiting_total(rows: &[ReservedRowModel]) -> usize {
    rows.iter().map(|row| row.waiting.len()).sum()
}

#[component]
pub fn ReservedPanel() -> impl IntoView {
    let ui = use_ui();
    let rows = move || ui.snapshot.with(|s| group_reservations(&s.reservations));
    let held = move || rows().iter().filter(|row| row.holder.is_some()).count();
    let waiting = move || waiting_total(&rows());
    view! {
        <div class="row wrap" style="gap:1em;margin-bottom:.8em">
            <span class="chip"><Icon name="lock" size=12 /> {move || format!("{} reserved", held())}</span>
            <span class="chip">{move || format!("{} waiting", waiting())}</span>
        </div>
        <div class="note" style="margin-bottom:.8em">
            <Icon name="info" size=14 />
            <span>"A file has one writer at a time. Agents can only write files their task has reserved. Anything else is blocked and reverted."</span>
        </div>
        <div class="tree">
            {move || {
                let list = rows();
                if list.is_empty() {
                    view! { <div class="note">"No files are reserved right now."</div> }.into_any()
                } else {
                    list.into_iter()
                        .map(|row| view! { <ReservedRow row=row /> })
                        .collect_view()
                        .into_any()
                }
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{ReservationId, TaskId};
    use time::OffsetDateTime;

    fn reservation(id: i64, task: &str, path: &str, state: ReservationState) -> Reservation {
        Reservation {
            id: ReservationId::new(id),
            task_id: TaskId::new(task),
            path: path.to_owned(),
            state,
            reserved_at: OffsetDateTime::UNIX_EPOCH,
            released_at: None,
        }
    }

    #[test]
    fn holder_and_queue_share_one_row_per_path() {
        let rows = group_reservations(&[
            reservation(1, "T-01", "src/a.rs", ReservationState::Granted),
            reservation(2, "T-02", "src/a.rs", ReservationState::Waiting),
            reservation(3, "T-03", "src/a.rs", ReservationState::Waiting),
        ]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].holder.as_deref(), Some("T-01"));
        assert_eq!(rows[0].waiting, vec!["T-02".to_owned(), "T-03".to_owned()]);
    }

    #[test]
    fn a_waiting_only_path_has_no_holder() {
        let rows = group_reservations(&[reservation(
            1,
            "T-02",
            "src/b.rs",
            ReservationState::Waiting,
        )]);
        assert_eq!(rows[0].holder, None);
        assert_eq!(waiting_total(&rows), 1);
    }

    #[test]
    fn released_and_revoked_rows_are_left_out() {
        let rows = group_reservations(&[
            reservation(1, "T-01", "src/a.rs", ReservationState::Released),
            reservation(2, "T-01", "src/b.rs", ReservationState::Revoked),
        ]);
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn rows_are_sorted_by_path() {
        let rows = group_reservations(&[
            reservation(1, "T-01", "src/z.rs", ReservationState::Granted),
            reservation(2, "T-02", "src/a.rs", ReservationState::Granted),
        ]);
        let paths: Vec<&str> = rows.iter().map(|row| row.path.as_str()).collect();
        assert_eq!(paths, vec!["src/a.rs", "src/z.rs"]);
    }
}
