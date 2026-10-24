//! Routines in the UI: the calls that load and change them, the brief feed, and the pure logic
//! the Settings page and the sidebar show (`docs/product-scope.md`, F25).

use leptos::prelude::Set;
use leptos::task::spawn_local;
use sentinel_core::routines::{
    Brief, EVERY_DAY, EditRoutine, NewRoutine, Routine, RoutineKind, WEEKDAYS,
};
use sentinel_core::{Event, EventRecord, RoutineId};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;

use crate::api::{self, ApiFailure};
use crate::state::{ToastKind, Ui};

/// Saturday and Sunday, in the same bit order as `WEEKDAYS`.
pub const WEEKENDS: u8 = 0b110_0000;
/// The briefs the page asks for.
pub const BRIEFS_SHOWN: u32 = 5;
/// The longest text a reminder may hold.
pub const TEXT_LIMIT: usize = 500;
/// The days choices a select offers. A custom mask is added only when a routine has one.
const DAY_CHOICES: [&str; 3] = ["Every day", "Weekdays", "Weekends"];
/// The first and last hour the time choices offer, on the hour.
const FIRST_HOUR: u16 = 6;
const LAST_HOUR: u16 = 21;

/// The label of a days mask, as the days select shows it.
pub fn days_label(days: u8) -> &'static str {
    match days {
        EVERY_DAY => "Every day",
        WEEKDAYS => "Weekdays",
        WEEKENDS => "Weekends",
        _ => "Custom",
    }
}

/// The days mask for a select label. Unknown labels, such as `Custom`, give every day.
pub fn days_from_label(label: &str) -> u8 {
    match label {
        "Weekdays" => WEEKDAYS,
        "Weekends" => WEEKENDS,
        _ => EVERY_DAY,
    }
}

/// The labels the days select offers for a routine. A custom mask keeps its own label in the list.
pub fn day_choices(current: u8) -> Vec<&'static str> {
    let mut choices = DAY_CHOICES.to_vec();
    if days_label(current) == "Custom" {
        choices.push("Custom");
    }
    choices
}

/// The times the time select offers: each hour from 06:00 to 21:00, plus the current time, sorted.
pub fn time_choices(current: u16) -> Vec<u16> {
    let mut choices: Vec<u16> = (FIRST_HOUR..=LAST_HOUR).map(|hour| hour * 60).collect();
    choices.push(current);
    choices.sort_unstable();
    choices.dedup();
    choices
}

/// The sentence that says what a routine of this kind does.
pub fn what_it_does(kind: RoutineKind) -> &'static str {
    match kind {
        RoutineKind::Morning => "What is waiting today, what you have, and what to start with.",
        RoutineKind::Sundown => {
            "What got done, what is stuck and why, and what waits for tomorrow."
        }
        RoutineKind::Reminder => "Your own text.",
        RoutineKind::BacklogRun => "Starts a run of a backlog file.",
    }
}

/// The line under a routine's name: what it does, then its own text when it has some.
pub fn about_text(routine: &Routine) -> String {
    let what = what_it_does(routine.kind);
    if routine.text.is_empty() {
        what.to_owned()
    } else {
        format!("{what} {}", routine.text)
    }
}

/// The last-run line. A routine that never fired says so.
pub fn last_text(last_fired_on: Option<&str>) -> String {
    match last_fired_on {
        Some(date) => format!("Last: {date}"),
        None => "Last: never".to_owned(),
    }
}

/// How many routines are on. The sidebar shows this number.
pub fn enabled_count(routines: &[Routine]) -> usize {
    routines.iter().filter(|routine| routine.enabled).count()
}

/// A brief's time for the page, in UTC. A time that does not parse shows as it came.
pub fn brief_time_text(created_at: &str) -> String {
    let format = format_description!("[year]-[month]-[day] [hour]:[minute] UTC");
    match OffsetDateTime::parse(created_at, &Rfc3339) {
        Ok(at) => at
            .to_offset(time::UtcOffset::UTC)
            .format(&format)
            .unwrap_or_else(|_| created_at.to_owned()),
        Err(_) => created_at.to_owned(),
    }
}

