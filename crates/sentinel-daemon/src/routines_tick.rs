//! The scheduler. Every 30 seconds it finds the routines due in local time, marks each one fired,
//! and runs it. A routine more than two hours late is skipped, so a host that was off all evening
//! does not send a stale morning brief at night.

use std::sync::Arc;
use std::time::Duration;

use sentinel_core::api::RunRequest;
use sentinel_core::routines::{Routine, RoutineKind};
use sentinel_core::{Event, PermissionMode};
use sentinel_store::routines::NewBrief;
use sentinel_telegram::levels::Level;
use time::OffsetDateTime;

use crate::briefs::local_offset;
use crate::controls::Caller;
use crate::engine::{Engine, Result};
use crate::telegram::upgrade;
use crate::telegram_store;

const TICK_EVERY: Duration = Duration::from_secs(30);
const LATE_LIMIT_MINUTES: u16 = 120;
const MISSED_WHY: &str = "Missed by more than 2 hours.";

/// The time as the owner's local clock shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalMoment {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub minute_of_day: u16,
    /// 0 is Monday.
    pub weekday: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DueRoutine {
    /// On time, or late by less than the limit.
    Fire(Routine),
    /// Too late to run. It is skipped, and today counts as used.
    Missed(Routine),
}

/// The routines that are due at `now`. A routine that already fired today is not due again.
pub(crate) fn due(routines: &[Routine], now: &LocalMoment) -> Vec<DueRoutine> {
    routines
        .iter()
        .filter_map(|routine| due_one(routine, now))
        .collect()
}

fn due_one(routine: &Routine, now: &LocalMoment) -> Option<DueRoutine> {
    let late_by = now.minute_of_day.checked_sub(routine.minute_of_day)?;
    let on_its_day = (routine.days & (1 << now.weekday)) != 0;
    let fired_today = routine.last_fired_on.as_deref() == Some(now.date.as_str());
    if !routine.enabled || !on_its_day || fired_today {
        return None;
    }
    if late_by > LATE_LIMIT_MINUTES {
        Some(DueRoutine::Missed(routine.clone()))
    } else {
        Some(DueRoutine::Fire(routine.clone()))
    }
}

/// The local date, minute, and weekday of a UTC moment.
pub(crate) fn local_moment(now: OffsetDateTime) -> LocalMoment {
    let local = now.to_offset(local_offset());
    LocalMoment {
        date: local.date().to_string(),
        minute_of_day: u16::from(local.hour()) * 60 + u16::from(local.minute()),
        weekday: local.weekday().number_days_from_monday(),
    }
}

