//! Plan validation: ids, references, scope paths, and dependency cycles.

use std::collections::HashSet;

use crate::planner::{Plan, PlanRequest, PlannedTask, dependencies_of};
use crate::{PlanError, Result, graph::TaskGraph};
use sentinel_core::{Dependency, Task, TaskId};

/// Validate a plan against the request.
pub fn validate_plan(plan: &Plan, request: &PlanRequest) -> Result<()> {
    // Check every backlog item has exactly one planned task
    let item_ids: HashSet<&TaskId> = request.items.iter().map(|i| &i.id).collect();
    let mut seen_ids = HashSet::new();

    for pt in &plan.tasks {
        if !item_ids.contains(&pt.id) {
            return Err(PlanError::Parse(format!(
                "plan contains unknown task: {}",
                pt.id
            )));
        }
        if !seen_ids.insert(&pt.id) {
            return Err(PlanError::Parse(format!(
                "duplicate planned task: {}",
                pt.id
            )));
        }
        check_planned_task(pt, &item_ids)?;
    }

    check_all_items_planned(request, &seen_ids)?;
    check_acyclic(plan)
}

/// Checks that need only the task itself and the set of backlog ids.
fn check_planned_task(pt: &PlannedTask, item_ids: &HashSet<&TaskId>) -> Result<()> {
    check_no_self_reference(pt)?;
    check_known_refs(item_ids, &pt.id, &pt.depends_on, "depends on")?;
    check_known_refs(
        item_ids,
        &pt.id,
        &pt.read_after_write,
        "has read_after_write to",
    )?;
    check_known_refs(item_ids, &pt.id, &pt.contradicts, "contradicts")?;
    pt.scope.iter().try_for_each(|path| check_scope_path(path))
}

fn check_no_self_reference(pt: &PlannedTask) -> Result<()> {
    let self_reference = pt.depends_on.contains(&pt.id)
        || pt.read_after_write.contains(&pt.id)
        || pt.contradicts.contains(&pt.id);
    if self_reference {
        return Err(PlanError::Parse(format!(
            "task {} depends on itself",
            pt.id
        )));
    }
    Ok(())
}

/// `relation` completes the sentence "task <owner> <relation> unknown task <ref>".
fn check_known_refs(
    item_ids: &HashSet<&TaskId>,
    owner: &TaskId,
    refs: &[TaskId],
    relation: &str,
) -> Result<()> {
    for dep in refs {
        if !item_ids.contains(dep) {
            return Err(PlanError::Parse(format!(
                "task {owner} {relation} unknown task {dep}"
            )));
        }
    }
    Ok(())
}

fn check_scope_path(path: &str) -> Result<()> {
    if path.is_empty() {
        return Err(PlanError::InvalidScope("empty scope path".to_string()));
    }
    if path.contains("..") {
        return Err(PlanError::InvalidScope(format!(
            "scope path contains '..': {path}"
        )));
    }
    if path.starts_with('/') {
        return Err(PlanError::InvalidScope(format!(
            "scope path is absolute: {path}"
        )));
    }
    Ok(())
}

fn check_all_items_planned(request: &PlanRequest, seen_ids: &HashSet<&TaskId>) -> Result<()> {
    for item in &request.items {
        if !seen_ids.contains(&item.id) {
            return Err(PlanError::Parse(format!(
                "backlog item {} missing from plan",
                item.id
            )));
        }
    }
    Ok(())
}

/// Check for cycles using `TaskGraph`.
fn check_acyclic(plan: &Plan) -> Result<()> {
    let tasks: Vec<Task> = plan
        .tasks
        .iter()
        .map(|pt| {
            let mut task = Task::new(pt.id.clone(), String::new(), String::new());
            task.tier = pt.tier;
            task
        })
        .collect();
    let deps: Vec<Dependency> = plan.tasks.iter().flat_map(dependencies_of).collect();

    let Err(err) = TaskGraph::new(&tasks, &deps) else {
        return Ok(());
    };
    if let PlanError::Cycle(ids) = err {
        return Err(PlanError::Parse(format!("dependency cycle: {ids:?}")));
    }
    Err(err)
}
