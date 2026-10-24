//! Care schedule: when a care run is due. The caller passes today's date, so tests use a fake clock.

use std::fmt;

/// A calendar date in the proleptic Gregorian calendar. Field order makes `Ord` chronological.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl Date {
    /// A valid date, or `None` when the month or day is out of range.
    pub fn new(year: i32, month: u8, day: u8) -> Option<Date> {
        let valid = (1..=12).contains(&month) && (1..=days_in_month(year, month)).contains(&day);
        valid.then_some(Date { year, month, day })
    }

    /// Whole days since 1970-01-01. Negative before that date.
    pub fn day_number(self) -> i64 {
        let year = i64::from(self.year) - i64::from(self.month <= 2);
        let era = year.div_euclid(400);
        let year_of_era = year - era * 400;
        let shifted_month = if self.month > 2 {
            i64::from(self.month) - 3
        } else {
            i64::from(self.month) + 9
        };
        let day_of_year = (153 * shifted_month + 2) / 5 + i64::from(self.day) - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days from `from` to `to`. Negative when `to` is earlier.
pub fn days_between(from: Date, to: Date) -> i64 {
    to.day_number() - from.day_number()
}

/// How often care runs. `Off` never runs on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frequency {
    Off,
    Daily,
    Weekly,
    Monthly,
    /// Every N runs. `AfterRuns(0)` never comes due.
    AfterRuns(u32),
}

/// True when a care run should start now. A missed run is due once, so the caller starts one run.
pub fn is_due(freq: Frequency, last_care: Option<Date>, today: Date, runs_since: u32) -> bool {
    match freq {
        Frequency::Off => false,
        Frequency::AfterRuns(every) => every > 0 && runs_since >= every,
        Frequency::Daily => days_passed(last_care, today, 1),
        Frequency::Weekly => days_passed(last_care, today, 7),
        Frequency::Monthly => month_passed(last_care, today),
    }
}

fn days_passed(last: Option<Date>, today: Date, days: i64) -> bool {
    last.is_none_or(|last| days_between(last, today) >= days)
}

fn month_passed(last: Option<Date>, today: Date) -> bool {
    last.is_none_or(|last| (today.year, today.month) > (last.year, last.month))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::new(year, month, day).expect("test date is valid")
    }

    #[test]
    fn date_rejects_impossible_days() {
        assert!(Date::new(2026, 2, 29).is_none());
        assert!(Date::new(2028, 2, 29).is_some());
        assert!(Date::new(2026, 13, 1).is_none());
    }

    #[test]
    fn day_numbers_count_real_days() {
        assert_eq!(date(1970, 1, 1).day_number(), 0);
        assert_eq!(days_between(date(2026, 2, 28), date(2026, 3, 1)), 1);
        assert_eq!(days_between(date(2024, 2, 28), date(2024, 3, 1)), 2);
    }

    #[test]
    fn off_is_never_due() {
        let today = date(2026, 10, 24);
        assert!(!is_due(Frequency::Off, None, today, 99));
    }

    #[test]
    fn daily_is_due_after_a_day_not_the_same_day() {
        let last = date(2026, 10, 23);
        assert!(!is_due(Frequency::Daily, Some(last), date(2026, 10, 23), 0));
        assert!(is_due(Frequency::Daily, Some(last), date(2026, 10, 24), 0));
    }

    #[test]
    fn missed_weekly_run_is_due_once_after_long_gap() {
        let last = date(2026, 1, 1);
        let today = date(2026, 10, 24);
        assert!(is_due(Frequency::Weekly, Some(last), today, 0));
    }

    #[test]
    fn weekly_waits_seven_days() {
        let last = date(2026, 10, 17);
        assert!(!is_due(
            Frequency::Weekly,
            Some(last),
            date(2026, 10, 23),
            0
        ));
        assert!(is_due(Frequency::Weekly, Some(last), date(2026, 10, 24), 0));
    }

    #[test]
    fn monthly_is_due_in_a_new_month() {
        let last = date(2026, 9, 30);
        assert!(!is_due(
            Frequency::Monthly,
            Some(last),
            date(2026, 9, 30),
            0
        ));
        assert!(is_due(Frequency::Monthly, Some(last), date(2026, 10, 1), 0));
    }

    #[test]
    fn never_run_schedule_is_due_now() {
        assert!(is_due(Frequency::Daily, None, date(2026, 10, 24), 0));
    }

    #[test]
    fn after_runs_counts_runs_since_last_care() {
        let today = date(2026, 10, 24);
        assert!(!is_due(Frequency::AfterRuns(5), None, today, 4));
        assert!(is_due(Frequency::AfterRuns(5), None, today, 5));
        assert!(!is_due(Frequency::AfterRuns(0), None, today, 50));
    }

    #[test]
    fn date_displays_as_iso_day() {
        assert_eq!(date(2026, 3, 7).to_string(), "2026-03-07");
    }
}