impl Engine {
    /// Starts the scheduler. It checks at once, then every 30 seconds, and ends with the engine.
    pub fn start_routines(&self) {
        let weak = Arc::downgrade(&self.inner);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(TICK_EVERY);
            loop {
                ticker.tick().await;
                let Some(engine) = upgrade(&weak) else {
                    return;
                };
                engine.run_due_routines().await;
            }
        });
    }

    /// Runs every routine that is due now. One failed routine does not stop the others.
    async fn run_due_routines(&self) {
        let Ok(routines) = self.list_routines() else {
            return;
        };
        let now = local_moment(self.now());
        for due_routine in due(&routines, &now) {
            if let Err(error) = self.run_due(due_routine, &now.date).await {
                eprintln!("sentinel: could not run a routine: {error}");
            }
        }
    }

    /// The day is marked before anything runs, so a crash cannot fire the routine twice.
    async fn run_due(&self, due_routine: DueRoutine, date: &str) -> Result<()> {
        match due_routine {
            DueRoutine::Missed(routine) => {
                self.store().mark_routine_fired(&routine.id, date)?;
                self.skip_routine(&routine, MISSED_WHY)
            }
            DueRoutine::Fire(routine) => {
                self.store().mark_routine_fired(&routine.id, date)?;
                self.fire(&routine, &Caller::phone("routine")).await
            }
        }
    }

    /// Records that a routine fired, then builds its brief or starts its backlog run.
    pub(crate) async fn fire(&self, routine: &Routine, caller: &Caller) -> Result<()> {
        self.emit_in(
            None,
            &Event::RoutineFired {
                routine: routine.id.clone(),
                routine_kind: routine.kind,
            },
        )?;
        match routine.kind {
            RoutineKind::BacklogRun => self.start_backlog_run(routine, caller).await,
            RoutineKind::Morning | RoutineKind::Sundown => {
                let (title, text) = self.build_brief(routine.kind)?;
                self.deliver_brief(routine, &title, &text)
            }
            RoutineKind::Reminder => self.deliver_brief(routine, &routine.name, &routine.text),
        }
    }

    /// Stores the brief. The app gets it at once when the routine asks for Telegram. Telegram gets
    /// it only when the routine asks for it and a paired chat has Reports.
    fn deliver_brief(&self, routine: &Routine, title: &str, text: &str) -> Result<()> {
        let sent = routine.to_telegram && self.telegram_gets_briefs()?;
        self.store().insert_brief(&NewBrief {
            routine: &routine.id,
            kind: routine.kind,
            title,
            body: text,
            created_at: self.now(),
            sent,
        })?;
        if routine.to_telegram {
            self.emit_in(
                None,
                &Event::BriefReady {
                    routine_kind: routine.kind,
                    title: title.to_owned(),
                    text: text.to_owned(),
                },
            )?;
        }
        Ok(())
    }

    fn telegram_gets_briefs(&self) -> Result<bool> {
        if !telegram_store::is_paired(self)? {
            return Ok(false);
        }
        let reports = telegram_store::load(self)?
            .is_some_and(|row| row.settings.levels.contains(&Level::Reports));
        Ok(reports)
    }

    async fn start_backlog_run(&self, routine: &Routine, caller: &Caller) -> Result<()> {
        if let Some(why) = self.backlog_blocker() {
            return self.skip_routine(routine, &why);
        }
        let backlog = (!routine.backlog.is_empty()).then(|| routine.backlog.clone());
        let request = RunRequest {
            backlog,
            ..RunRequest::default()
        };
        match self.start_run_as(request, caller).await {
            Ok(_) => Ok(()),
            Err(error) => self.skip_routine(routine, &error.to_string()),
        }
    }

    /// Why a backlog run cannot start now, or `None` when it can.
    fn backlog_blocker(&self) -> Option<String> {
        if self.config().permission_mode == PermissionMode::Bypass {
            return Some("Permissions are on Bypass, so a backlog run does not start.".to_owned());
        }
        if self.is_run_live() {
            return Some("A run is already live.".to_owned());
        }
        None
    }

    /// Records that a routine did not run, and why. The app and the brief list show it.
    fn skip_routine(&self, routine: &Routine, why: &str) -> Result<()> {
        self.emit_in(
            None,
            &Event::RoutineSkipped {
                routine: routine.id.clone(),
                why: why.to_owned(),
            },
        )?;
        self.store().insert_brief(&NewBrief {
            routine: &routine.id,
            kind: routine.kind,
            title: &routine.name,
            body: why,
            created_at: self.now(),
            sent: false,
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use sentinel_core::RoutineId;
    use sentinel_core::routines::{EVERY_DAY, Routine, RoutineKind, WEEKDAYS};

    use super::{DueRoutine, LocalMoment, due};

    const MORNING: u16 = 8 * 60;
    const MONDAY: u8 = 0;
    const SATURDAY: u8 = 5;

    fn morning(days: u8) -> Routine {
        Routine {
            id: RoutineId::new("morning"),
            name: "Morning brief".to_owned(),
            kind: RoutineKind::Morning,
            minute_of_day: MORNING,
            days,
            enabled: true,
            to_telegram: true,
            text: String::new(),
            backlog: String::new(),
            last_fired_on: None,
        }
    }

    fn at(minute_of_day: u16, weekday: u8) -> LocalMoment {
        LocalMoment {
            date: "2026-10-26".to_owned(),
            minute_of_day,
            weekday,
        }
    }

    #[test]
    fn a_routine_fires_once_a_day() {
        let routine = morning(EVERY_DAY);
        let first = due(std::slice::from_ref(&routine), &at(MORNING + 5, MONDAY));
        assert_eq!(first, vec![DueRoutine::Fire(routine.clone())]);
        let mut fired = routine;
        fired.last_fired_on = Some("2026-10-26".to_owned());
        assert_eq!(due(&[fired], &at(MORNING + 10, MONDAY)), Vec::new());
    }

    #[test]
    fn a_routine_only_fires_on_its_weekdays() {
        let weekdays = [morning(WEEKDAYS)];
        assert_eq!(due(&weekdays, &at(MORNING, SATURDAY)), Vec::new());
        assert_eq!(due(&weekdays, &at(MORNING, MONDAY)).len(), 1);
    }

    #[test]
    fn a_routine_thirty_minutes_late_still_fires() {
        let routine = morning(EVERY_DAY);
        let due_now = due(std::slice::from_ref(&routine), &at(MORNING + 30, MONDAY));
        assert_eq!(due_now, vec![DueRoutine::Fire(routine)]);
    }

    #[test]
    fn a_routine_three_hours_late_is_missed_not_fired() {
        let routine = morning(EVERY_DAY);
        let due_now = due(std::slice::from_ref(&routine), &at(MORNING + 180, MONDAY));
        assert_eq!(due_now, vec![DueRoutine::Missed(routine)]);
    }

    #[test]
    fn a_routine_does_not_fire_before_its_time() {
        assert_eq!(
            due(&[morning(EVERY_DAY)], &at(MORNING - 1, MONDAY)),
            Vec::new()
        );
    }

    #[test]
    fn a_disabled_routine_never_fires() {
        let mut off = morning(EVERY_DAY);
        off.enabled = false;
        assert_eq!(due(&[off], &at(MORNING + 5, MONDAY)), Vec::new());
    }
}
