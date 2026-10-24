//! Planner contract: request/response, prompt, validation, backends.

#![allow(clippy::too_many_lines)]
#![allow(clippy::format_push_string)]
#![allow(clippy::single_char_add_str)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::manual_string_new)]
#![allow(clippy::wildcard_enum_match_arm)]

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;

use crate::{PlanError, Result, backlog::BacklogItem, graph::TaskGraph};
use futures::FutureExt;
use sentinel_core::{Dependency, DependencyKind, Scope, Task, TaskFlags, TaskId, Tier};
use serde::{Deserialize, Serialize};

/// Request sent to the planner backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanRequest {
    pub items: Vec<BacklogItem>,
    pub repo_files: Vec<String>,
}

/// A planned task returned by the planner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedTask {
    pub id: TaskId,
    pub scope: Vec<String>,
    pub depends_on: Vec<TaskId>,
    pub read_after_write: Vec<TaskId>,
    pub tier: Tier,
    #[serde(default)]
    pub already_done: bool,
    #[serde(default)]
    pub contradicts: Vec<TaskId>,
}

/// The full plan response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub tasks: Vec<PlannedTask>,
}

/// Trait for planner backends (model-based or heuristic).
pub trait PlannerBackend: Send + Sync {
    fn plan<'a>(
        &'a self,
        request: &'a PlanRequest,
    ) -> Pin<Box<dyn Future<Output = Result<Plan>> + Send + 'a>>;
}

/// Generate the prompt for the planner model.
pub fn planner_prompt(request: &PlanRequest) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are a task planner for Sentinel, a multi-agent coding system.\n");
    prompt.push_str("Given a backlog and a list of repository files, produce a JSON plan.\n\n");

    prompt.push_str("Repository files:\n");
    for f in &request.repo_files {
        prompt.push_str(&format!("  {}\n", f));
    }
    prompt.push_str("\n");

    prompt.push_str("Backlog tasks:\n");
    for item in &request.items {
        prompt.push_str(&format!("  {}: {}\n", item.id, item.title));
        if !item.body.is_empty() {
            prompt.push_str(&format!("    Body: {}\n", item.body));
        }
        if item.claims_done {
            prompt.push_str("    (claims_done: true)\n");
        }
    }
    prompt.push_str("\n");

    prompt.push_str("Output ONE JSON object with this shape (no prose, no code fences):\n");
    prompt.push_str(
        r#"{
  "tasks": [
    {
      "id": "T-01",
      "scope": ["src/file.rs"],
      "depends_on": [],
      "read_after_write": [],
      "tier": "cheap",
      "already_done": false,
      "contradicts": []
    }
  ]
}"#,
    );
    prompt.push_str("\n\nRules:\n");
    prompt.push_str("- Every backlog item must have exactly one planned task.\n");
    prompt.push_str("- Scope entries are non-empty relative paths without '..'.\n");
    prompt.push_str("- depends_on, read_after_write, contradicts name known task ids and never the task itself.\n");
    prompt.push_str("- No dependency cycles.\n");
    prompt.push_str("- Tier is one of: cheap, mid, smart.\n");
    prompt.push_str("- If a task claims done, set already_done to true.\n");
    prompt.push_str(
        "- Two tasks with identical titles (ignoring case) should contradict each other.\n",
    );

    prompt
}

