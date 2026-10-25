//! The Reserved panel: which task holds each file, and who waits (`docs/ui-rules.md`, Views).

use std::collections::{BTreeMap, HashMap};

use leptos::prelude::*;
use sentinel_core::{EventRecord, Reservation, ReservationState};

use crate::components::icon::Icon;
use crate::components::reserved_row::{HolderDetail, ReservedRow, ReservedRowModel};
use crate::state::{refresh, use_ui};
use crate::views::board::owner_of;
use crate::views::live_fetch::keep_live;

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

/// The agent and grant time of each granted file, keyed by path. The agent comes from the
/// holder task's latest `X goes to AGENT` message.
pub fn holder_details(
    reservations: &[Reservation],
    events: &[EventRecord],
) -> HashMap<String, HolderDetail> {
    reservations
        .iter()
        .filter(|reservation| reservation.state == ReservationState::Granted)
        .map(|reservation| {
            let agent = owner_of(&reservation.task_id, events).map(|tool| tool.to_string());
            let detail = HolderDetail {
                agent,
                since: Some(reservation.reserved_at),
            };
            (reservation.path.clone(), detail)
        })
        .collect()
}

/// How many tasks are queued behind a held file, summed over all paths.
fn waiting_total(rows: &[ReservedRowModel]) -> usize {
    rows.iter().map(|row| row.waiting.len()).sum()
}

/// `2 files held, 1 waiting`. The word `file` goes singular for one.
pub fn reserved_summary(held: usize, waiting: usize) -> String {
    let files = if held == 1 { "file" } else { "files" };
    format!("{held} {files} held, {waiting} waiting")
}

#[component]
pub fn ReservedPanel() -> impl IntoView {
    let ui = use_ui();
    keep_live(ui, move || refresh(ui));
    let rows = move || ui.snapshot.with(|s| group_reservations(&s.reservations));
    let held = move || rows().iter().filter(|row| row.holder.is_some()).count();
    let waiting = move || waiting_total(&rows());
    view! {
        <div class="sub" style="margin-bottom:.6em">{move || reserved_summary(held(), waiting())}</div>
        <div class="note" style="margin-bottom:.8em">
            <Icon name="info" size=14 />
            <span>"A file has one writer at a time. Agents can only write files their task has reserved. Anything else is blocked and reverted."</span>
        </div>
        <div class="tree">
            {move || {
                let list = rows();
                if list.is_empty() {
                    view! { <div class="note">"No files are reserved. Each task reserves the files it may write, and nothing else writes them."</div> }.into_any()
                } else {
                    let details = ui.snapshot.with(|s| holder_details(&s.reservations, &s.events));
                    list.into_iter()
                        .map(|row| {
                            let detail = details.get(&row.path).cloned();
                            view! { <ReservedRow row=row detail=detail /> }
                        })
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

    fn goes_to(task: &str, agent: &str) -> EventRecord {
        EventRecord {
            chat: None,
            id: 1,
            at: OffsetDateTime::UNIX_EPOCH,
            event: sentinel_core::Event::Message {
                from: "sentinel".to_owned(),
                to: Some(sentinel_core::ToolId::new(agent)),
                text: format!("{task} goes to Agent (easy task)."),
            },
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

    #[test]
    fn the_summary_counts_held_files_and_waiting_tasks() {
        assert_eq!(reserved_summary(2, 1), "2 files held, 1 waiting");
        assert_eq!(reserved_summary(1, 0), "1 file held, 0 waiting");
        assert_eq!(reserved_summary(0, 0), "0 files held, 0 waiting");
    }

    #[test]
    fn a_holder_detail_names_the_agent_and_the_grant_time() {
        let details = holder_details(
            &[reservation(
                1,
                "T-01",
                "src/a.rs",
                ReservationState::Granted,
            )],
            &[goes_to("T-01", "claude")],
        );
        let detail = &details["src/a.rs"];
        assert_eq!(detail.agent.as_deref(), Some("claude"));
        assert_eq!(detail.since, Some(OffsetDateTime::UNIX_EPOCH));
    }

    #[test]
    fn waiting_tasks_have_no_holder_detail() {
        let details = holder_details(
            &[reservation(
                1,
                "T-02",
                "src/b.rs",
                ReservationState::Waiting,
            )],
            &[],
        );
        assert!(!details.contains_key("src/b.rs"));
    }
}
