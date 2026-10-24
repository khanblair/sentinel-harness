//! The always-on host card: what the daemon reports about the host, as check rows and short text,
//! and the two calls the card makes (`GET /api/host` and `POST /api/host/check`).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::{HostStatus, ServiceState, TailscaleStatus};
use sentinel_core::{Event, EventRecord, TailscaleState};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::api;
use crate::state::{ToastKind, Ui};

const MINUTE_SECONDS: u64 = 60;
const HOUR_SECONDS: u64 = 3_600;

/// One line of the host card. `ok` picks the dot or the ring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckRow {
    pub title: &'static str,
    pub detail: String,
    pub ok: bool,
}

/// The card's first check rows: service, Tailscale, agent CLIs, and repository.
/// `now` is passed in so the text is testable and the browser clock is read in one place.
pub fn check_rows(status: &HostStatus, now: OffsetDateTime) -> Vec<CheckRow> {
    vec![
        service_row(status, now),
        tailscale_row(&status.tailscale),
        agents_row(status),
        repository_row(status.repo_path.as_deref()),
    ]
}

fn service_row(status: &HostStatus, now: OffsetDateTime) -> CheckRow {
    let installed = status.service == ServiceState::Installed;
    let detail = if installed {
        format!(
            "Installed and running. {}",
            heartbeat_text(status.last_heartbeat.as_deref(), now)
        )
    } else {
        "Not installed. Run sentinel service install on this machine.".to_owned()
    };
    CheckRow {
        title: "Sentinel service",
        detail,
        ok: installed,
    }
}

fn tailscale_row(status: &TailscaleStatus) -> CheckRow {
    let (ok, detail) = match status.state {
        TailscaleState::Running => (true, connected_text(status)),
        TailscaleState::SignedOut => (false, "Installed, but signed out.".to_owned()),
        TailscaleState::NotInstalled => (false, "Not installed.".to_owned()),
    };
    CheckRow {
        title: "Tailscale",
        detail,
        ok,
    }
}

fn connected_text(status: &TailscaleStatus) -> String {
    match status.machine_name.as_deref().or(status.address.as_deref()) {
        Some(name) => format!("Connected as {name}."),
        None => "Connected.".to_owned(),
    }
}

fn agents_row(status: &HostStatus) -> CheckRow {
    let all_ready = status.agents_total > 0 && status.agents_ready >= status.agents_total;
    CheckRow {
        title: "Agent CLIs",
        detail: format!("{} of {} ready.", status.agents_ready, status.agents_total),
        ok: all_ready,
    }
}

fn repository_row(repo_path: Option<&str>) -> CheckRow {
    match repo_path {
        Some(path) => CheckRow {
            title: "Repository",
            detail: path.to_owned(),
            ok: true,
        },
        None => CheckRow {
            title: "Repository",
            detail: "No folder chosen.".to_owned(),
            ok: false,
        },
    }
}

/// The last heartbeat as text. A missing or unreadable time shows as no heartbeat.
pub fn heartbeat_text(last: Option<&str>, now: OffsetDateTime) -> String {
    match last.and_then(|text| seconds_since(text, now)) {
        Some(seconds) => format!("Last heartbeat {seconds} seconds ago."),
        None => "No heartbeat yet.".to_owned(),
    }
}

/// Seconds from `text` to `now`, never negative. A clock that runs behind the host shows 0.
fn seconds_since(text: &str, now: OffsetDateTime) -> Option<i64> {
    let moment = OffsetDateTime::parse(text, &Rfc3339).ok()?;
    Some((now - moment).whole_seconds().max(0))
}

/// How long the host was gone, in words: "under a minute", "8 minutes", "1 hour".
pub fn gap_text(gap_seconds: u64) -> String {
    if gap_seconds < MINUTE_SECONDS {
        return "under a minute".to_owned();
    }
    if gap_seconds < HOUR_SECONDS {
        return counted(gap_seconds / MINUTE_SECONDS, "minute");
    }
    counted(gap_seconds / HOUR_SECONDS, "hour")
}

fn counted(count: u64, unit: &str) -> String {
    if count == 1 {
        format!("1 {unit}")
    } else {
        format!("{count} {unit}s")
    }
}