/// Parse a JSON plan from model output (may be wrapped in prose or code fences).
pub fn parse_plan_json(text: &str, request: &PlanRequest) -> Result<Plan> {
    // Find the first JSON object in the text
    let json_start = text
        .find('{')
        .ok_or_else(|| PlanError::Parse("no JSON object found".to_string()))?;

    // Find the matching closing brace
    let mut depth = 0;
    let mut json_end = None;
    for (i, ch) in text[json_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    json_end = Some(json_start + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }

    let json_end = json_end.ok_or_else(|| PlanError::Parse("unclosed JSON object".to_string()))?;
    let json_text = &text[json_start..json_end];

    let plan: Plan = serde_json::from_str(json_text)?;
    validate_plan(&plan, request)?;
    Ok(plan)
}

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

        // Check self-dependencies
        if pt.depends_on.contains(&pt.id)
            || pt.read_after_write.contains(&pt.id)
            || pt.contradicts.contains(&pt.id)
        {
            return Err(PlanError::Parse(format!(
                "task {} depends on itself",
                pt.id
            )));
        }

        // Check dependencies name known tasks
        for dep in &pt.depends_on {
            if !item_ids.contains(dep) {
                return Err(PlanError::Parse(format!(
                    "task {} depends on unknown task {}",
                    pt.id, dep
                )));
            }
        }
        for dep in &pt.read_after_write {
            if !item_ids.contains(dep) {
                return Err(PlanError::Parse(format!(
                    "task {} has read_after_write to unknown task {}",
                    pt.id, dep
                )));
            }
        }
        for c in &pt.contradicts {
            if !item_ids.contains(c) {
                return Err(PlanError::Parse(format!(
                    "task {} contradicts unknown task {}",
                    pt.id, c
                )));
            }
        }

        // Validate scope entries
        for scope_path in &pt.scope {
            if scope_path.is_empty() {
                return Err(PlanError::InvalidScope("empty scope path".to_string()));
            }
            if scope_path.contains("..") {
                return Err(PlanError::InvalidScope(format!(
                    "scope path contains '..': {}",
                    scope_path
                )));
            }
            if scope_path.starts_with('/') {
                return Err(PlanError::InvalidScope(format!(
                    "scope path is absolute: {}",
                    scope_path
                )));
            }
        }
    }

    // Check no missing tasks
    for item in &request.items {
        if !seen_ids.contains(&item.id) {
            return Err(PlanError::Parse(format!(
                "backlog item {} missing from plan",
                item.id
            )));
        }
    }

    // Check for cycles using TaskGraph
    let tasks: Vec<Task> = plan
        .tasks
        .iter()
        .map(|pt| {
            let mut task = Task::new(pt.id.clone(), "".to_string(), "".to_string());
            task.tier = pt.tier;
            task
        })
        .collect();

    let deps: Vec<Dependency> = plan
        .tasks
        .iter()
        .flat_map(|pt| {
            pt.depends_on
                .iter()
                .map(move |d| Dependency {
                    task: pt.id.clone(),
                    depends_on: d.clone(),
                    kind: DependencyKind::Order,
                })
                .chain(pt.read_after_write.iter().map(move |d| Dependency {
                    task: pt.id.clone(),
                    depends_on: d.clone(),
                    kind: DependencyKind::ReadAfterWrite,
                }))
        })
        .collect();

    TaskGraph::new(&tasks, &deps).map_err(|e| match e {
        PlanError::Cycle(ids) => PlanError::Parse(format!("dependency cycle: {:?}", ids)),
        other => other,
    })?;

    Ok(())
}

/// Build sentinel-core Task and Dependency from a validated plan.
pub fn build_tasks(request: &PlanRequest, plan: &Plan) -> Result<(Vec<Task>, Vec<Dependency>)> {
    let mut tasks = Vec::new();
    let mut deps = Vec::new();

    let item_map: HashMap<&TaskId, &BacklogItem> =
        request.items.iter().map(|i| (&i.id, i)).collect();

    for pt in &plan.tasks {
        let item = item_map
            .get(&pt.id)
            .ok_or_else(|| PlanError::UnknownTask(pt.id.to_string()))?;

        let scope =
            Scope::from_paths(&pt.scope).map_err(|e| PlanError::InvalidScope(e.to_string()))?;

        let mut task = Task::new(pt.id.clone(), item.title.clone(), item.body.clone());
        task.status = sentinel_core::TaskStatus::Planned;
        task.tier = pt.tier;
        task.scope = scope;
        task.flags = TaskFlags {
            already_done: pt.already_done,
            contradicts: pt.contradicts.clone(),
        };
        tasks.push(task);

        for dep in &pt.depends_on {
            deps.push(Dependency {
                task: pt.id.clone(),
                depends_on: dep.clone(),
                kind: DependencyKind::Order,
            });
        }
        for dep in &pt.read_after_write {
            deps.push(Dependency {
                task: pt.id.clone(),
                depends_on: dep.clone(),
                kind: DependencyKind::ReadAfterWrite,
            });
        }
    }

    Ok((tasks, deps))
}

/// Heuristic planner: no model, used for tests and --fake-agents.
pub struct HeuristicPlanner;