/// The text of a failed call. The daemon's own sentence comes first, without the status code.
pub fn failure_text(failure: &ApiFailure) -> String {
    match failure {
        ApiFailure::Status { message, .. } => message.clone(),
        ApiFailure::Network(_) | ApiFailure::Decode(_) => failure.to_string(),
    }
}

/// A brief that was just built, for the toast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefNotice {
    pub event_id: i64,
    pub title: String,
}

/// The newest `BriefReady` in the events, if there is one.
pub fn latest_brief_ready(events: &[EventRecord]) -> Option<BriefNotice> {
    events.iter().rev().find_map(|record| {
        let Event::BriefReady { title, .. } = &record.event else {
            return None;
        };
        Some(BriefNotice {
            event_id: record.id,
            title: title.clone(),
        })
    })
}

fn routine_path(id: &RoutineId) -> String {
    format!("/api/routines/{}", id.as_str())
}

/// Puts the routine list in `ui.routines`. A failure shows as a toast.
pub async fn load_routines(ui: Ui) {
    match api::get::<Vec<Routine>>("/api/routines").await {
        Ok(list) => ui.routines.set(list),
        Err(failure) => ui.show_toast(failure_text(&failure), ToastKind::Danger),
    }
}

/// Puts the latest briefs in `ui.briefs`. A failure shows as a toast.
pub async fn load_briefs(ui: Ui) {
    let path = format!("/api/briefs?limit={BRIEFS_SHOWN}");
    match api::get::<Vec<Brief>>(&path).await {
        Ok(list) => ui.briefs.set(list),
        Err(failure) => ui.show_toast(failure_text(&failure), ToastKind::Danger),
    }
}

/// Loads both lists. The Routines page runs this when it opens.
pub async fn load_page(ui: Ui) {
    load_routines(ui).await;
    load_briefs(ui).await;
}

/// Reloads the routine list after a change, or shows the daemon's sentence. Returns whether the
/// change worked.
async fn settle<T>(ui: Ui, result: Result<T, ApiFailure>) -> bool {
    match result {
        Ok(_) => {
            load_routines(ui).await;
            true
        }
        Err(failure) => {
            ui.show_toast(failure_text(&failure), ToastKind::Danger);
            false
        }
    }
}

/// Adds a reminder. Returns whether it was saved.
pub async fn create(ui: Ui, new: &NewRoutine) -> bool {
    settle(ui, api::post::<_, Routine>("/api/routines", new).await).await
}

/// Changes the fields a routine sends. Returns whether it was saved.
pub async fn edit(ui: Ui, id: &RoutineId, change: &EditRoutine) -> bool {
    settle(
        ui,
        api::send::<_, Routine>("PUT", &routine_path(id), change).await,
    )
    .await
}

/// Deletes a reminder or a backlog run. Returns whether it was deleted.
pub async fn remove(ui: Ui, id: &RoutineId) -> bool {
    let empty = serde_json::json!({});
    let result = api::send::<_, serde_json::Value>("DELETE", &routine_path(id), &empty).await;
    settle(ui, result).await
}

/// Runs a routine now. The brief it builds shows in the list after this returns.
pub async fn send_now(ui: Ui, id: &RoutineId) -> bool {
    let path = format!("{}/run", routine_path(id));
    let sent = settle(ui, api::post_empty::<serde_json::Value>(&path).await).await;
    if sent {
        load_briefs(ui).await;
    }
    sent
}

