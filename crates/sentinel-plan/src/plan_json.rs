//! Reading a plan out of what a model answered: cutting the JSON out of prose, finding the `tasks`
//! list, and giving the tasks the ids of the backlog.

use std::collections::HashMap;

use sentinel_core::TaskId;

use crate::plan_check::validate_plan;
use crate::planner::{Plan, PlanRequest};
use crate::{PlanError, Result};

/// Parse a JSON plan from model output (may be wrapped in prose or code fences).
pub fn parse_plan_json(text: &str, request: &PlanRequest) -> Result<Plan> {
    let json_text = first_json_object(text)?;
    let value: serde_json::Value = serde_json::from_str(json_text)?;
    let Some(found) = plan_in(&value) else {
        return Err(PlanError::Parse(format!(
            "no \"tasks\" list in the JSON, which has the keys {}",
            top_keys(&value)
        )));
    };
    let plan: Plan = serde_json::from_value(found.clone())?;
    let plan = match_ids(plan, request);
    validate_plan(&plan, request)?;
    Ok(plan)
}

/// A key that ignores case and the dashes and underscores of an id, so `T-01` and `T01` match.
fn id_key(id: &TaskId) -> String {
    id.as_str()
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Gives every id of the plan the spelling of the backlog. A model that copies the example id
/// `T-01` for a backlog that writes `T01` still names the same tasks.
fn match_ids(mut plan: Plan, request: &PlanRequest) -> Plan {
    let real: HashMap<String, TaskId> = request
        .items
        .iter()
        .map(|item| (id_key(&item.id), item.id.clone()))
        .collect();
    let fix = |id: &TaskId| real.get(&id_key(id)).cloned().unwrap_or_else(|| id.clone());
    for task in &mut plan.tasks {
        task.id = fix(&task.id);
        for list in [
            &mut task.depends_on,
            &mut task.read_after_write,
            &mut task.contradicts,
        ] {
            *list = list.iter().map(&fix).collect();
        }
    }
    plan
}

/// The object that holds the `tasks` list: the value itself, or an object inside it, such as
/// `{"plan": {"tasks": [...]}}`.
fn plan_in(value: &serde_json::Value) -> Option<&serde_json::Value> {
    if value.get("tasks").is_some_and(serde_json::Value::is_array) {
        return Some(value);
    }
    value.as_object()?.values().find_map(plan_in)
}

fn top_keys(value: &serde_json::Value) -> String {
    value.as_object().map_or_else(
        || "none".to_owned(),
        |map| map.keys().cloned().collect::<Vec<_>>().join(", "),
    )
}

/// True when the text carries a plan's `tasks` key. A broken plan must never reach the chat as a
/// reply, so a planner answer with this key is not a reply even when the plan does not parse.
pub fn holds_plan_json(text: &str) -> bool {
    text.contains("\"tasks\"")
}

/// The first `{ ... }` block in `text` that holds a `tasks` key, braces matched. Braces inside
/// quoted strings do not count, and a block of prose braces before the plan is skipped. Without a
/// block that holds `tasks`, the first block is returned so the error says what was wrong with it.
pub(crate) fn first_json_object(text: &str) -> Result<&str> {
    let mut first: Option<&str> = None;
    for (start, _) in text.match_indices('{') {
        let Some(len) = object_len(&text[start..]) else {
            continue;
        };
        let block = &text[start..start + len];
        if holds_tasks_key(block) {
            return Ok(block);
        }
        first.get_or_insert(block);
    }
    first.ok_or_else(|| {
        let message = if text.contains('{') {
            "unclosed JSON object"
        } else {
            "no JSON object found"
        };
        PlanError::Parse(message.to_string())
    })
}

fn holds_tasks_key(block: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(block).is_ok_and(|value| plan_in(&value).is_some())
}

/// Byte length of the object at the start of `text`, which must begin with `{`. A brace inside a
/// quoted string, with its escapes, is text and not part of the structure.
fn object_len(text: &str) -> Option<usize> {
    let mut scan = Scan::default();
    for (i, ch) in text.char_indices() {
        if scan.step(ch)? {
            return Some(i + 1);
        }
    }
    None
}

/// The state of a scan over JSON text: the depth of braces, and whether a string is open.
#[derive(Default)]
struct Scan {
    depth: usize,
    in_string: bool,
    escaped: bool,
}

impl Scan {
    /// Reads one character. `Some(true)` means the object just closed, and `None` means the text
    /// closes a brace that was never opened.
    fn step(&mut self, ch: char) -> Option<bool> {
        if self.in_string {
            self.in_string = !self.string_ends(ch);
            return Some(false);
        }
        match ch {
            '"' => self.in_string = true,
            '{' => self.depth += 1,
            '}' => {
                self.depth = self.depth.checked_sub(1)?;
                return Some(self.depth == 0);
            }
            _ => {}
        }
        Some(false)
    }

    /// True when `ch` closes the open string. A backslash makes the next character plain text.
    fn string_ends(&mut self, ch: char) -> bool {
        if self.escaped {
            self.escaped = false;
            return false;
        }
        match ch {
            '\\' => {
                self.escaped = true;
                false
            }
            '"' => true,
            _ => false,
        }
    }
}
