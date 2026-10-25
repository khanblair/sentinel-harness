//! Planner contract: request/response, prompt, validation, backends.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use crate::{PlanError, Result, backlog::BacklogItem};
use futures::FutureExt;
use sentinel_core::{Dependency, DependencyKind, Scope, Task, TaskFlags, TaskId, Tier};
use serde::{Deserialize, Serialize};

/// Bodies shorter than this many bytes plan at the cheap tier.
const CHEAP_BODY_BYTES: usize = 200;
/// Bodies shorter than this many bytes plan at the mid tier.
const MID_BODY_BYTES: usize = 600;

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
    #[serde(default)]
    pub scope: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<TaskId>,
    #[serde(default)]
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

const PLAN_SHAPE: &str = r#"{
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
}"#;

const PLANNER_RULES: [&str; 10] = [
    "If the message is not a request to change the project (a greeting, a question, thanks), do not write a plan. Answer it in one to four plain sentences instead, and put no JSON in the answer.",
    "Every backlog item must have exactly one planned task.",
    "Scope entries are non-empty relative paths without '..'.",
    "Scope lists the files the task may write. Never list a test file that already exists: editing an existing test fails the task.",
    "Give each task one NEW test file for its own tests, named for the task and not in the repository files, for example tests/health.rs.",
    "depends_on, read_after_write, contradicts name known task ids and never the task itself.",
    "No dependency cycles.",
    "Tier is one of: cheap, mid, smart.",
    "If a task claims done, set already_done to true.",
    "Two tasks with identical titles (ignoring case) should contradict each other.",
];

/// Generate the prompt for the planner model.
pub fn planner_prompt(request: &PlanRequest) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are a task planner for Sentinel, a multi-agent coding system.\n");
    prompt.push_str("Given a backlog and a list of repository files, produce a JSON plan.\n\n");
    push_repo_files(&mut prompt, &request.repo_files);
    push_backlog(&mut prompt, &request.items);

    prompt.push_str(
        "For a request to change the project, output ONE JSON object with this shape (no prose, no code fences). Otherwise answer in plain sentences:\n",
    );
    prompt.push_str(PLAN_SHAPE);
    prompt.push_str("\n\nRules:\n");
    for rule in PLANNER_RULES {
        prompt.push_str("- ");
        prompt.push_str(rule);
        prompt.push('\n');
    }
    prompt
}

fn push_line(prompt: &mut String, line: &str) {
    prompt.push_str(line);
    prompt.push('\n');
}

fn push_repo_files(prompt: &mut String, repo_files: &[String]) {
    prompt.push_str("Repository files:\n");
    for file in repo_files {
        push_line(prompt, &format!("  {file}"));
    }
    prompt.push('\n');
}

fn push_backlog(prompt: &mut String, items: &[BacklogItem]) {
    prompt.push_str("Backlog tasks:\n");
    for item in items {
        push_line(prompt, &format!("  {}: {}", item.id, item.title));
        if !item.body.is_empty() {
            push_line(prompt, &format!("    Body: {}", item.body));
        }
        if item.claims_done {
            push_line(prompt, "    (claims_done: true)");
        }
    }
    prompt.push('\n');
}

/// Parse a JSON plan from model output (may be wrapped in prose or code fences).
pub fn parse_plan_json(text: &str, request: &PlanRequest) -> Result<Plan> {
    let json_text = first_json_object(text)?;
    let plan: Plan = serde_json::from_str(json_text)?;
    validate_plan(&plan, request)?;
    Ok(plan)
}

/// True when the text carries a plan's `tasks` key. A broken plan must never reach the chat as a
/// reply, so a planner answer with this key is not a reply even when the plan does not parse.
pub fn holds_plan_json(text: &str) -> bool {
    text.contains("\"tasks\"")
}

