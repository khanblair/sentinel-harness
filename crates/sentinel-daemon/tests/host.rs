//! The always-on host: the heartbeat, the back-online gap, and the host card.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers fail the test by panicking"
)]

use sentinel_core::api::ServiceState;
use sentinel_daemon::folders::Pick;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

mod common;
use common::{fake_engine, sample_repo};

fn minutes_ago(minutes: i64) -> String {
    (OffsetDateTime::now_utc() - Duration::minutes(minutes))
        .format(&Rfc3339)
        .unwrap()
}

#[test]
fn a_new_store_has_no_heartbeat_and_no_gap() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    assert_eq!(engine.last_heartbeat().unwrap(), None);
    assert_eq!(engine.downtime_seconds().unwrap(), None);
}

#[test]
fn a_fresh_heartbeat_is_not_a_gap() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    engine.beat().unwrap();
    assert_eq!(engine.downtime_seconds().unwrap(), None);
}

#[test]
fn a_heartbeat_from_five_minutes_ago_is_a_gap_of_about_five_minutes() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    engine
        .store()
        .set_setting("host.heartbeat", &minutes_ago(5))
        .unwrap();
    let gap = engine.downtime_seconds().unwrap().unwrap();
    assert!((295..=320).contains(&gap), "{gap}");
}

#[tokio::test]
async fn the_host_card_reports_the_service_and_the_agents() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    engine.scan_tools().await.unwrap();
    engine.set_headless(true);
    let status = engine.host_status().await.unwrap();
    assert!(status.headless);
    assert!(status.agents_ready >= 1);
    // The test home is the repository folder, which holds no service file.
    assert_eq!(status.service, ServiceState::NotInstalled);
}

#[test]
fn a_headless_host_has_no_native_folder_dialog() {
    let repo = sample_repo();
    let engine = fake_engine(repo.path());
    engine.set_headless(true);
    assert_eq!(engine.pick_folder_blocking(), Pick::Unavailable);
}
