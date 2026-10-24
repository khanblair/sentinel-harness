//! Task graph with cycle detection, topological order, and ready tasks.

#![allow(clippy::too_many_lines)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::doc_markdown)]

use std::collections::{HashMap, HashSet};

use crate::{PlanError, Result};
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use sentinel_core::{Dependency, DependencyKind, Task, TaskId};

/// Task graph using petgraph DiGraph.
/// Nodes are TaskIds, edges are dependencies with kind.
#[derive(Debug)]
pub struct TaskGraph {
    graph: DiGraph<TaskId, DependencyKind>,
    node_map: HashMap<TaskId, NodeIndex>,
    task_order: Vec<TaskId>,
}

impl TaskGraph {
    /// Create a new task graph from tasks and dependencies.
    /// Returns UnknownTask if a dependency names a missing task.
    /// Returns Cycle with the task ids on the cycle if one exists.
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
            let cycle_node = cycle.node_id();
            let cycle_ids = extract_cycle_ids(&graph, cycle_node, &node_map);
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
        // We need a stable topological sort that respects task_order for ties.
        // petgraph's toposort doesn't guarantee stability, so we implement our own.
        let mut in_degree: HashMap<NodeIndex, usize> = HashMap::new();
        for node in self.graph.node_indices() {
            in_degree.insert(node, 0);
        }
        for edge in self.graph.edge_references() {
            *in_degree.get_mut(&edge.target()).unwrap() += 1;
        }

        // Use a priority queue based on task_order index
        let order_index: HashMap<&TaskId, usize> = self
            .task_order
            .iter()
            .enumerate()
            .map(|(i, id)| (id, i))
            .collect();

        let mut ready: Vec<NodeIndex> = in_degree
            .iter()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(&node, _)| node)
            .collect();

        // Sort ready nodes by task_order (lowest index first)
        ready.sort_by_key(|&n| {
            order_index
                .get(self.graph.node_weight(n).unwrap())
                .copied()
                .unwrap_or(usize::MAX)
        });

        let mut result = Vec::new();
        while let Some(node) = ready.first().copied() {
            ready.remove(0);
            let task_id = self.graph.node_weight(node).unwrap().clone();
            result.push(task_id);

            let mut newly_ready = Vec::new();
            for edge in self.graph.edges(node) {
                let target = edge.target();
                let deg = in_degree.get_mut(&target).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    newly_ready.push(target);
                }
            }
            // Sort newly ready by task_order and merge with ready, keeping sorted order
            newly_ready.sort_by_key(|&n| {
                order_index
                    .get(self.graph.node_weight(n).unwrap())
                    .copied()
                    .unwrap_or(usize::MAX)
            });
            // Merge two sorted vectors
            let mut merged = Vec::with_capacity(ready.len() + newly_ready.len());
            let mut i = 0;
            let mut j = 0;
            while i < ready.len() && j < newly_ready.len() {
                let ri = order_index
                    .get(self.graph.node_weight(ready[i]).unwrap())
                    .copied()
                    .unwrap_or(usize::MAX);
                let nj = order_index
                    .get(self.graph.node_weight(newly_ready[j]).unwrap())
                    .copied()
                    .unwrap_or(usize::MAX);
                if ri <= nj {
                    merged.push(ready[i]);
                    i += 1;
                } else {
                    merged.push(newly_ready[j]);
                    j += 1;
                }
            }
            merged.extend_from_slice(&ready[i..]);
            merged.extend_from_slice(&newly_ready[j..]);
            ready = merged;
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
            let node = self.node_map[task_id];
            let all_deps_done = self
                .graph
                .edges_directed(node, petgraph::Direction::Incoming)
                .all(|e| done.contains(self.graph.node_weight(e.source()).unwrap()));
            if all_deps_done {
                ready.push(task_id.clone());
            }
        }
        ready
    }

    /// Return tasks that depend on the given task (direct dependents).
    pub fn dependents_of(&self, id: &TaskId) -> Vec<TaskId> {
        let node = match self.node_map.get(id) {
            Some(n) => *n,
            None => return Vec::new(),
        };
        self.graph
            .edges_directed(node, petgraph::Direction::Outgoing)
            .map(|e| self.graph.node_weight(e.target()).unwrap().clone())
            .collect()
    }
}

fn extract_cycle_ids(
    graph: &DiGraph<TaskId, DependencyKind>,
    start: NodeIndex,
    _node_map: &HashMap<TaskId, NodeIndex>,
) -> Vec<TaskId> {
    // Use DFS to find a cycle path from start back to start
    let mut visited = HashSet::new();
    let mut path = Vec::new();
    let mut cycle = Vec::new();

    fn dfs(
        graph: &DiGraph<TaskId, DependencyKind>,
        node: NodeIndex,
        start: NodeIndex,
        visited: &mut HashSet<NodeIndex>,
        path: &mut Vec<NodeIndex>,
        cycle: &mut Vec<TaskId>,
    ) -> bool {
        visited.insert(node);
        path.push(node);

        if node == start && path.len() > 1 {
            // Found cycle
            *cycle = path
                .iter()
                .map(|n| graph.node_weight(*n).unwrap().clone())
                .collect();
            return true;
        }

        for edge in graph.edges(node) {
            if !visited.contains(&edge.target()) {
                if dfs(graph, edge.target(), start, visited, path, cycle) {
                    return true;
                }
            } else if edge.target() == start && path.len() > 1 {
                // Found cycle back to start
                *cycle = path
                    .iter()
                    .map(|n| graph.node_weight(*n).unwrap().clone())
                    .collect();
                return true;
            }
        }

        path.pop();
        false
    }

    dfs(graph, start, start, &mut visited, &mut path, &mut cycle);
    cycle
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
