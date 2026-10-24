//! Task graph with cycle detection, topological order, and ready tasks.

use std::collections::{HashMap, HashSet};

use crate::{PlanError, Result};
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use sentinel_core::{Dependency, DependencyKind, Task, TaskId};

/// Task graph using petgraph `DiGraph`.
/// Nodes are `TaskId`s, edges are dependencies with kind.
#[derive(Debug)]
pub struct TaskGraph {
    graph: DiGraph<TaskId, DependencyKind>,
    node_map: HashMap<TaskId, NodeIndex>,
    task_order: Vec<TaskId>,
}

impl TaskGraph {
    /// Create a new task graph from tasks and dependencies.
    /// Returns `UnknownTask` if a dependency names a missing task.
    /// Returns `Cycle` with the task ids on the cycle if one exists.
    pub fn new(tasks: &[Task], deps: &[Dependency]) -> Result<TaskGraph> {
        let mut graph = DiGraph::new();
        let mut node_map = HashMap::new();
        let mut task_order = Vec::new();

        // Add all tasks as nodes, preserving order
        for task in tasks {
            let idx = graph.add_node(task.id.clone());
            node_map.insert(task.id.clone(), idx);
            task_order.push(task.id.clone());
        }

        // Add dependency edges
        for dep in deps {
            let from_idx = *node_map
                .get(&dep.depends_on)
                .ok_or_else(|| PlanError::UnknownTask(dep.depends_on.to_string()))?;
            let to_idx = *node_map
                .get(&dep.task)
                .ok_or_else(|| PlanError::UnknownTask(dep.task.to_string()))?;
            graph.add_edge(from_idx, to_idx, dep.kind);
        }

        // Check for cycles
        if let Err(cycle) = toposort(&graph, None) {
            let cycle_ids = extract_cycle_ids(&graph, cycle.node_id());
            return Err(PlanError::Cycle(cycle_ids));
        }

        Ok(TaskGraph {
            graph,
            node_map,
            task_order,
        })
    }

    /// Return a topological order, ties broken by the original task order.
    pub fn order(&self) -> Vec<TaskId> {
        // petgraph's toposort does not keep task order for ties, so Kahn's
        // algorithm runs here with a sorted ready list.
        let order_index: HashMap<&TaskId, usize> = self
            .task_order
            .iter()
            .enumerate()
            .map(|(i, id)| (id, i))
            .collect();
        let mut in_degree = self.in_degrees();
        let mut ready: Vec<NodeIndex> = self
            .graph
            .node_indices()
            .filter(|node| in_degree[node.index()] == 0)
            .collect();
        self.sort_by_task_order(&mut ready, &order_index);

        let mut result = Vec::with_capacity(self.graph.node_count());
        while !ready.is_empty() {
            let node = ready.remove(0);
            if let Some(id) = self.graph.node_weight(node) {
                result.push(id.clone());
            }
            ready.extend(self.release_dependents(node, &mut in_degree));
            self.sort_by_task_order(&mut ready, &order_index);
        }
        result
    }

    /// Return tasks not in `done` whose dependencies are all in `done`, in task order.
    pub fn ready(&self, done: &HashSet<TaskId>) -> Vec<TaskId> {
        let mut ready = Vec::new();
        for task_id in &self.task_order {
            if done.contains(task_id) {
                continue;
            }
            let Some(&node) = self.node_map.get(task_id) else {
                continue;
            };
            let all_deps_done = self
                .graph
                .edges_directed(node, petgraph::Direction::Incoming)
                .all(|e| {
                    self.graph
                        .node_weight(e.source())
                        .is_some_and(|id| done.contains(id))
                });
            if all_deps_done {
                ready.push(task_id.clone());
            }
        }
        ready
    }

    /// Return tasks that depend on the given task (direct dependents).
    pub fn dependents_of(&self, id: &TaskId) -> Vec<TaskId> {
        let Some(&node) = self.node_map.get(id) else {
            return Vec::new();
        };
        self.graph
            .edges_directed(node, petgraph::Direction::Outgoing)
            .filter_map(|e| self.graph.node_weight(e.target()).cloned())
            .collect()
    }

    /// Number of incoming edges per node, indexed by `NodeIndex::index`.
    fn in_degrees(&self) -> Vec<usize> {
        let mut in_degree = vec![0; self.graph.node_count()];
        for edge in self.graph.edge_references() {
            in_degree[edge.target().index()] += 1;
        }
        in_degree
    }

    /// Remove one incoming edge from each dependent of `node`.
    /// Returns the dependents that have no edges left.
    fn release_dependents(&self, node: NodeIndex, in_degree: &mut [usize]) -> Vec<NodeIndex> {
        let mut released = Vec::new();
        for edge in self.graph.edges(node) {
            let target = edge.target();
            in_degree[target.index()] -= 1;
            if in_degree[target.index()] == 0 {
                released.push(target);
            }
        }
        released
    }

