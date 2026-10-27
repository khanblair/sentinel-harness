//! The tables, bars, and status marks of the results page. Each function returns HTML. Text from
//! the run goes through `esc` before it reaches the page.

use sentinel_core::Cost;
use serde_json::Value;

use super::{TaskRow, esc};

/// What the page shows for a figure the evidence file does not have.
const NOT_KNOWN: &str = "not known";
/// The stop sign is a shape, drawn in CSS, so it reads without color.
const STOP_SIGN: &str = "<span class=\"stop-sign\"></span>";

/// The evidence value at `pointer`. `None` when there is no evidence or no such key.
pub(super) fn value_at<'a>(evidence: Option<&'a Value>, pointer: &str) -> Option<&'a Value> {
    evidence?.pointer(pointer)
}

pub(super) fn cost_of(value: Option<&Value>) -> Option<Cost> {
    value.and_then(|value| serde_json::from_value(value.clone()).ok())
}

/// Dollars as the page shows them. An amount Sentinel guessed says so.
pub(super) fn money_text(cost: Cost) -> String {
    if cost.is_estimated() {
        format!("{cost} est")
    } else {
        cost.to_string()
    }
}

pub(super) fn count_shown(value: Option<&Value>) -> String {
    match value.and_then(Value::as_u64) {
        Some(count) => count.to_string(),
        None => NOT_KNOWN.to_owned(),
    }
}

pub(super) fn share_shown(value: Option<&Value>) -> String {
    match value.and_then(Value::as_f64) {
        Some(share) => format!("{share:.2}"),
        None => NOT_KNOWN.to_owned(),
    }
}

fn money_shown(value: Option<&Value>) -> String {
    cost_of(value).map_or_else(|| NOT_KNOWN.to_owned(), money_text)
}

fn yes_no(value: Option<&Value>) -> String {
    let text = match value.and_then(Value::as_bool) {
        Some(true) => "yes",
        Some(false) => "no",
        None => "not checked",
    };
    text.to_owned()
}

fn text_of<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// The list at `pointer`, or an empty list when the evidence has none.
fn items<'a>(evidence: &'a Value, pointer: &str) -> &'a [Value] {
    match evidence.pointer(pointer).and_then(Value::as_array) {
        Some(list) => list.as_slice(),
        None => &[],
    }
}

/// A status as a shape and a word. The class sets the color, and the word says the rest.
fn status_html(status: &str) -> String {
    let (class, mark) = status_shape(status);
    let word = esc(status);
    format!(
        "<span class=\"status status--{class}\"><span class=\"status__mark\" aria-hidden=\"true\">{mark}</span>{word}</span>"
    )
}

fn status_shape(status: &str) -> (&'static str, &'static str) {
    match status {
        "Verified" => ("verified", "&#10003;"),
        "Partial" => ("partial", "&#9680;"),
        "Blocked" => ("blocked", STOP_SIGN),
        "Skipped" => ("skipped", "&ndash;"),
        "Failed" | "Contradiction" => ("failed", "&#10005;"),
        _ => ("unknown", "&ndash;"),
    }
}

fn section(title: &str, body: &str) -> String {
    format!("<section>\n<h2>{title}</h2>\n{body}</section>\n")
}

fn notice(text: &str) -> String {
    format!("<p class=\"notice\">{text}</p>\n")
}

/// A table inside its scroll box, so a narrow screen scrolls the table and not the page.
fn table(headers: &[&str], body: &str) -> String {
    let head = headers.iter().copied().map(header_cell).collect::<String>();
    format!(
        "<div class=\"table-wrap\">\n<table>\n<thead><tr>{head}</tr></thead>\n<tbody>\n{body}</tbody>\n</table>\n</div>\n"
    )
}

fn header_cell(label: &str) -> String {
    format!("<th scope=\"col\">{label}</th>")
}

/// One table row. The cells are HTML already, so the caller escapes run text.
fn row(cells: &[String]) -> String {
    let body: String = cells.iter().map(String::as_str).map(cell).collect();
    format!("<tr>{body}</tr>\n")
}

fn cell(text: &str) -> String {
    format!("<td>{text}</td>")
}

/// The task table. Each status is a shape and a word.
pub(super) fn task_table(rows: &[TaskRow]) -> String {
    let body = if rows.is_empty() {
        notice("The report has no tasks.")
    } else {
        let lines = rows.iter().map(task_html).collect::<String>();
        table(&["Task", "Status", "Reason", "Cost"], &lines)
    };
    section("Tasks", &body)
}

