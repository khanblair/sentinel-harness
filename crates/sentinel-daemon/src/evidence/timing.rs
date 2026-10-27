//! Run slicing and timing: the events of the last run, its waves, and how long its tasks ran.

use std::collections::BTreeMap;

use sentinel_core::{Event, EventRecord, TaskId, TaskStatus};
use time::OffsetDateTime;

use super::WaveEvidence;

/// The events from the last `RunStarted` to its `RunFinished`, or to the end when it never ended.
pub(super) fn current_run(events: &[EventRecord]) -> &[EventRecord] {
    let Some(start) = events
        .iter()
        .rposition(|record| matches!(record.event, Event::RunStarted { .. }))
    else {
        return &[];
    };
    let rest = &events[start..];
    let end = rest
        .iter()
        .position(|record| matches!(record.event, Event::RunFinished { .. }))
        .map_or(rest.len(), |index| index + 1);
    &rest[..end]
}

pub(super) fn waves_for(run: &[EventRecord]) -> WaveEvidence {
    let sizes: Vec<usize> = run
        .iter()
        .filter_map(|record| wave_size(&record.event))
        .collect();
    WaveEvidence {
        count: sizes.len(),
        most_tasks: sizes.iter().copied().max().unwrap_or(0),
        parallel_share: parallel_share(run),
    }
}

fn wave_size(event: &Event) -> Option<usize> {
    let Event::WaveStarted { tasks, .. } = event else {
        return None;
    };
    Some(tasks.len())
}

fn parallel_share(run: &[EventRecord]) -> f64 {
    let Some(wall) = wall_millis(run) else {
        return 0.0;
    };
    ratio(task_run_millis(run), wall)
}

/// From `RunStarted` to `RunFinished`, or to the last event when the run never finished.
fn wall_millis(run: &[EventRecord]) -> Option<i128> {
    let start = run.first()?.at;
    let finish = run
        .iter()
        .find(|record| matches!(record.event, Event::RunFinished { .. }))
        .or(run.last())?;
    Some((finish.at - start).whole_milliseconds())
}

/// The sum of every attempt's run time. An attempt starts at its `Running` status and ends at the
/// next checkpoint or verdict. An attempt still open at the end runs to the last event.
fn task_run_millis(run: &[EventRecord]) -> i128 {
    let mut open: BTreeMap<TaskId, OffsetDateTime> = BTreeMap::new();
    let mut total: i128 = 0;
    for record in run {
        if let Event::TaskStatus {
            task,
            status: TaskStatus::Running,
        } = &record.event
        {
            open.insert(task.clone(), record.at);
        } else if let Some(task) = ended_task(&record.event)
            && let Some(start) = open.remove(task)
        {
            total += (record.at - start).whole_milliseconds();
        }
    }
    if let Some(last) = run.last() {
        total += open
            .values()
            .map(|start| (last.at - *start).whole_milliseconds())
            .sum::<i128>();
    }
    total
}

fn ended_task(event: &Event) -> Option<&TaskId> {
    let (Event::Checkpoint { task, .. } | Event::Verdict { task, .. }) = event else {
        return None;
    };
    Some(task)
}

/// `part` over `whole`, to two decimals. Zero when there is no wall time to divide by.
fn ratio(part: i128, whole: i128) -> f64 {
    if whole <= 0 {
        return 0.0;
    }
    let hundredths = part * 100 / whole;
    f64::from(u32::try_from(hundredths).unwrap_or(0)) / 100.0
}