    fn sort_by_task_order(&self, ready: &mut [NodeIndex], order_index: &HashMap<&TaskId, usize>) {
        ready.sort_by_key(|&node| self.task_rank(node, order_index));
    }

    fn task_rank(&self, node: NodeIndex, order_index: &HashMap<&TaskId, usize>) -> usize {
        self.graph
            .node_weight(node)
            .and_then(|id| order_index.get(id))
            .copied()
            .unwrap_or(usize::MAX)
    }
}

/// Depth-first search for a cycle that passes through `start`.
struct CycleSearch<'a> {
    graph: &'a DiGraph<TaskId, DependencyKind>,
    start: NodeIndex,
    visited: HashSet<NodeIndex>,
    path: Vec<NodeIndex>,
}

impl CycleSearch<'_> {
    fn visit(&mut self, node: NodeIndex) -> Option<Vec<TaskId>> {
        self.visited.insert(node);
        self.path.push(node);
        let graph = self.graph;
        for target in graph.edges(node).map(|edge| edge.target()) {
            if let Some(cycle) = self.follow(target) {
                return Some(cycle);
            }
        }
        self.path.pop();
        None
    }

    fn follow(&mut self, target: NodeIndex) -> Option<Vec<TaskId>> {
        if !self.visited.contains(&target) {
            return self.visit(target);
        }
        if target == self.start && self.path.len() > 1 {
            return Some(self.path_ids());
        }
        None
    }

    fn path_ids(&self) -> Vec<TaskId> {
        self.path
            .iter()
            .filter_map(|node| self.graph.node_weight(*node).cloned())
            .collect()
    }
}

fn extract_cycle_ids(graph: &DiGraph<TaskId, DependencyKind>, start: NodeIndex) -> Vec<TaskId> {
    let mut search = CycleSearch {
        graph,
        start,
        visited: HashSet::new(),
        path: Vec::new(),
    };
    search.visit(start).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{Dependency, DependencyKind, Task, TaskId};

    fn make_task(id: &str) -> Task {
        Task::new(TaskId::new(id), format!("Task {id}"), "Body".to_string())
    }

    #[test]
    fn cycle_error_names_tasks() {
        let tasks = vec![make_task("T-01"), make_task("T-02"), make_task("T-03")];
        let deps = vec![
            Dependency {
                task: TaskId::new("T-01"),
                depends_on: TaskId::new("T-02"),
                kind: DependencyKind::Order,
            },
            Dependency {
                task: TaskId::new("T-02"),
                depends_on: TaskId::new("T-03"),
                kind: DependencyKind::Order,
            },
            Dependency {
                task: TaskId::new("T-03"),
                depends_on: TaskId::new("T-01"),
                kind: DependencyKind::Order,
            },
        ];
        let err = TaskGraph::new(&tasks, &deps).unwrap_err();
        assert!(matches!(err, PlanError::Cycle(ids) if ids.len() >= 3));
    }

    #[test]
    fn order_respects_dependencies_and_task_order() {
        let tasks = vec![
            make_task("T-01"),
            make_task("T-02"),
            make_task("T-03"),
            make_task("T-04"),
        ];
        let deps = vec![
            Dependency {
                task: TaskId::new("T-03"),
                depends_on: TaskId::new("T-01"),
                kind: DependencyKind::Order,
            },
            Dependency {
                task: TaskId::new("T-04"),
                depends_on: TaskId::new("T-02"),
                kind: DependencyKind::Order,
            },
        ];
        let graph = TaskGraph::new(&tasks, &deps).unwrap();
        let order = graph.order();
        // T-01 before T-03, T-02 before T-04, and original order for ties
        let pos: HashMap<&TaskId, usize> =
            order.iter().enumerate().map(|(i, id)| (id, i)).collect();
        assert!(pos[&TaskId::new("T-01")] < pos[&TaskId::new("T-03")]);
        assert!(pos[&TaskId::new("T-02")] < pos[&TaskId::new("T-04")]);
        // T-01 and T-02 have no deps between them, so T-01 comes first (original order)
        assert!(pos[&TaskId::new("T-01")] < pos[&TaskId::new("T-02")]);
    }

    #[test]
    fn ready_with_done_set() {
        let tasks = vec![make_task("T-01"), make_task("T-02"), make_task("T-03")];
        let deps = vec![
            Dependency {
                task: TaskId::new("T-02"),
                depends_on: TaskId::new("T-01"),
                kind: DependencyKind::Order,
            },
            Dependency {
                task: TaskId::new("T-03"),
                depends_on: TaskId::new("T-02"),
                kind: DependencyKind::Order,
            },
        ];
        let graph = TaskGraph::new(&tasks, &deps).unwrap();

        let mut done = HashSet::new();
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
        let deps = vec![Dependency {
            task: TaskId::new("T-01"),
            depends_on: TaskId::new("T-99"),
            kind: DependencyKind::Order,
        }];
        let err = TaskGraph::new(&tasks, &deps).unwrap_err();
        assert!(matches!(err, PlanError::UnknownTask(s) if s == "T-99"));
    }
}
