//! Routines: jobs that run at a set time, such as the Morning brief (`docs/product-scope.md`, F25).
//! These are the shapes the daemon sends and the UI reads.

use serde::{Deserialize, Serialize};

use crate::ids::RoutineId;

/// Bit 0 is Monday and bit 6 is Sunday.
pub const WEEKDAYS: u8 = 0b001_1111;
pub const EVERY_DAY: u8 = 0b111_1111;

/// What a routine does when its time comes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutineKind {
    /// Gets you ready for the day.
    Morning,
    /// Tells you how the day went.
    Sundown,
    /// Sends your own text.
    Reminder,
    /// Starts a run of a backlog file.
    BacklogRun,
}

impl RoutineKind {
    /// The title a brief of this kind carries.
    pub fn title(self) -> &'static str {
        match self {
            RoutineKind::Morning => "Morning brief",
            RoutineKind::Sundown => "Sundown wrap-up",
            RoutineKind::Reminder => "Reminder",
            RoutineKind::BacklogRun => "Backlog run",
        }
    }
}

/// A routine as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Routine {
    pub id: RoutineId,
    pub name: String,
    pub kind: RoutineKind,
    /// Minutes after midnight, in this computer's local time.
    pub minute_of_day: u16,
    /// A mask of allowed weekdays, see `WEEKDAYS` and `EVERY_DAY`.
    pub days: u8,
    pub enabled: bool,
    pub to_telegram: bool,
    /// The text of a reminder.
    #[serde(default)]
    pub text: String,
    /// The backlog file of a backlog run. Empty means the configured backlog.
    #[serde(default)]
    pub backlog: String,
    /// The local date it last fired, `YYYY-MM-DD`.
    #[serde(default)]
    pub last_fired_on: Option<String>,
}

/// `POST /api/routines`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewRoutine {
    pub name: String,
    pub kind: RoutineKind,
    pub minute_of_day: u16,
    pub days: u8,
    #[serde(default)]
    pub to_telegram: bool,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub backlog: String,
}

/// `PUT /api/routines/:id`. A missing field stays as it is.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EditRoutine {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub minute_of_day: Option<u16>,
    #[serde(default)]
    pub days: Option<u8>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub to_telegram: Option<bool>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub backlog: Option<String>,
}

/// One brief that was built, as `GET /api/briefs` returns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Brief {
    pub id: i64,
    pub routine: RoutineId,
    pub kind: RoutineKind,
    pub title: String,
    pub body: String,
    /// RFC 3339.
    pub created_at: String,
    pub sent_to_telegram: bool,
}

/// `HH:MM` for minutes after midnight.
pub fn clock_text(minute_of_day: u16) -> String {
    format!("{:02}:{:02}", minute_of_day / 60, minute_of_day % 60)
}

/// Minutes after midnight for `HH:MM`, or `None` for anything else.
pub fn parse_clock(text: &str) -> Option<u16> {
    let (hours, minutes) = text.trim().split_once(':')?;
    let hours: u16 = hours.parse().ok()?;
    let minutes: u16 = minutes.parse().ok()?;
    (hours < 24 && minutes < 60 && text.trim().len() == 5).then_some(hours * 60 + minutes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clock_round_trips() {
        assert_eq!(clock_text(8 * 60 + 5), "08:05");
        assert_eq!(parse_clock("18:00"), Some(1080));
        assert_eq!(parse_clock("24:00"), None);
        assert_eq!(parse_clock("8:00"), None);
    }

    #[test]
    fn weekdays_are_five_bits() {
        assert_eq!(WEEKDAYS.count_ones(), 5);
        assert_eq!(EVERY_DAY.count_ones(), 7);
    }
}