/// The one-line note under the card. A short gap needs no time in the sentence.
pub fn back_online_text(gap_seconds: u64) -> String {
    if gap_seconds < MINUTE_SECONDS {
        return "Back online.".to_owned();
    }
    format!("Back online after {}.", gap_text(gap_seconds))
}

/// The gap of the newest `HostBackOnline` event, if the stream has one.
pub fn latest_back_online(events: &[EventRecord]) -> Option<u64> {
    events.iter().rev().find_map(|record| {
        if let Event::HostBackOnline { gap_seconds } = &record.event {
            Some(*gap_seconds)
        } else {
            None
        }
    })
}

/// The id of the newest host event, heartbeat or back-online. A change of id means the card
/// should load the host again.
pub fn latest_host_event(events: &[EventRecord]) -> Option<i64> {
    events
        .iter()
        .rev()
        .find_map(|record| is_host_event(&record.event).then_some(record.id))
}

fn is_host_event(event: &Event) -> bool {
    matches!(event, Event::HostHeartbeat | Event::HostBackOnline { .. })
}

/// The status line beside the title: where Sentinel runs.
pub fn location_label(headless: bool) -> &'static str {
    if headless {
        "Always-on machine"
    } else {
        "This computer"
    }
}

/// The sentence under the title, matching where Sentinel runs.
pub fn location_sub(headless: bool) -> &'static str {
    if headless {
        "Sentinel, the repository, your agent CLIs, and the Telegram bot all run on a server or a spare computer that stays on. Your PC, phone, and Telegram are remotes."
    } else {
        "Telegram only works while this computer is awake. When it sleeps, the bot goes quiet and runs stop."
    }
}

/// The browser clock. `time` has no clock on `wasm32-unknown-unknown` in this build, so this
/// reads `js_sys`, as `ws.rs` does.
pub fn browser_now() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + time::Duration::seconds_f64(js_sys::Date::now() / 1000.0)
}

