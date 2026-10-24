//! A wave is the set of ready tasks that can run together.

use sentinel_core::{Scope, Task, TaskId};

/// Pick ready tasks whose scopes do not collide with the wave or with work held
/// by other tasks. Input order decides output order.
pub fn build_wave(ready: &[Task], held: &[(TaskId, Scope)], max_tasks: usize) -> Vec<TaskId> {
    let mut wave = Vec::new();
    let mut wave_scopes: Vec<&Scope> = Vec::new();
    for task in ready {
        if wave.len() >= max_tasks {
            break;
        }
        if wave_scopes.iter().any(|scope| scope.overlaps(&task.scope)) {
            continue;
        }
        let blocked = held
            .iter()
            .any(|(id, scope)| id != &task.id && scope.overlaps(&task.scope));
        if blocked {
            continue;
        }
        wave.push(task.id.clone());
        wave_scopes.push(&task.scope);
    }
    wave
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, paths: &[&str]) -> Task {
        let mut task = Task::new(TaskId::new(id), id, id);
        for path in paths {
            task.scope.insert(path).unwrap();
        }
        task
    }

    fn held(task: &str, paths: &[&str]) -> (TaskId, Scope) {
        (TaskId::new(task), Scope::from_paths(paths).unwrap())
    }

    #[test]
    fn overlapping_tasks_split_and_the_next_free_task_is_taken() {
        let ready = [
            task("A", &["src/a.rs"]),
            task("B", &["src/a.rs"]),
            task("C", &["src/b.rs"]),
        ];
        let wave = build_wave(&ready, &[], 10);
        assert_eq!(wave, vec![TaskId::new("A"), TaskId::new("C")]);
    }

    #[test]
    fn max_tasks_caps_the_wave_and_zero_gives_none() {
        let ready = [task("A", &[]), task("B", &[]), task("C", &[])];
        assert_eq!(build_wave(&ready, &[], 0), Vec::<TaskId>::new());
        let capped = build_wave(&ready, &[], 2);
        assert_eq!(capped, vec![TaskId::new("A"), TaskId::new("B")]);
    }

    #[test]
    fn held_scopes_block_other_tasks_but_not_the_holding_task() {
        let ready = [task("B", &["src/a.rs"])];
        let foreign = [held("A", &["src/"])];
        assert_eq!(build_wave(&ready, &foreign, 5), Vec::<TaskId>::new());
        let own = [held("B", &["src/"])];
        assert_eq!(build_wave(&ready, &own, 5), vec![TaskId::new("B")]);
    }

    #[test]
    fn an_empty_scope_rides_along_next_to_held_work() {
        let ready = [task("A", &[]), task("B", &["src/a.rs"])];
        let held = [held("C", &["src/"])];
        assert_eq!(build_wave(&ready, &held, 5), vec![TaskId::new("A")]);
    }

    #[test]
    fn the_wave_follows_the_ready_order() {
        let ready = [task("C", &["docs/"]), task("A", &["src/"]), task("B", &[])];
        let wave = build_wave(&ready, &[], 5);
        assert_eq!(
            wave,
            vec![TaskId::new("C"), TaskId::new("A"), TaskId::new("B")]
        );
    }
}
