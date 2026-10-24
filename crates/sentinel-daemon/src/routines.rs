//! Routines: jobs that run at a set local time. The engine checks who asks and what the routine
//! says, then stores it. The scheduler in `routines_tick.rs` fires them.

use sentinel_core::RoutineId;
use sentinel_core::routines::{EVERY_DAY, EditRoutine, NewRoutine, Routine, RoutineKind, WEEKDAYS};
use uuid::Uuid;

use crate::brain::require_pc;
use crate::controls::Caller;
use crate::engine::{Engine, EngineError, Result};
use crate::run_source::backlog_path;

const NAME_LIMIT: usize = 60;
const TEXT_LIMIT: usize = 500;
const MINUTES_PER_DAY: u16 = 1440;
const MORNING_MINUTE: u16 = 8 * 60;
const SUNDOWN_MINUTE: u16 = 18 * 60;
const MORNING_ID: &str = "morning";
const SUNDOWN_ID: &str = "sundown";
/// Made routines get `r-` and a random part, so an id is never reused.
const ID_PREFIX: &str = "r-";

impl Engine {
    /// Every routine. The two built-in ones are added the first time this runs, turned off.
    pub fn list_routines(&self) -> Result<Vec<Routine>> {
        self.seed_built_ins()?;
        Ok(self.store().list_routines()?)
    }

    /// Makes a routine that is on. Only the computer may make one.
    pub fn create_routine(&self, new: &NewRoutine, caller: &Caller) -> Result<Routine> {
        require_pc(caller)?;
        let routine = Routine {
            id: RoutineId::new(format!("{ID_PREFIX}{}", Uuid::new_v4().simple())),
            name: new.name.trim().to_owned(),
            kind: new.kind,
            minute_of_day: new.minute_of_day,
            days: new.days,
            enabled: true,
            to_telegram: new.to_telegram,
            text: new.text.trim().to_owned(),
            backlog: new.backlog.trim().to_owned(),
            last_fired_on: None,
        };
        validate(&routine)?;
        self.store().insert_routine(&routine, self.now())?;
        Ok(routine)
    }

    /// Changes a routine. A phone may only turn one off. The computer may change any field.
    pub fn edit_routine(
        &self,
        id: &RoutineId,
        edit: EditRoutine,
        caller: &Caller,
    ) -> Result<Routine> {
        if !caller.from_pc && edit != turn_off() {
            return Err(EngineError::Refused(
                "A phone can only turn a routine off. Change the rest at the computer.".to_owned(),
            ));
        }
        let routine = merged(self.find_routine(id)?, edit);
        validate(&routine)?;
        self.store().update_routine(&routine)?;
        Ok(routine)
    }

    /// Deletes a routine the computer made. The two built-in ones can only be turned off.
    pub fn delete_routine(&self, id: &RoutineId, caller: &Caller) -> Result<()> {
        require_pc(caller)?;
        if is_built_in(id) {
            return Err(EngineError::Refused(
                "The two built-in routines can only be turned off.".to_owned(),
            ));
        }
        self.find_routine(id)?;
        self.store().delete_routine(id)?;
        Ok(())
    }

    /// Builds and delivers a routine's brief now. A phone may press this. It does not change the
    /// day the routine last fired, so the scheduled run still happens.
    pub async fn send_routine_now(&self, id: &RoutineId, caller: &Caller) -> Result<()> {
        let routine = self.find_routine(id)?;
        self.fire(&routine, caller).await
    }

    fn find_routine(&self, id: &RoutineId) -> Result<Routine> {
        self.seed_built_ins()?;
        self.store()
            .get_routine(id)?
            .ok_or_else(|| EngineError::NotFound(format!("routine {}", id.as_str())))
    }

    fn seed_built_ins(&self) -> Result<()> {
        let built_ins = [
            built_in(MORNING_ID, RoutineKind::Morning, MORNING_MINUTE, WEEKDAYS),
            built_in(SUNDOWN_ID, RoutineKind::Sundown, SUNDOWN_MINUTE, EVERY_DAY),
        ];
        self.store().seed_routines(&built_ins, self.now())?;
        Ok(())
    }
}

/// The rules every routine keeps. Checked on create, and again on the merged routine after an edit.
fn validate(routine: &Routine) -> Result<()> {
    let name_length = routine.name.chars().count();
    if !(1..=NAME_LIMIT).contains(&name_length) {
        return Err(invalid(&format!("A name is 1 to {NAME_LIMIT} characters.")));
    }
    if routine.minute_of_day >= MINUTES_PER_DAY {
        return Err(invalid("A time is between 00:00 and 23:59."));
    }
    if routine.days == 0 || (routine.days & !EVERY_DAY) != 0 {
        return Err(invalid("Pick at least one day."));
    }
    if routine.text.chars().count() > TEXT_LIMIT {
        return Err(invalid(&format!("Text is up to {TEXT_LIMIT} characters.")));
    }
    if routine.kind == RoutineKind::Reminder && routine.text.is_empty() {
        return Err(invalid("A reminder needs text."));
    }
    if !routine.backlog.is_empty() {
        backlog_path(&routine.backlog)?;
    }
    Ok(())
}

fn invalid(text: &str) -> EngineError {
    EngineError::Invalid(text.to_owned())
}

/// What a phone may send: turn the routine off, and nothing else.
fn turn_off() -> EditRoutine {
    EditRoutine {
        enabled: Some(false),
        ..EditRoutine::default()
    }
}

fn merged(mut routine: Routine, edit: EditRoutine) -> Routine {
    if let Some(name) = edit.name {
        routine.name = String::from(name.trim());
    }
    if let Some(minute) = edit.minute_of_day {
        routine.minute_of_day = minute;
    }
    if let Some(days) = edit.days {
        routine.days = days;
    }
    if let Some(enabled) = edit.enabled {
        routine.enabled = enabled;
    }
    if let Some(to_telegram) = edit.to_telegram {
        routine.to_telegram = to_telegram;
    }
    if let Some(text) = edit.text {
        routine.text = String::from(text.trim());
    }
    if let Some(backlog) = edit.backlog {
        routine.backlog = String::from(backlog.trim());
    }
    routine
}

/// The two routines every install starts with. They are off until the owner turns them on.
fn built_in(id: &str, kind: RoutineKind, minute_of_day: u16, days: u8) -> Routine {
    Routine {
        id: RoutineId::new(id),
        name: kind.title().to_owned(),
        kind,
        minute_of_day,
        days,
        enabled: false,
        to_telegram: true,
        text: String::new(),
        backlog: String::new(),
        last_fired_on: None,
    }
}

fn is_built_in(id: &RoutineId) -> bool {
    matches!(id.as_str(), MORNING_ID | SUNDOWN_ID)
}