/// The first `{ ... }` block in `text`, braces matched.
fn first_json_object(text: &str) -> Result<&str> {
    let start = text
        .find('{')
        .ok_or_else(|| PlanError::Parse("no JSON object found".to_string()))?;
    let len = object_len(&text[start..])
        .ok_or_else(|| PlanError::Parse("unclosed JSON object".to_string()))?;
    Ok(&text[start..start + len])
}

/// Byte length of the object at the start of `text`, which must begin with `{`.
fn object_len(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, ch) in text.char_indices() {
        depth = match ch {
            '{' => depth + 1,
            '}' => depth - 1,
            _ => depth,
        };
        if ch == '}' && depth == 0 {
            return Some(i + 1);
        }
    }
    None
}

pub use crate::plan_check::validate_plan;

/// Order edges, then read-after-write edges, for one planned task.
pub(crate) fn dependencies_of(pt: &PlannedTask) -> impl Iterator<Item = Dependency> + '_ {
    let ordered = pt.depends_on.iter().map(move |dep| Dependency {
        task: pt.id.clone(),
        depends_on: dep.clone(),
        kind: DependencyKind::Order,
    });
    let raw = pt.read_after_write.iter().map(move |dep| Dependency {
        task: pt.id.clone(),
        depends_on: dep.clone(),
        kind: DependencyKind::ReadAfterWrite,
    });
    ordered.chain(raw)
}

/// Build sentinel-core Task and Dependency from a validated plan.
pub fn build_tasks(request: &PlanRequest, plan: &Plan) -> Result<(Vec<Task>, Vec<Dependency>)> {
    let item_map: HashMap<&TaskId, &BacklogItem> =
        request.items.iter().map(|i| (&i.id, i)).collect();

    let tasks = plan
        .tasks
        .iter()
        .map(|pt| build_task(&item_map, pt))
        .collect::<Result<Vec<Task>>>()?;
    let deps = plan.tasks.iter().flat_map(dependencies_of).collect();
    Ok((tasks, deps))
}

fn build_task(item_map: &HashMap<&TaskId, &BacklogItem>, pt: &PlannedTask) -> Result<Task> {
    let item = item_map
        .get(&pt.id)
        .ok_or_else(|| PlanError::UnknownTask(pt.id.to_string()))?;
    let scope = Scope::from_paths(&pt.scope).map_err(|e| PlanError::InvalidScope(e.to_string()))?;

    let mut task = Task::new(pt.id.clone(), item.title.clone(), item.body.clone());
    task.status = sentinel_core::TaskStatus::Planned;
    task.tier = pt.tier;
    task.scope = scope;
    task.flags = TaskFlags {
        already_done: pt.already_done,
        contradicts: pt.contradicts.clone(),
    };
    Ok(task)
}

/// Heuristic planner: no model, used for tests.
pub struct HeuristicPlanner;

impl PlannerBackend for HeuristicPlanner {
    fn plan<'a>(
        &'a self,
        request: &'a PlanRequest,
    ) -> Pin<Box<dyn Future<Output = Result<Plan>> + Send + 'a>> {
        async move { Ok(heuristic_plan(request)) }.boxed()
    }
}

fn heuristic_plan(request: &PlanRequest) -> Plan {
    let contradicts = contradiction_map(&request.items);
    let tasks = request
        .items
        .iter()
        .map(|item| PlannedTask {
            id: item.id.clone(),
            scope: scope_from_text(item, &request.repo_files),
            depends_on: Vec::new(),
            read_after_write: Vec::new(),
            tier: tier_for_body(&item.body),
            already_done: item.claims_done,
            contradicts: contradicts.get(&item.id).cloned().unwrap_or_default(),
        })
        .collect();
    Plan { tasks }
}

