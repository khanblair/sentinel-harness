//! Two small background jobs of a run: the write guard's events become stream events, and the
//! budget guard's spend and thresholds become stream events every half second.

use std::sync::Arc;
use std::time::Duration;

use sentinel_core::Event;
use sentinel_workers::Budget;
use sentinel_workers::budget::BudgetEvent;
use tokio::sync::{mpsc, watch};

use crate::engine::Engine;

const PUMP_INTERVAL: Duration = Duration::from_millis(500);

/// Events from the write guard thread become stream events.
pub(crate) async fn forward_guard_events(
    engine: Engine,
    mut events: mpsc::UnboundedReceiver<Event>,
) {
    while let Some(event) = events.recv().await {
        engine.emit_quiet(&event);
    }
}

/// The supervisor records spend without knowing about the stream. This turns budget events and
/// cost changes into events every half second.
pub(crate) async fn budget_pump(
    engine: Engine,
    budget: Arc<Budget>,
    mut stop: watch::Receiver<bool>,
) {
    let mut last = None;
    while !*stop.borrow() {
        for event in budget.drain_events() {
            engine.emit_quiet(&budget_event(event));
        }
        let spent = budget.spent();
        if last != Some(spent) {
            engine.emit_quiet(&Event::CostUpdate {
                total: spent,
                cap: budget.cap(),
            });
            last = Some(spent);
        }
        tokio::select! {
            () = tokio::time::sleep(PUMP_INTERVAL) => {}
            _ = stop.changed() => {}
        }
    }
    for event in budget.drain_events() {
        engine.emit_quiet(&budget_event(event));
    }
}

fn budget_event(event: BudgetEvent) -> Event {
    match event {
        BudgetEvent::Threshold70 => Event::BudgetThreshold { percent: 70 },
        BudgetEvent::Threshold95 => Event::BudgetThreshold { percent: 95 },
        BudgetEvent::CeilingHit {
            task,
            cost,
            ceiling,
        } => Event::TaskCeilingHit {
            task,
            cost,
            ceiling,
        },
    }
}
