//! A roster of three scripted agents runs the sample backlog, and each agent takes at least one task.

use std::collections::HashSet;

use sentinel_core::api::RunRequest;
use sentinel_core::{Event, ToolId};
use sentinel_daemon::roster::parse_roster;

mod common;
use common::{fake_engine, sample_repo, wait_done};

/// The agents that a "goes to" line of the stream names, one per task attempt.
fn agents_that_took_a_task(events: &[sentinel_core::EventRecord]) -> HashSet<ToolId> {
    events
        .iter()
        .filter_map(|record| {
            let Event::Message {
                to: Some(agent),
                text,
                ..
            } = &record.event
            else {
                return None;
            };
            text.contains(" goes to ").then(|| agent.clone())
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_roster_of_three_agents_gives_each_one_a_task() {
    let dir = sample_repo();
    let engine = fake_engine(dir.path());
    engine.scan_tools().await.unwrap();
    let specs = parse_roster("fake-a,fake-b,fake-c").unwrap();
    let roster = engine.apply_roster(&specs).unwrap();
    engine.start_run(RunRequest::default()).await.unwrap();
    wait_done(&engine).await;

    let events = engine.events_after(0, 100_000).unwrap();
    let taken = agents_that_took_a_task(&events);
    for agent in &roster {
        assert!(taken.contains(agent), "{agent} took no task");
    }
}