fn task_html(task: &TaskRow) -> String {
    row(&[
        esc(&task.id),
        status_html(&task.status),
        esc(&task.reason),
        esc(&task.cost),
    ])
}

/// Every evidence section, in the order the page shows them.
pub(super) fn evidence_sections(evidence: &Value) -> String {
    [
        agents_section(evidence),
        collisions_section(evidence),
        cost_section(evidence),
        claims_section(evidence),
    ]
    .concat()
}

fn agents_section(evidence: &Value) -> String {
    let agents = items(evidence, "/agents");
    let body = if agents.is_empty() {
        notice("No agent started a task.")
    } else {
        let lines = agents.iter().map(agent_html).collect::<String>();
        table(&["Agent", "Tasks started", "Verified", "Spend"], &lines)
    };
    section("Agents", &body)
}

fn agent_html(agent: &Value) -> String {
    row(&[
        esc(text_of(agent, "agent")),
        count_shown(agent.get("tasks_started")),
        count_shown(agent.get("tasks_verified")),
        money_shown(agent.get("spend")),
    ])
}

fn collisions_section(evidence: &Value) -> String {
    let counts = [
        ("Writes blocked", "/collisions/writes_blocked"),
        ("Writes restored", "/collisions/writes_restored"),
        ("Tampers blocked", "/collisions/tampers_blocked"),
    ]
    .into_iter()
    .map(|(label, pointer)| {
        row(&[
            esc(label),
            count_shown(value_at(Some(evidence), pointer)),
        ])
    })
    .collect::<String>();
    // The evidence file does not count tasks that waited, so the row says it is not known.
    let waited = row(&["Tasks that waited".to_owned(), NOT_KNOWN.to_owned()]);
    let grid = table(&["Check", "Count"], &format!("{counts}{waited}"));
    section("Collisions", &format!("{grid}{}", paths_html(evidence)))
}

fn paths_html(evidence: &Value) -> String {
    let paths = items(evidence, "/collisions/blocked_paths")
        .iter()
        .filter_map(Value::as_str)
        .map(|path| format!("<li><code>{}</code></li>\n", esc(path)))
        .collect::<Vec<String>>();
    if paths.is_empty() {
        return String::new();
    }
    format!(
        "<p>First blocked paths:</p>\n<ul class=\"paths\">\n{}</ul>\n",
        paths.concat()
    )
}

fn cost_section(evidence: &Value) -> String {
    let spent = money_shown(value_at(Some(evidence), "/cost/spent"));
    let cap = money_shown(value_at(Some(evidence), "/cost/cap"));
    let percent = value_at(Some(evidence), "/cost/percent_of_cap").and_then(Value::as_u64);
    let body = match percent {
        Some(percent) => cost_bar(percent, &spent, &cap),
        None => notice(&format!("There is no cap to measure against. Spent {}.", esc(&spent))),
    };
    section("Cost", &body)
}

/// The share of the cap that is spent, as a bar and as text. The bar stops at full width.
fn cost_bar(percent: u64, spent: &str, cap: &str) -> String {
    let width = percent.min(100);
    format!(
        "<div class=\"bar\" aria-hidden=\"true\"><div class=\"bar__fill\" style=\"width: {width}%\"></div></div>\n\
         <p class=\"cost-note\">{percent}% of the cap spent: {} of {}.</p>\n",
        esc(spent),
        esc(cap)
    )
}

fn claims_section(evidence: &Value) -> String {
    let claims = items(evidence, "/claims");
    let body = if claims.is_empty() {
        notice("No tasks.")
    } else {
        let lines = claims.iter().map(claim_html).collect::<String>();
        format!(
            "<p class=\"muted\">A finished task is the agent's claim of done. The check columns are \
             what the harness found. Not checked means no check was stored.</p>\n{}",
            table(
                &[
                    "Task",
                    "Finished",
                    "Tests pass",
                    "New test",
                    "Reviewer ok",
                    "Tamper clear",
                ],
                &lines,
            )
        )
    };
    section("Claims", &body)
}

fn claim_html(claim: &Value) -> String {
    row(&[
        esc(text_of(claim, "task")),
        yes_no(claim.get("finished")),
        yes_no(claim.get("tests_pass")),
        yes_no(claim.get("new_test")),
        yes_no(claim.get("reviewer_ok")),
        yes_no(claim.get("tamper_clear")),
    ])
}