impl PlannerBackend for HeuristicPlanner {
    fn plan<'a>(
        &'a self,
        request: &'a PlanRequest,
    ) -> Pin<Box<dyn Future<Output = Result<Plan>> + Send + 'a>> {
        async move {
            let mut planned = Vec::new();
            let mut title_map: HashMap<String, Vec<TaskId>> = HashMap::new();

            // First pass: collect titles for contradiction detection
            for item in &request.items {
                let lower_title = item.title.to_lowercase();
                title_map
                    .entry(lower_title)
                    .or_default()
                    .push(item.id.clone());
            }

            // Find contradicting titles
            let mut contradicts_map: HashMap<TaskId, Vec<TaskId>> = HashMap::new();
            for (_, ids) in title_map {
                if ids.len() > 1 {
                    for id in &ids {
                        let mut others = ids.clone();
                        others.retain(|x| x != id);
                        contradicts_map.insert(id.clone(), others);
                    }
                }
            }

            for item in &request.items {
                // Scope: repo files whose filename (without extension) appears as a word in title or body
                let mut scope = Vec::new();
                let text = format!("{} {}", item.title, item.body).to_lowercase();
                for file in &request.repo_files {
                    let file_name = std::path::Path::new(file)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if !file_name.is_empty() && text.split_whitespace().any(|w| w == file_name) {
                        scope.push(file.clone());
                    }
                }

                // Tier by body length
                let tier = if item.body.len() < 200 {
                    Tier::Cheap
                } else if item.body.len() < 600 {
                    Tier::Mid
                } else {
                    Tier::Smart
                };

                planned.push(PlannedTask {
                    id: item.id.clone(),
                    scope,
                    depends_on: Vec::new(),
                    read_after_write: Vec::new(),
                    tier,
                    already_done: item.claims_done,
                    contradicts: contradicts_map.get(&item.id).cloned().unwrap_or_default(),
                });
            }

            Ok(Plan { tasks: planned })
        }
        .boxed()
    }
}

/// Fixed planner: returns a stored plan, for tests.
pub struct FixedPlanner {
    pub plan: Plan,
}

