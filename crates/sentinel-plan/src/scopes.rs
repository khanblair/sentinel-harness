//! Scopes of planned tasks: the files a task's text names, a folder when it names none, and an
//! order for tasks that write the same file.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use sentinel_core::{Scope, TaskId};

use crate::planner::{Plan, PlanRequest, PlannedTask, text_of};

/// Words and path parts shorter than this many letters never name a file.
const MIN_WORD_LETTERS: usize = 3;
/// A task's text names at most this many files.
const MAX_SCOPE_FILES: usize = 4;
/// Top-level folders that never hold the source a task changes.
const NON_SOURCE_FOLDERS: [&str; 4] = ["test", "tests", "docs", "scripts"];

/// Repo files whose path parts match the words of `text`. A word matches a part with or without a
/// plural `s` or `es`. Files with more matching words come first, and at most four are kept.
pub fn scope_from_text(text: &str, repo_files: &[String]) -> Vec<String> {
    let words: Vec<BTreeSet<String>> = words_of(text).iter().map(|word| word_forms(word)).collect();
    let mut ranked: Vec<(usize, &String)> = repo_files
        .iter()
        .filter(|file| !is_test_path(file))
        .filter_map(|file| {
            let parts = path_forms(file);
            let score = words
                .iter()
                .filter(|forms| !forms.is_disjoint(&parts))
                .count();
            (score > 0).then_some((score, file))
        })
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    ranked
        .into_iter()
        .take(MAX_SCOPE_FILES)
        .map(|(_, file)| file.clone())
        .collect()
}

fn long_enough(text: &str) -> bool {
    text.chars().count() >= MIN_WORD_LETTERS
}

/// The distinct lower-case words of a text that are long enough to name a file.
fn words_of(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| long_enough(word))
        .map(str::to_lowercase)
        .collect()
}

/// A word as written and without a plural `s` or `es`. A stem that is too short is left out.
fn word_forms(word: &str) -> BTreeSet<String> {
    let mut forms = BTreeSet::from([word.to_owned()]);
    for suffix in ["es", "s"] {
        if let Some(stem) = word.strip_suffix(suffix).filter(|stem| long_enough(stem)) {
            forms.insert(stem.to_owned());
        }
    }
    forms
}

/// The word forms of every part of a path. `src/fees/schedule.js` gives `src`, `fees`, `fee`,
/// `schedule`. Splitting on any non-letter also splits `rate_limit.rs` into `rate` and `limit`.
fn path_forms(file: &str) -> BTreeSet<String> {
    file.split(|c: char| !c.is_alphanumeric())
        .filter(|part| long_enough(part))
        .flat_map(|part| word_forms(&part.to_lowercase()))
        .collect()
}

/// Test files are never a task's scope: editing an existing test fails the task.
fn is_test_path(file: &str) -> bool {
    let in_test_folder = file
        .split('/')
        .any(|part| matches!(part, "test" | "tests" | "acceptance" | "__tests__" | "spec"));
    let name = file.rsplit('/').next().unwrap_or(file).to_lowercase();
    in_test_folder
        || name.contains(".test.")
        || name.contains(".spec.")
        || name.contains("_test.")
        || name.starts_with("test_")
}

/// The top-level folder with the most repository files. Tests, docs, scripts, and hidden folders
/// are skipped. A tie goes to the name that sorts first. The result ends in `/`, so the scope
/// covers the whole folder.
pub fn common_source_folder(repo_files: &[String]) -> Option<String> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for file in repo_files {
        let Some((top, _)) = file.split_once('/') else {
            continue;
        };
        if is_source_folder(top) {
            *counts.entry(top).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))
        .map(|(top, _)| format!("{top}/"))
}

fn is_source_folder(name: &str) -> bool {
    !name.starts_with('.') && !NON_SOURCE_FOLDERS.contains(&name)
}

/// The path of a new test file for a task, named like the repository's own test files, or `None`
/// when the repository has no test file to copy the style of. A task needs a new test file in its
/// scope: without one it cannot prove its work, and the guard would undo a test written anyway.
pub fn new_test_path(repo_files: &[String], id: &TaskId) -> Option<String> {
    let example = repo_files
        .iter()
        .find(|file| is_test_path(file) && !file.starts_with("acceptance/"))?;
    let (folder, name) = example
        .rsplit_once('/')
        .map_or(("", example.as_str()), |(f, n)| (f, n));
    let lower_id = id.as_str().to_lowercase();
    let fresh = match name.split_once('.') {
        Some((stem, rest)) if rest.starts_with("test.") || rest.starts_with("spec.") => {
            let kind = rest.split('.').next().unwrap_or("test");
            let ext = rest.split_once('.').map_or("", |(_, ext)| ext);
            let _ = stem;
            format!("{lower_id}.{kind}.{ext}")
        }
        Some((stem, ext)) if stem.starts_with("test_") => format!("test_{lower_id}.{ext}"),
        Some((stem, ext)) if stem.ends_with("_test") => format!("{lower_id}_test.{ext}"),
        Some((_, ext)) => format!("{lower_id}.{ext}"),
        None => lower_id,
    };
    Some(if folder.is_empty() {
        fresh
    } else {
        format!("{folder}/{fresh}")
    })
}