/// Every item whose title matches another item's title (ignoring case) contradicts it.
fn contradiction_map(items: &[BacklogItem]) -> HashMap<TaskId, Vec<TaskId>> {
    let mut title_map: HashMap<String, Vec<TaskId>> = HashMap::new();
    for item in items {
        title_map
            .entry(item.title.to_lowercase())
            .or_default()
            .push(item.id.clone());
    }

    let mut contradicts = HashMap::new();
    for ids in title_map.into_values().filter(|ids| ids.len() > 1) {
        for id in &ids {
            let others: Vec<TaskId> = ids.iter().filter(|other| *other != id).cloned().collect();
            contradicts.insert(id.clone(), others);
        }
    }
    contradicts
}

/// Repo files whose name without extension is a word in the item's title or body.
fn scope_from_text(item: &BacklogItem, repo_files: &[String]) -> Vec<String> {
    let text = format!("{} {}", item.title, item.body).to_lowercase();
    let words: Vec<&str> = text.split_whitespace().collect();
    repo_files
        .iter()
        .filter(|file| {
            let stem = file_stem_lowercase(file);
            !stem.is_empty() && words.contains(&stem.as_str())
        })
        .cloned()
        .collect()
}

fn file_stem_lowercase(file: &str) -> String {
    std::path::Path::new(file)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn tier_for_body(body: &str) -> Tier {
    if body.len() < CHEAP_BODY_BYTES {
        Tier::Cheap
    } else if body.len() < MID_BODY_BYTES {
        Tier::Mid
    } else {
        Tier::Smart
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

    fn single_item_request() -> PlanRequest {
        make_request(vec![make_item("T-01", "Task", "Body", false)], vec![])
    }

    fn planned_task(id: &str, depends_on: &[&str]) -> PlannedTask {
        PlannedTask {
            id: TaskId::new(id),
            scope: vec![],
            depends_on: depends_on.iter().map(|dep| TaskId::new(*dep)).collect(),
            read_after_write: vec![],
            tier: Tier::Cheap,
            already_done: false,
            contradicts: vec![],
        }
    }

    #[test]
    fn a_greeting_answer_holds_no_plan_json() {
        assert!(!holds_plan_json("Hello! What would you like to build?"));
    }

    #[test]
    fn a_planner_answer_with_a_tasks_key_holds_plan_json() {
        assert!(holds_plan_json(r#"{"tasks": []}"#));
    }

    #[test]
    fn the_planner_prompt_tells_the_model_to_answer_a_greeting_without_a_plan() {
        let text = planner_prompt(&make_request(vec![], vec![]));
        assert!(text.contains("do not write a plan"), "{text}");
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
            tasks: vec![planned_task("T-01", &[])],
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
            tasks: vec![planned_task("T-01", &["T-99"])],
        };
        let request = single_item_request();
        let err = validate_plan(&plan, &request).unwrap_err();
        assert!(matches!(err, PlanError::Parse(s) if s.contains("unknown task")));
    }

    #[test]
    fn validate_rejects_self_dependency() {
        let plan = Plan {
            tasks: vec![planned_task("T-01", &["T-01"])],
        };
        let request = single_item_request();
        let err = validate_plan(&plan, &request).unwrap_err();
        assert!(matches!(err, PlanError::Parse(s) if s.contains("depends on itself")));
    }

    #[test]
    fn validate_rejects_scope_with_parent() {
        let plan = Plan {
            tasks: vec![PlannedTask {
                scope: vec!["src/../foo.rs".to_string()],
                ..planned_task("T-01", &[])
            }],
        };
        let request = single_item_request();
        let err = validate_plan(&plan, &request).unwrap_err();
        assert!(matches!(err, PlanError::InvalidScope(_)));
    }

    #[test]
    fn validate_rejects_cycle() {
        let plan = Plan {
            tasks: vec![
                planned_task("T-01", &["T-02"]),
                planned_task("T-02", &["T-01"]),
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

    fn fixture_plan() -> Plan {
        Plan {
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
        }
    }

    fn fixture_request() -> PlanRequest {
        make_request(
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
        )
    }

    #[test]
    fn build_tasks_sets_status_tier_scope_and_dep_kinds() {
        let request = fixture_request();
        let plan = fixture_plan();
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