impl PlannerBackend for FixedPlanner {
    fn plan<'a>(
        &'a self,
        _request: &'a PlanRequest,
    ) -> Pin<Box<dyn Future<Output = Result<Plan>> + Send + 'a>> {
        async move { Ok(self.plan.clone()) }.boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backlog::BacklogItem;

    fn make_request(items: Vec<BacklogItem>, repo_files: Vec<String>) -> PlanRequest {
        PlanRequest { items, repo_files }
    }

    fn make_item(id: &str, title: &str, body: &str, claims_done: bool) -> BacklogItem {
        BacklogItem {
            id: TaskId::new(id),
            title: title.to_string(),
            body: body.to_string(),
            claims_done,
        }
    }

    #[test]
    fn parse_plan_json_handles_prose_around_json() {
        let text = r#"Here is the plan:
{
  "tasks": [
    {
      "id": "T-01",
      "scope": [],
      "depends_on": [],
      "read_after_write": [],
      "tier": "cheap",
      "already_done": false,
      "contradicts": []
    }
  ]
}
That was the plan."#;
        let request = make_request(vec![make_item("T-01", "Task", "Body", false)], vec![]);
        let plan = parse_plan_json(text, &request).unwrap();
        assert_eq!(plan.tasks.len(), 1);
        assert_eq!(plan.tasks[0].id.as_str(), "T-01");
    }

    #[test]
    fn parse_plan_json_handles_fenced_block() {
        let text = r#"```json
{
  "tasks": [
    {
      "id": "T-01",
      "scope": [],
      "depends_on": [],
      "read_after_write": [],
      "tier": "cheap",
      "already_done": false,
      "contradicts": []
    }
  ]
}
```"#;
        let request = make_request(vec![make_item("T-01", "Task", "Body", false)], vec![]);
        let plan = parse_plan_json(text, &request).unwrap();
        assert_eq!(plan.tasks.len(), 1);
    }

    #[test]
    fn validate_rejects_missing_task() {
        let plan = Plan {
            tasks: vec![PlannedTask {
                id: TaskId::new("T-01"),
                scope: vec![],
                depends_on: vec![],
                read_after_write: vec![],
                tier: Tier::Cheap,
                already_done: false,
                contradicts: vec![],
            }],
        };
        let request = make_request(
            vec![
                make_item("T-01", "Task", "Body", false),
                make_item("T-02", "Task2", "Body", false),
            ],
            vec![],
        );
        let err = validate_plan(&plan, &request).unwrap_err();
        assert!(matches!(err, PlanError::Parse(s) if s.contains("missing from plan")));
    }

    #[test]
    fn validate_rejects_unknown_dependency() {
        let plan = Plan {
            tasks: vec![PlannedTask {
                id: TaskId::new("T-01"),
                scope: vec![],
                depends_on: vec![TaskId::new("T-99")],
                read_after_write: vec![],
                tier: Tier::Cheap,
                already_done: false,
                contradicts: vec![],
            }],
        };
        let request = make_request(vec![make_item("T-01", "Task", "Body", false)], vec![]);
        let err = validate_plan(&plan, &request).unwrap_err();
        assert!(matches!(err, PlanError::Parse(s) if s.contains("unknown task")));
    }

    #[test]
    fn validate_rejects_self_dependency() {
        let plan = Plan {
            tasks: vec![PlannedTask {
                id: TaskId::new("T-01"),
                scope: vec![],
                depends_on: vec![TaskId::new("T-01")],
                read_after_write: vec![],
                tier: Tier::Cheap,
                already_done: false,
                contradicts: vec![],
            }],
        };
        let request = make_request(vec![make_item("T-01", "Task", "Body", false)], vec![]);
        let err = validate_plan(&plan, &request).unwrap_err();
        assert!(matches!(err, PlanError::Parse(s) if s.contains("depends on itself")));
    }

    #[test]
    fn validate_rejects_scope_with_parent() {
        let plan = Plan {
            tasks: vec![PlannedTask {
                id: TaskId::new("T-01"),
                scope: vec!["src/../foo.rs".to_string()],
                depends_on: vec![],
                read_after_write: vec![],
                tier: Tier::Cheap,
                already_done: false,
                contradicts: vec![],
            }],
        };
        let request = make_request(vec![make_item("T-01", "Task", "Body", false)], vec![]);
        let err = validate_plan(&plan, &request).unwrap_err();
        assert!(matches!(err, PlanError::InvalidScope(_)));
    }

    #[test]
    fn validate_rejects_cycle() {
        let plan = Plan {
            tasks: vec![
                PlannedTask {
                    id: TaskId::new("T-01"),
                    scope: vec![],
                    depends_on: vec![TaskId::new("T-02")],
                    read_after_write: vec![],
                    tier: Tier::Cheap,
                    already_done: false,
                    contradicts: vec![],
                },
                PlannedTask {
                    id: TaskId::new("T-02"),
                    scope: vec![],
                    depends_on: vec![TaskId::new("T-01")],
                    read_after_write: vec![],
                    tier: Tier::Cheap,
                    already_done: false,
                    contradicts: vec![],
                },
            ],
        };
        let request = make_request(
            vec![
                make_item("T-01", "Task", "Body", false),
                make_item("T-02", "Task", "Body", false),
            ],
            vec![],
        );
        let err = validate_plan(&plan, &request).unwrap_err();
        assert!(matches!(err, PlanError::Parse(s) if s.contains("cycle")));
    }

    #[test]
    fn build_tasks_sets_status_tier_scope_and_dep_kinds() {
        let plan = Plan {
            tasks: vec![
                PlannedTask {
                    id: TaskId::new("T-01"),
                    scope: vec!["src/a.rs".to_string()],
                    depends_on: vec![TaskId::new("T-02")],
                    read_after_write: vec![TaskId::new("T-03")],
                    tier: Tier::Mid,
                    already_done: true,
                    contradicts: vec![],
                },
                PlannedTask {
                    id: TaskId::new("T-02"),
                    scope: vec!["src/b.rs".to_string()],
                    depends_on: vec![],
                    read_after_write: vec![],
                    tier: Tier::Cheap,
                    already_done: false,
                    contradicts: vec![],
                },
                PlannedTask {
                    id: TaskId::new("T-03"),
                    scope: vec!["src/c.rs".to_string()],
                    depends_on: vec![],
                    read_after_write: vec![],
                    tier: Tier::Smart,
                    already_done: false,
                    contradicts: vec![],
                },
            ],
        };
        let request = make_request(
            vec![
                make_item("T-01", "Task 1", "Body 1", true),
                make_item("T-02", "Task 2", "Body 2", false),
                make_item("T-03", "Task 3", "Body 3", false),
            ],
            vec![
                "src/a.rs".to_string(),
                "src/b.rs".to_string(),
                "src/c.rs".to_string(),
            ],
        );
        let (tasks, deps) = build_tasks(&request, &plan).unwrap();

        assert_eq!(tasks.len(), 3);
        let t1 = tasks.iter().find(|t| t.id.as_str() == "T-01").unwrap();
        assert_eq!(t1.status, sentinel_core::TaskStatus::Planned);
        assert_eq!(t1.tier, Tier::Mid);
        assert!(t1.flags.already_done);
        assert_eq!(t1.scope.entries().collect::<Vec<_>>(), vec!["src/a.rs"]);

        let dep_order: Vec<_> = deps
            .iter()
            .filter(|d| d.kind == DependencyKind::Order)
            .collect();
        let dep_raw: Vec<_> = deps
            .iter()
            .filter(|d| d.kind == DependencyKind::ReadAfterWrite)
            .collect();
        assert_eq!(dep_order.len(), 1);
        assert_eq!(dep_raw.len(), 1);
        assert_eq!(dep_order[0].task.as_str(), "T-01");
        assert_eq!(dep_order[0].depends_on.as_str(), "T-02");
        assert_eq!(dep_raw[0].task.as_str(), "T-01");
        assert_eq!(dep_raw[0].depends_on.as_str(), "T-03");
    }

    #[test]
    fn heuristic_planner_ties_scope_to_matching_file_names() {
        let request = make_request(
            vec![make_item(
                "T-01",
                "Add rate limit",
                "Implement rate limiting in rate.rs",
                false,
            )],
            vec!["src/rate.rs".to_string(), "src/other.rs".to_string()],
        );
        let planner = HeuristicPlanner;
        let plan = futures::executor::block_on(planner.plan(&request)).unwrap();
        assert_eq!(plan.tasks[0].scope, vec!["src/rate.rs"]);
    }

    #[test]
    fn heuristic_planner_tiers_by_body_length() {
        let short = make_item("T-01", "Short", &"a".repeat(100), false);
        let mid = make_item("T-02", "Mid", &"b".repeat(400), false);
        let smart = make_item("T-03", "Smart", &"c".repeat(800), false);
        let request = make_request(vec![short, mid, smart], vec![]);
        let planner = HeuristicPlanner;
        let plan = futures::executor::block_on(planner.plan(&request)).unwrap();
        assert_eq!(plan.tasks[0].tier, Tier::Cheap);
        assert_eq!(plan.tasks[1].tier, Tier::Mid);
        assert_eq!(plan.tasks[2].tier, Tier::Smart);
    }

    #[test]
    fn heuristic_planner_flags_identical_titles_as_contradicting() {
        let request = make_request(
            vec![
                make_item("T-01", "Add rate limit", "Body one", false),
                make_item("T-02", "Add Rate Limit", "Body two", false),
                make_item("T-03", "Different task", "Body three", false),
            ],
            vec![],
        );
        let planner = HeuristicPlanner;
        let plan = futures::executor::block_on(planner.plan(&request)).unwrap();
        let t1 = plan.tasks.iter().find(|t| t.id.as_str() == "T-01").unwrap();
        let t2 = plan.tasks.iter().find(|t| t.id.as_str() == "T-02").unwrap();
        let t3 = plan.tasks.iter().find(|t| t.id.as_str() == "T-03").unwrap();
        assert_eq!(t1.contradicts, vec![TaskId::new("T-02")]);
        assert_eq!(t2.contradicts, vec![TaskId::new("T-01")]);
        assert_eq!(t3.contradicts, Vec::<TaskId>::new());
    }

    #[test]
    fn heuristic_planner_full_integration() {
        let backlog = r"
## T-01 Add rate limit
Short body.

## T-02 Add rate limit
This task also adds a rate limit but differently. It has a longer body that should make it mid tier at least. This body is intentionally long enough to exceed the 200 character threshold for the mid tier classification in the heuristic planner.
";
        let items = crate::backlog::parse_backlog(backlog).unwrap();
        let request = make_request(items, vec!["src/rate.rs".to_string()]);
        let planner = HeuristicPlanner;
        let plan = futures::executor::block_on(planner.plan(&request)).unwrap();

        // Both tasks contradict each other (identical title ignoring case)
        let t1 = plan.tasks.iter().find(|t| t.id.as_str() == "T-01").unwrap();
        let t2 = plan.tasks.iter().find(|t| t.id.as_str() == "T-02").unwrap();
        assert_eq!(t1.contradicts, vec![TaskId::new("T-02")]);
        assert_eq!(t2.contradicts, vec![TaskId::new("T-01")]);

        // Tier by body length
        assert_eq!(t1.tier, Tier::Cheap);
        assert_eq!(t2.tier, Tier::Mid);

        // Scope matches file name
        assert_eq!(t1.scope, vec!["src/rate.rs"]);
        assert_eq!(t2.scope, vec!["src/rate.rs"]);

        // find_contradictions should find one pair
        let pairs = crate::contradiction::find_contradictions(&plan);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].a.as_str(), "T-01");
        assert_eq!(pairs[0].b.as_str(), "T-02");
    }
}