/// Gives every task a new test file in its scope, when it has none and the repository shows how
/// test files are named. Returns the plan.
pub fn ensure_test_files(request: &PlanRequest, mut plan: Plan) -> Plan {
    for pt in &mut plan.tasks {
        if pt.scope.iter().any(|entry| is_test_path(entry)) {
            continue;
        }
        if let Some(path) = new_test_path(&request.repo_files, &pt.id) {
            pt.scope.push(path);
        }
    }
    plan
}

/// Gives every task with no scope the files its text names, or else the repository's main source
/// folder. Returns the plan and, for each task that got only the folder, its id and that folder.
pub fn fill_empty_scopes(request: &PlanRequest, mut plan: Plan) -> (Plan, Vec<(TaskId, String)>) {
    let folder = common_source_folder(&request.repo_files);
    let mut folder_only = Vec::new();
    for pt in plan.tasks.iter_mut().filter(|pt| pt.scope.is_empty()) {
        pt.scope = named_files(request, &pt.id);
        if !pt.scope.is_empty() {
            continue;
        }
        let Some(root) = &folder else {
            continue;
        };
        pt.scope = vec![root.clone()];
        folder_only.push((pt.id.clone(), root.clone()));
    }
    (plan, folder_only)
}

/// The repository files whose names the task's text matches, minus test files.
fn named_files(request: &PlanRequest, id: &TaskId) -> Vec<String> {
    let text = item_text(request, id);
    scope_from_text(&text, &request.repo_files)
        .into_iter()
        .filter(|file| !is_test_path(file))
        .collect()
}

fn item_text(request: &PlanRequest, id: &TaskId) -> String {
    request
        .items
        .iter()
        .find(|item| &item.id == id)
        .map_or_else(String::new, text_of)
}

/// Makes tasks that write the same file run in id order, unless one already waits for the other.
/// An edge that would close a cycle is left out.
pub fn order_shared_scopes(mut plan: Plan) -> Plan {
    for (first, later) in sharing_pairs(&plan) {
        if reaches(&plan, &first, &later) || reaches(&plan, &later, &first) {
            continue;
        }
        if let Some(task) = plan.tasks.iter_mut().find(|task| task.id == later) {
            task.depends_on.push(first);
        }
    }
    plan
}

/// Pairs of tasks whose scopes overlap, lower id first, in id order.
fn sharing_pairs(plan: &Plan) -> Vec<(TaskId, TaskId)> {
    let mut pairs = Vec::new();
    for (index, task) in plan.tasks.iter().enumerate() {
        for other in &plan.tasks[index + 1..] {
            if shares_scope(task, other) {
                pairs.push(lower_id_first(&task.id, &other.id));
            }
        }
    }
    pairs.sort();
    pairs
}

fn shares_scope(a: &PlannedTask, b: &PlannedTask) -> bool {
    let scope_a = Scope::from_paths(&a.scope).unwrap_or_default();
    let scope_b = Scope::from_paths(&b.scope).unwrap_or_default();
    scope_a.overlaps(&scope_b)
}

fn lower_id_first(a: &TaskId, b: &TaskId) -> (TaskId, TaskId) {
    if a < b {
        (a.clone(), b.clone())
    } else {
        (b.clone(), a.clone())
    }
}

/// True when `from` waits for `to`, directly or through other tasks.
fn reaches(plan: &Plan, from: &TaskId, to: &TaskId) -> bool {
    let mut stack = vec![from.clone()];
    let mut seen = HashSet::new();
    while let Some(current) = stack.pop() {
        if &current == to {
            return true;
        }
        if !seen.insert(current.clone()) {
            continue;
        }
        if let Some(task) = plan.tasks.iter().find(|task| task.id == current) {
            stack.extend(
                task.depends_on
                    .iter()
                    .chain(&task.read_after_write)
                    .cloned(),
            );
        }
    }
    false
}