/// Runs `send_now` from a click handler.
pub fn send_now_in_background(ui: Ui, id: RoutineId) {
    spawn_local(async move {
        send_now(ui, &id).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::ChatId;

    fn routine(name: &str, enabled: bool) -> Routine {
        Routine {
            id: RoutineId::new(name),
            name: name.to_owned(),
            kind: RoutineKind::Reminder,
            minute_of_day: 540,
            days: WEEKDAYS,
            enabled,
            to_telegram: false,
            text: String::new(),
            backlog: String::new(),
            last_fired_on: None,
        }
    }

    fn record(id: i64, event: Event) -> EventRecord {
        EventRecord {
            id,
            chat: None::<ChatId>,
            at: OffsetDateTime::UNIX_EPOCH,
            event,
        }
    }

    #[test]
    fn a_days_mask_has_its_label_and_back() {
        assert_eq!(days_label(EVERY_DAY), "Every day");
        assert_eq!(days_label(WEEKDAYS), "Weekdays");
        assert_eq!(days_label(WEEKENDS), "Weekends");
        assert_eq!(days_from_label("Weekdays"), WEEKDAYS);
        assert_eq!(days_from_label("Weekends"), WEEKENDS);
        assert_eq!(days_from_label("Every day"), EVERY_DAY);
    }

    #[test]
    fn a_custom_mask_is_named_custom_and_offered_once() {
        let custom = 0b000_0101;
        assert_eq!(days_label(custom), "Custom");
        assert_eq!(day_choices(custom).last(), Some(&"Custom"));
        assert_eq!(day_choices(WEEKDAYS).len(), 3);
    }

    #[test]
    fn time_choices_are_hours_from_six_to_nine_plus_the_current_time() {
        let choices = time_choices(540);
        assert_eq!(choices.first(), Some(&360));
        assert_eq!(choices.last(), Some(&1260));
        assert!(choices.contains(&540));
        assert_eq!(choices.len(), 16);
    }

    #[test]
    fn time_choices_keep_an_odd_time_once_and_in_order() {
        let choices = time_choices(555);
        let position = choices.iter().position(|minute| *minute == 555);
        assert!(position.is_some());
        assert!(choices.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn the_last_run_line_says_never_or_the_date() {
        assert_eq!(last_text(None), "Last: never");
        assert_eq!(last_text(Some("2026-10-25")), "Last: 2026-10-25");
    }

    #[test]
    fn only_enabled_routines_are_counted() {
        let list = [routine("a", true), routine("b", false), routine("c", true)];
        assert_eq!(enabled_count(&list), 2);
        assert_eq!(enabled_count(&[]), 0);
    }

    #[test]
    fn a_reminder_says_its_text_after_its_kind_sentence() {
        let mut reminder = routine("Stand up", true);
        reminder.text = "Review the plan.".to_owned();
        assert_eq!(about_text(&reminder), "Your own text. Review the plan.");
    }

    #[test]
    fn a_brief_time_shows_utc_to_the_minute() {
        assert_eq!(
            brief_time_text("2026-10-25T05:00:00Z"),
            "2026-10-25 05:00 UTC"
        );
        assert_eq!(brief_time_text("not a time"), "not a time");
    }

    #[test]
    fn a_daemon_sentence_is_shown_without_the_status() {
        let failure = ApiFailure::Status {
            code: 400,
            message: "Name is empty.".to_owned(),
        };
        assert_eq!(failure_text(&failure), "Name is empty.");
    }

    #[test]
    fn the_newest_brief_ready_is_the_notice() {
        let events = [
            record(
                1,
                Event::BriefReady {
                    routine_kind: RoutineKind::Morning,
                    title: "Morning brief".to_owned(),
                    text: "old".to_owned(),
                },
            ),
            record(2, Event::HostHeartbeat),
            record(
                3,
                Event::BriefReady {
                    routine_kind: RoutineKind::Sundown,
                    title: "Sundown wrap-up".to_owned(),
                    text: "new".to_owned(),
                },
            ),
        ];
        assert_eq!(
            latest_brief_ready(&events),
            Some(BriefNotice {
                event_id: 3,
                title: "Sundown wrap-up".to_owned(),
            })
        );
        assert_eq!(latest_brief_ready(&events[1..2]), None);
    }
}
