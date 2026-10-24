//! Graph tests for the task graph.

use sentinel_core::{Dependency, DependencyKind, Task, TaskId};
use sentinel_plan::{PlanError, graph::TaskGraph};

fn make_task(id: &str) -> Task {
    Task::new(TaskId::new(id), format!("Task {id}"), "Body".to_string())
}

fn order_dep(task: &str, depends_on: &str) -> Dependency {
    Dependency {
        task: TaskId::new(task),
        depends_on: TaskId::new(depends_on),
        kind: DependencyKind::Order,
    }
}

#[test]
fn cycle_error_names_tasks() {
    let tasks = vec![make_task("T-01"), make_task("T-02"), make_task("T-03")];
    let deps = vec![
        order_dep("T-01", "T-02"),
        order_dep("T-02", "T-03"),
        order_dep("T-03", "T-01"),
    ];
    let err = TaskGraph::new(&tasks, &deps).unwrap_err();
    assert!(matches!(err, PlanError::Cycle(ref ids) if ids.len() >= 3));
    // Check all three tasks are in the cycle
    if let PlanError::Cycle(ids) = err {
        let id_strs: Vec<_> = ids.iter().map(TaskId::as_str).collect();
        assert!(id_strs.contains(&"T-01"));
        assert!(id_strs.contains(&"T-02"));
        assert!(id_strs.contains(&"T-03"));
    }
}

#[test]
fn order_respects_dependencies_and_task_order() {
    let tasks = vec![
        make_task("T-01"),
        make_task("T-02"),
        make_task("T-03"),
        make_task("T-04"),
    ];
    let deps = vec![order_dep("T-03", "T-01"), order_dep("T-04", "T-02")];
    let graph = TaskGraph::new(&tasks, &deps).unwrap();
    let order = graph.order();
    let pos: std::collections::HashMap<&TaskId, usize> =
        order.iter().enumerate().map(|(i, id)| (id, i)).collect();
    assert!(pos[&TaskId::new("T-01")] < pos[&TaskId::new("T-03")]);
    assert!(pos[&TaskId::new("T-02")] < pos[&TaskId::new("T-04")]);
    assert!(pos[&TaskId::new("T-01")] < pos[&TaskId::new("T-02")]);
}

#[test]
fn ready_with_done_set() {
    let tasks = vec![make_task("T-01"), make_task("T-02"), make_task("T-03")];
    let deps = vec![order_dep("T-02", "T-01"), order_dep("T-03", "T-02")];
    let graph = TaskGraph::new(&tasks, &deps).unwrap();

    let mut done = std::collections::HashSet::new();
    assert_eq!(graph.ready(&done), vec![TaskId::new("T-01")]);

    done.insert(TaskId::new("T-01"));
    assert_eq!(graph.ready(&done), vec![TaskId::new("T-02")]);

    done.insert(TaskId::new("T-02"));
    assert_eq!(graph.ready(&done), vec![TaskId::new("T-03")]);

    done.insert(TaskId::new("T-03"));
    assert_eq!(graph.ready(&done), Vec::<TaskId>::new());
}

#[test]
fn unknown_dependency_error() {
    let tasks = vec![make_task("T-01")];
    let deps = vec![order_dep("T-01", "T-99")];
    let err = TaskGraph::new(&tasks, &deps).unwrap_err();
    assert!(matches!(err, PlanError::UnknownTask(s) if s == "T-99"));
}

#[test]
fn dependents_of_returns_direct_dependents() {
    let tasks = vec![
        make_task("T-01"),
        make_task("T-02"),
        make_task("T-03"),
        make_task("T-04"),
    ];
    let deps = vec![
        order_dep("T-02", "T-01"),
        order_dep("T-03", "T-01"),
        order_dep("T-04", "T-02"),
    ];
    let graph = TaskGraph::new(&tasks, &deps).unwrap();

    let deps_of_t1 = graph.dependents_of(&TaskId::new("T-01"));
    assert_eq!(deps_of_t1.len(), 2);
    assert!(deps_of_t1.contains(&TaskId::new("T-02")));
    assert!(deps_of_t1.contains(&TaskId::new("T-03")));

    let deps_of_t2 = graph.dependents_of(&TaskId::new("T-02"));
    assert_eq!(deps_of_t2, vec![TaskId::new("T-04")]);

    let deps_of_t4 = graph.dependents_of(&TaskId::new("T-04"));
    assert_eq!(deps_of_t4, Vec::<TaskId>::new());

    // Unknown task returns empty
    let deps_of_unknown = graph.dependents_of(&TaskId::new("T-99"));
    assert_eq!(deps_of_unknown, Vec::<TaskId>::new());
}