/// Loads the host state into `status`. A failure shows as a toast.
pub fn load(status: RwSignal<Option<HostStatus>>, ui: Ui) {
    spawn_local(async move {
        match api::get::<HostStatus>("/api/host").await {
            Ok(value) => status.set(Some(value)),
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

/// Writes a heartbeat now and shows the answer. Only the computer can do this.
pub fn check(status: RwSignal<Option<HostStatus>>, ui: Ui) {
    spawn_local(async move {
        match api::post_empty::<HostStatus>("/api/host/check").await {
            Ok(value) => {
                status.set(Some(value));
                ui.show_toast("Host checked.", ToastKind::Ok);
            }
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> OffsetDateTime {
        OffsetDateTime::parse(text, &Rfc3339).expect("test time is RFC 3339")
    }

    fn host(service: ServiceState) -> HostStatus {
        HostStatus {
            service,
            headless: true,
            uptime_seconds: 0,
            last_heartbeat: None,
            tailscale: TailscaleStatus {
                state: TailscaleState::Running,
                address: Some("100.101.102.103".to_owned()),
                machine_name: Some("fee-host".to_owned()),
            },
            agents_ready: 3,
            agents_total: 3,
            repo_path: Some("~/code/fee-ledger-api".to_owned()),
        }
    }

    fn record(id: i64, event: Event) -> EventRecord {
        EventRecord {
            id,
            chat: None,
            at: OffsetDateTime::UNIX_EPOCH,
            event,
        }
    }

    #[test]
    fn heartbeat_text_counts_seconds_since_the_last_beat() {
        let now = at("2026-10-25T10:00:10Z");
        assert_eq!(
            heartbeat_text(Some("2026-10-25T10:00:06Z"), now),
            "Last heartbeat 4 seconds ago."
        );
    }

    #[test]
    fn heartbeat_text_never_goes_negative() {
        let now = at("2026-10-25T10:00:00Z");
        assert_eq!(
            heartbeat_text(Some("2026-10-25T10:00:05Z"), now),
            "Last heartbeat 0 seconds ago."
        );
    }

    #[test]
    fn heartbeat_text_says_none_yet_without_a_readable_time() {
        let now = at("2026-10-25T10:00:00Z");
        assert_eq!(heartbeat_text(None, now), "No heartbeat yet.");
        assert_eq!(heartbeat_text(Some("not a time"), now), "No heartbeat yet.");
    }

    #[test]
    fn gap_text_is_under_a_minute_for_short_gaps() {
        assert_eq!(gap_text(30), "under a minute");
    }

    #[test]
    fn gap_text_counts_minutes_and_hours() {
        assert_eq!(gap_text(60), "1 minute");
        assert_eq!(gap_text(480), "8 minutes");
        assert_eq!(gap_text(3_600), "1 hour");
        assert_eq!(gap_text(7_200), "2 hours");
    }

    #[test]
    fn back_online_text_leaves_out_the_gap_for_short_outages() {
        assert_eq!(back_online_text(30), "Back online.");
        assert_eq!(back_online_text(480), "Back online after 8 minutes.");
    }

    #[test]
    fn service_row_is_ok_when_installed_and_names_the_heartbeat() {
        let mut status = host(ServiceState::Installed);
        status.last_heartbeat = Some("2026-10-25T10:00:06Z".to_owned());
        let row = check_rows(&status, at("2026-10-25T10:00:10Z"))[0].clone();
        assert!(row.ok);
        assert_eq!(
            row.detail,
            "Installed and running. Last heartbeat 4 seconds ago."
        );
    }

    #[test]
    fn service_row_tells_the_user_how_to_install_when_missing() {
        let status = host(ServiceState::NotInstalled);
        let row = check_rows(&status, at("2026-10-25T10:00:00Z"))[0].clone();
        assert!(!row.ok);
        assert_eq!(
            row.detail,
            "Not installed. Run sentinel service install on this machine."
        );
    }

    #[test]
    fn tailscale_row_names_the_machine_when_running() {
        let status = host(ServiceState::Installed);
        let row = check_rows(&status, at("2026-10-25T10:00:00Z"))[1].clone();
        assert!(row.ok);
        assert_eq!(row.detail, "Connected as fee-host.");
    }

    #[test]
    fn tailscale_row_is_not_ok_when_signed_out() {
        let mut status = host(ServiceState::Installed);
        status.tailscale.state = TailscaleState::SignedOut;
        let row = check_rows(&status, at("2026-10-25T10:00:00Z"))[1].clone();
        assert!(!row.ok);
        assert_eq!(row.detail, "Installed, but signed out.");
    }

    #[test]
    fn agents_row_is_ok_only_when_every_agent_is_ready() {
        let mut status = host(ServiceState::Installed);
        status.agents_ready = 2;
        let row = check_rows(&status, at("2026-10-25T10:00:00Z"))[2].clone();
        assert!(!row.ok);
        assert_eq!(row.detail, "2 of 3 ready.");
    }

    #[test]
    fn repository_row_says_when_no_folder_is_chosen() {
        let mut status = host(ServiceState::Installed);
        status.repo_path = None;
        let row = check_rows(&status, at("2026-10-25T10:00:00Z"))[3].clone();
        assert!(!row.ok);
        assert_eq!(row.detail, "No folder chosen.");
    }

    #[test]
    fn latest_back_online_reads_the_newest_gap() {
        let events = [
            record(1, Event::HostBackOnline { gap_seconds: 120 }),
            record(2, Event::HostHeartbeat),
            record(3, Event::HostBackOnline { gap_seconds: 480 }),
        ];
        assert_eq!(latest_back_online(&events), Some(480));
        assert_eq!(latest_back_online(&events[1..2]), None);
    }

    #[test]
    fn latest_host_event_is_the_newest_heartbeat_or_back_online() {
        let events = [
            record(4, Event::HostHeartbeat),
            record(5, Event::HostBackOnline { gap_seconds: 90 }),
            record(6, Event::BarrierStart { wave: 2 }),
        ];
        assert_eq!(latest_host_event(&events), Some(5));
        assert_eq!(latest_host_event(&[]), None);
    }

    #[test]
    fn location_text_follows_headless() {
        assert_eq!(location_label(true), "Always-on machine");
        assert_eq!(location_label(false), "This computer");
        assert!(location_sub(false).starts_with("Telegram only works"));
    }
}
