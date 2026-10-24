//! The reservation table: which task holds which path, and who waits for whom.
//!
//! The store is the state of record. Every state change goes through the store,
//! and `load` rebuilds the in-memory view from the Granted and Waiting rows.

use std::collections::BTreeSet;

use sentinel_core::{Reservation, ReservationId, ReservationState, Scope, TaskId};
use sentinel_store::Store;
use time::OffsetDateTime;

use crate::Result;
use crate::claims::{Claim, entry_scope};

/// The outcome of asking for paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grant {
    Granted(Vec<Reservation>),
    Waiting { blocked_by: Vec<TaskId> },
}

pub struct ReservationTable {
    pub(crate) store: Store,
    pub(crate) held: Vec<Reservation>,
    pub(crate) waiting: Vec<Reservation>,
}

impl ReservationTable {
    /// Rebuild the table from the rows in state Granted and Waiting.
    pub fn load(store: Store) -> Result<ReservationTable> {
        let rows =
            store.list_reservations(&[ReservationState::Granted, ReservationState::Waiting])?;
        let mut held = Vec::new();
        let mut waiting = Vec::new();
        for row in rows {
            entry_scope(&row.path)?;
            if row.state == ReservationState::Granted {
                held.push(row);
            } else {
                waiting.push(row);
            }
        }
        Ok(ReservationTable {
            store,
            held,
            waiting,
        })
    }

    /// Claim every entry of `scope`, or claim nothing and wait for the holders.
    pub fn try_reserve(
        &mut self,
        task: &TaskId,
        scope: &Scope,
        now: OffsetDateTime,
    ) -> Result<Grant> {
        let claims: Vec<Claim<'_>> = scope.entries().map(|path| Claim::new(task, path)).collect();
        let mut blocked_by = BTreeSet::new();
        for claim in &claims {
            // Entries the task already holds stay, and never count against it.
            if !claim.claimed_in(&self.held) {
                blocked_by.extend(claim.conflicting_holders(&self.held)?);
            }
        }
        if blocked_by.is_empty() {
            let rows = self.grant_entries(&claims, now)?;
            return Ok(Grant::Granted(rows));
        }
        self.remember_waiting(&claims, now)?;
        Ok(Grant::Waiting {
            blocked_by: blocked_by.into_iter().collect(),
        })
    }

    /// Claim one extra path and add it to what the task already holds.
    pub fn request_more(
        &mut self,
        task: &TaskId,
        path: &str,
        now: OffsetDateTime,
    ) -> Result<Grant> {
        self.try_reserve(task, &entry_scope(path)?, now)
    }

    /// Granted rows of `task` move to Released. Returns how many rows moved.
    pub fn release_task(&mut self, task: &TaskId, now: OffsetDateTime) -> Result<usize> {
        self.finish(task, ReservationState::Released, now)
    }

    /// Granted rows of `task` move to Revoked. Returns how many rows moved.
    pub fn revoke_task(&mut self, task: &TaskId, now: OffsetDateTime) -> Result<usize> {
        self.finish(task, ReservationState::Revoked, now)
    }

    /// Waiting rows of `task` move to Abandoned. Returns how many rows moved.
    pub fn abandon_waiting(&mut self, task: &TaskId, now: OffsetDateTime) -> Result<usize> {
        self.finish(task, ReservationState::Abandoned, now)
    }

