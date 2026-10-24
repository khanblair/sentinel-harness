//! Quiet hours hold ordinary messages. Approvals and budget alerts always pass.

const MINUTES_PER_DAY: u16 = 1440;

/// Minutes after midnight. The window may wrap past midnight. A window whose start equals its
/// end holds nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuietHours {
    pub start_minute: u16,
    pub end_minute: u16,
}

/// Urgent messages ignore quiet hours. Normal messages wait until the window ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    Normal,
    Urgent,
}

impl QuietHours {
    fn contains(self, minute_of_day: u16) -> bool {
        let minute = minute_of_day % MINUTES_PER_DAY;
        if self.start_minute < self.end_minute {
            return minute >= self.start_minute && minute < self.end_minute;
        }
        if self.start_minute > self.end_minute {
            return minute >= self.start_minute || minute < self.end_minute;
        }
        false
    }
}

pub fn held(quiet: Option<QuietHours>, minute_of_day: u16, priority: Priority) -> bool {
    let Some(quiet) = quiet else {
        return false;
    };
    priority == Priority::Normal && quiet.contains(minute_of_day)
}

#[cfg(test)]
mod tests {
    use super::{Priority, QuietHours, held};

    const NIGHT: Option<QuietHours> = Some(QuietHours {
        start_minute: 22 * 60,
        end_minute: 7 * 60,
    });

    #[test]
    fn a_normal_message_is_held_inside_a_window_that_wraps_midnight() {
        assert!(held(NIGHT, 23 * 60, Priority::Normal));
        assert!(held(NIGHT, 2 * 60, Priority::Normal));
    }

    #[test]
    fn a_normal_message_passes_outside_the_window() {
        assert!(!held(NIGHT, 12 * 60, Priority::Normal));
        assert!(!held(NIGHT, 7 * 60, Priority::Normal));
    }

    #[test]
    fn an_urgent_message_is_never_held() {
        assert!(!held(NIGHT, 23 * 60, Priority::Urgent));
    }

    #[test]
    fn no_quiet_hours_holds_nothing() {
        assert!(!held(None, 2 * 60, Priority::Normal));
    }

    #[test]
    fn a_window_with_equal_start_and_end_holds_nothing() {
        let same = Some(QuietHours {
            start_minute: 600,
            end_minute: 600,
        });
        assert!(!held(same, 600, Priority::Normal));
    }
}
