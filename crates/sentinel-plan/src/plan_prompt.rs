//! The prompt of the planner: the shape of a plan, the rules, the repository files, and the backlog.

use crate::backlog::BacklogItem;
use crate::planner::PlanRequest;

const PLAN_SHAPE: &str = r#"{
  "tasks": [
    {
      "id": "T-01",
      "scope": ["src/file.rs"],
      "depends_on": [],
      "read_after_write": [],
      "tier": "cheap",
      "already_done": false,
      "contradicts": [],
      "note": ""
    }
  ]
}"#;

const PLANNER_RULES: &[&str] = &[
    "If the message is not a request to change the project (a greeting, a question, thanks), do not write a plan. Answer it in one to four plain sentences instead, and put no JSON in the answer.",
    "Every backlog item must have exactly one planned task. Write each task id exactly as the backlog writes it.",
    "Scope entries are non-empty relative paths without '..'.",
    "Scope lists the files the task may write. Never list a test file that already exists: editing an existing test fails the task.",
    "Give each task one NEW test file for its own tests, named for the task and not in the repository files, for example tests/health.rs.",
    "depends_on, read_after_write, contradicts name known task ids and never the task itself.",
    "No dependency cycles.",
    "Tier is one of: cheap, mid, smart.",
    "If a task claims done, set already_done to true. The claim may be wrong, so its scope still lists the files that implement the feature, plus one new test file.",
    "Two tasks with identical titles (ignoring case) should contradict each other.",
    "A task that needs something another task creates, such as an id, a field, an endpoint or a function, names that task in depends_on.",
    "A task that must read the final text of a file another task writes names that task in read_after_write.",
    "Tasks that list the same file run one after the other on their own. Do not add depends_on only because two tasks share a file.",
    "Scope must list every file the task has to change, including route tables, wiring and config files. A file that is missing from the scope blocks the task and loses its work. A file that two tasks list only makes them run one after the other, so when unsure, list the file.",
    "If two tasks ask for things that cannot both be true, for example one removes a charge and the other sets a minimum for that charge, list each in the other's contradicts and give one sentence of reason per task in note.",
];

/// Generate the prompt for the planner model. The planner may answer a message that is not a
/// request to change the project with plain sentences.
pub fn planner_prompt(request: &PlanRequest) -> String {
    planner_prompt_for(request, true)
}

/// The same prompt, with or without the way out of a plan. A backlog is always a request to change
/// the project, so for a backlog the model must answer with a plan: given the way out, a model
/// may say that no request came through.
pub fn planner_prompt_for(request: &PlanRequest, allow_reply: bool) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are a task planner for Sentinel, a multi-agent coding system.\n");
    prompt.push_str("Given a backlog and a list of repository files, produce a JSON plan.\n\n");
    push_repo_files(&mut prompt, &request.repo_files);
    push_backlog(&mut prompt, &request.items);

    prompt.push_str(if allow_reply {
        "For a request to change the project, output ONE JSON object with this shape (no prose, no code fences). Otherwise answer in plain sentences:\n"
    } else {
        "The backlog above is the request. Output ONE JSON object with this shape and nothing else (no prose, no code fences):\n"
    });
    let example = request
        .items
        .first()
        .map_or("T-01", |item| item.id.as_str());
    prompt.push_str(&PLAN_SHAPE.replace("T-01", example));
    prompt.push_str("\n\nRules:\n");
    for rule in PLANNER_RULES.iter().skip(usize::from(!allow_reply)) {
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