    /// One Waiting row per requested entry, reusing rows from an earlier try.
    fn remember_waiting(&mut self, claims: &[Claim<'_>], now: OffsetDateTime) -> Result<()> {
        for claim in claims {
            let reused = claim.claimed_in(&self.waiting) || claim.claimed_in(&self.held);
            if !reused {
                let row = self.insert_row(claim, ReservationState::Waiting, now)?;
                self.waiting.push(row);
            }
        }
        Ok(())
    }

    fn grant_entries(
        &mut self,
        claims: &[Claim<'_>],
        now: OffsetDateTime,
    ) -> Result<Vec<Reservation>> {
        let mut granted = Vec::with_capacity(claims.len());
        for claim in claims {
            // Entries the task already holds are kept, so no row is duplicated.
            granted.push(match claim.matching_row(&self.held).cloned() {
                Some(row) => row,
                None => self.grant_one(claim, now)?,
            });
        }
        Ok(granted)
    }

    /// A Waiting row moves to Granted; a fresh claim runs through Requested.
    fn grant_one(&mut self, claim: &Claim<'_>, now: OffsetDateTime) -> Result<Reservation> {
        if let Some(index) = self
            .waiting
            .iter()
            .position(|row| &row.task_id == claim.task && row.path == claim.path)
        {
            let mut row = self.waiting.remove(index);
            self.store
                .set_reservation_state(row.id, ReservationState::Granted, now)?;
            row.state = ReservationState::Granted;
            self.held.push(row.clone());
            return Ok(row);
        }
        let row = self.insert_row(claim, ReservationState::Granted, now)?;
        self.held.push(row.clone());
        Ok(row)
    }

    /// Insert as Requested first, then move on, so the store sees every row.
    fn insert_row(
        &self,
        claim: &Claim<'_>,
        next: ReservationState,
        now: OffsetDateTime,
    ) -> Result<Reservation> {
        let row = self.store.insert_reservation(
            claim.task,
            claim.path,
            ReservationState::Requested,
            now,
        )?;
        self.store.set_reservation_state(row.id, next, now)?;
        Ok(Reservation { state: next, ..row })
    }

    /// Move the rows of `task` out of the live view and into `next`.
    fn finish(
        &mut self,
        task: &TaskId,
        next: ReservationState,
        now: OffsetDateTime,
    ) -> Result<usize> {
        let rows = if next == ReservationState::Abandoned {
            &self.waiting
        } else {
            &self.held
        };
        let ids: Vec<ReservationId> = rows
            .iter()
            .filter(|row| &row.task_id == task)
            .map(|row| row.id)
            .collect();
        for id in &ids {
            self.store.set_reservation_state(*id, next, now)?;
        }
        let kept = if next == ReservationState::Abandoned {
            &mut self.waiting
        } else {
            &mut self.held
        };
        kept.retain(|row| &row.task_id != task);
        Ok(ids.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_table() -> ReservationTable {
        ReservationTable::load(Store::open_in_memory().unwrap()).unwrap()
    }

    fn now() -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH
    }

    fn scope(paths: &[&str]) -> Scope {
        Scope::from_paths(paths).unwrap()
    }

    fn assert_granted(grant: &Grant) {
        assert!(
            matches!(grant, Grant::Granted(_)),
            "expected a grant: {grant:?}"
        );
    }

    fn assert_waiting(grant: Grant, blockers: &[TaskId]) {
        match grant {
            Grant::Waiting { blocked_by } => assert_eq!(blocked_by, blockers),
            granted @ Grant::Granted(_) => panic!("expected waiting, got {granted:?}"),
        }
    }

    fn assert_agrees_with_store(table: &ReservationTable) {
        let reloaded = ReservationTable::load(table.store.clone()).unwrap();
        assert_eq!(reloaded.all_held(), table.all_held());
    }

    fn grant(table: &mut ReservationTable, task: &str, paths: &[&str]) {
        let ask = table
            .try_reserve(&TaskId::new(task), &scope(paths), now())
            .unwrap();
        assert_granted(&ask);
        assert_agrees_with_store(table);
    }

    fn wait(table: &mut ReservationTable, task: &str, paths: &[&str], blockers: &[&str]) {
        let ask = table
            .try_reserve(&TaskId::new(task), &scope(paths), now())
            .unwrap();
        let blockers = blockers
            .iter()
            .map(|id| TaskId::new(*id))
            .collect::<Vec<_>>();
        assert_waiting(ask, &blockers);
        assert_agrees_with_store(table);
    }

    #[test]
    fn a_second_task_waits_while_the_holder_keeps_the_path() {
        let mut table = fresh_table();
        grant(&mut table, "A", &["src/a.rs"]);
        wait(&mut table, "B", &["src/a.rs"], &["A"]);
        assert_eq!(table.holder_of("src/a.rs"), Some(TaskId::new("A")));
        assert_eq!(table.blockers_of(&TaskId::new("B")), vec![TaskId::new("A")]);
    }

    #[test]
    fn disjoint_scopes_are_both_granted() {
        let mut table = fresh_table();
        grant(&mut table, "A", &["src/a.rs"]);
        grant(&mut table, "B", &["src/b.rs"]);
        assert_eq!(table.holder_of("src/a.rs"), Some(TaskId::new("A")));
        assert_eq!(table.holder_of("src/b.rs"), Some(TaskId::new("B")));
        assert_eq!(table.all_held().len(), 2);
    }

    #[test]
    fn a_folder_entry_blocks_a_file_below_it() {
        let mut table = fresh_table();
        grant(&mut table, "A", &["src/limits/"]);
        wait(&mut table, "B", &["src/limits/daily.rs"], &["A"]);
        assert!(!table.may_write(&TaskId::new("B"), "src/limits/daily.rs"));
    }

    #[test]
    fn a_partly_conflicting_request_grants_nothing() {
        let mut table = fresh_table();
        grant(&mut table, "A", &["src/a.rs"]);
        wait(&mut table, "B", &["src/free.rs", "src/a.rs"], &["A"]);
        assert!(!table.may_write(&TaskId::new("B"), "src/free.rs"));
        assert_eq!(table.holder_of("src/free.rs"), None);
        let rows = table.store.reservations_of_task(&TaskId::new("B")).unwrap();
        assert_eq!(rows.len(), 2, "one Waiting row per requested entry");
    }

    #[test]
    fn release_lets_the_waiter_take_the_path_on_the_next_try() {
        let mut table = fresh_table();
        grant(&mut table, "A", &["src/a.rs"]);
        wait(&mut table, "B", &["src/a.rs"], &["A"]);
        assert_eq!(table.release_task(&TaskId::new("A"), now()).unwrap(), 1);
        let ask = table.try_reserve(&TaskId::new("B"), &scope(&["src/a.rs"]), now());
        assert_granted(&ask.unwrap());
        assert_eq!(table.holder_of("src/a.rs"), Some(TaskId::new("B")));
        let rows = table.store.reservations_of_task(&TaskId::new("B")).unwrap();
        assert_eq!(rows[0].state, ReservationState::Granted);
    }

    #[test]
    fn revoke_marks_rows_revoked_and_frees_the_paths() {
        let a = TaskId::new("A");
        let mut table = fresh_table();
        grant(&mut table, "A", &["src/a.rs", "src/b.rs"]);
        assert_eq!(table.revoke_task(&a, now()).unwrap(), 2);
        assert_eq!(table.all_held(), Vec::new());
        assert_eq!(table.holder_of("src/a.rs"), None);
        let rows = table.store.reservations_of_task(&a).unwrap();
        assert!(
            rows.iter()
                .all(|row| row.state == ReservationState::Revoked)
        );
    }

    #[test]
    fn request_more_grants_a_free_path_and_waits_for_a_held_path() {
        let a = TaskId::new("A");
        let b = TaskId::new("B");
        let mut table = fresh_table();
        grant(&mut table, "A", &["src/a.rs"]);
        grant(&mut table, "B", &["src/b.rs"]);
        assert_granted(&table.request_more(&b, "src/c.rs", now()).unwrap());
        assert!(table.may_write(&b, "src/c.rs"));
        assert_waiting(table.request_more(&b, "src/a.rs", now()).unwrap(), &[a]);
        assert!(!table.may_write(&b, "src/a.rs"));
    }

    #[test]
    fn may_write_covers_only_the_task_entries() {
        let a = TaskId::new("A");
        let mut table = fresh_table();
        grant(&mut table, "A", &["src/a.rs"]);
        assert!(table.may_write(&a, "src/a.rs"));
        assert!(!table.may_write(&a, "src/b.rs"));
        assert!(!table.may_write(&TaskId::new("B"), "src/a.rs"));
    }

    #[test]
    fn abandon_waiting_moves_only_the_waiting_rows() {
        let b = TaskId::new("B");
        let mut table = fresh_table();
        grant(&mut table, "A", &["src/a.rs"]);
        wait(&mut table, "B", &["src/a.rs"], &["A"]);
        assert_eq!(table.abandon_waiting(&b, now()).unwrap(), 1);
        let rows = table.store.reservations_of_task(&b).unwrap();
        assert!(
            rows.iter()
                .all(|row| row.state == ReservationState::Abandoned)
        );
        assert_eq!(
            table.all_held(),
            vec![(TaskId::new("A"), scope(&["src/a.rs"]))]
        );
    }

    #[test]
    fn re_reserving_own_entries_stays_held_without_duplicate_rows() {
        let a = TaskId::new("A");
        let mut table = fresh_table();
        grant(&mut table, "A", &["src/a.rs"]);
        assert_granted(&table.try_reserve(&a, &scope(&["src/a.rs"]), now()).unwrap());
        assert_eq!(table.store.reservations_of_task(&a).unwrap().len(), 1);
    }

    #[test]
    fn a_reloaded_table_sees_the_same_holders_at_every_step() {
        let a = TaskId::new("A");
        let b = TaskId::new("B");
        let mut table = fresh_table();
        grant(&mut table, "A", &["src/a.rs"]);
        wait(&mut table, "B", &["src/a.rs"], &["A"]);
        table.release_task(&a, now()).unwrap();
        assert_agrees_with_store(&table);
        let ask = table.try_reserve(&b, &scope(&["src/a.rs"]), now());
        assert_granted(&ask.unwrap());
        assert_agrees_with_store(&table);
        assert_eq!(table.holder_of("src/a.rs"), Some(b));
    }

    #[test]
    fn an_invalid_path_returns_invalid() {
        let mut table = fresh_table();
        let ask = table.request_more(&TaskId::new("A"), "../escape.rs", now());
        assert!(matches!(ask, Err(crate::ReserveError::Invalid(_))));
    }
}
