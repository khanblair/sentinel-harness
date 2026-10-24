//! Property tests for the wave builder: the wave stays conflict free and capped.

use proptest::prelude::*;
use sentinel_core::{Scope, Task, TaskId};
use sentinel_reserve::build_wave;

/// A small path alphabet. `c/` is a folder that covers `c/x.rs` below it.
const PATHS: [&str; 5] = ["a.rs", "b.rs", "c/", "c/x.rs", "d/c.rs"];

fn scope_strategy() -> impl Strategy<Value = Vec<usize>> {
    prop::collection::vec(0..PATHS.len(), 0..3).prop_map(|picks| {
        let mut picks = picks;
        picks.sort_unstable();
        picks.dedup();
        picks
    })
}

fn task_of(id: usize, picks: &[usize]) -> Task {
    let mut task = Task::new(TaskId::new(format!("T{id}")), format!("task {id}"), "");
    for pick in picks {
        if let Err(error) = task.scope.insert(PATHS[*pick]) {
            panic!("test alphabet paths are valid: {error}");
        }
    }
    task
}

fn held_of(id: usize, picks: &[usize]) -> (TaskId, Scope) {
    let paths: Vec<&str> = picks.iter().map(|pick| PATHS[*pick]).collect();
    match Scope::from_paths(paths) {
        Ok(scope) => (TaskId::new(format!("T{id}")), scope),
        Err(error) => panic!("test alphabet paths are valid: {error}"),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn wave_stays_conflict_free_capped_and_in_ready_order(
        ready_picks in prop::collection::vec(scope_strategy(), 0..5),
        held_items in prop::collection::vec((0..5usize, scope_strategy()), 0..4),
        max_tasks in 0..6usize,
    ) {
        let ready: Vec<Task> = ready_picks
            .iter()
            .enumerate()
            .map(|(id, picks)| task_of(id, picks))
            .collect();
        let held: Vec<(TaskId, Scope)> = held_items
            .iter()
            .map(|(id, picks)| held_of(*id, picks))
            .collect();
        let wave = build_wave(&ready, &held, max_tasks);

        prop_assert!(wave.len() <= max_tasks);

        let positions: Vec<usize> = wave
            .iter()
            .map(|id| ready.iter().position(|task| &task.id == id).unwrap())
            .collect();
        for pair in positions.windows(2) {
            prop_assert!(pair[0] < pair[1], "wave keeps the ready order");
        }

        for &taken in &positions {
            for &other in &positions {
                if other != taken {
                    prop_assert!(
                        !ready[taken].scope.overlaps(&ready[other].scope),
                        "two wave tasks overlap: {} and {}",
                        ready[taken].id,
                        ready[other].id
                    );
                }
            }
            for (held_id, held_scope) in &held {
                if *held_id != ready[taken].id {
                    prop_assert!(
                        !held_scope.overlaps(&ready[taken].scope),
                        "wave task {} overlaps held work of {}",
                        ready[taken].id,
                        held_id
                    );
                }
            }
        }
    }
}
