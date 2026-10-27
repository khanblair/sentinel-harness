//! The results page of a run. `sentinel report --page` writes it from `.sentinel/report.md` and,
//! when it exists, `.sentinel/evidence.json`. The page has no script and loads nothing from outside.

mod tables;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

use tables::{
    cost_of, count_shown, evidence_sections, money_text, share_shown, task_table, value_at,
};

/// Where the page goes when `--page` has no path.
const DEFAULT_PAGE: &str = ".sentinel/results.html";
/// The header of the task table in the report. Only the rows under it are tasks, so the
/// evidence tables that follow it are never read as tasks.
const TASK_HEADER: &str = "| Task | Status | Why | Receipt | Cost |";
const EVIDENCE_MISSING: &str = "The evidence file was not found, so only the task table is shown. \
    Agents, collisions, cost, and claims need .sentinel/evidence.json.";
const FOOTER: &str = "<footer>Made by sentinel report --page from .sentinel/report.md and \
    .sentinel/evidence.json.</footer>";

const STYLE: &str = r"
:root {
  --page: #f6f7f9;
  --card: #ffffff;
  --ink: #1d2330;
  --muted: #5b6475;
  --line: #d9dde5;
  --ok: #17703a;
  --warn: #8a5300;
  --stop: #b42318;
  --bar: #2f6fd6;
  --track: #e6e9ef;
}
@media (prefers-color-scheme: dark) {
  :root {
    --page: #141820;
    --card: #1b2029;
    --ink: #e6e9ef;
    --muted: #a3acbc;
    --line: #2c3340;
    --ok: #5fd08a;
    --warn: #f0b04d;
    --stop: #ff8a80;
    --bar: #7aa7ff;
    --track: #2c3340;
  }
}
* { box-sizing: border-box; }
body {
  margin: 0;
  background: var(--page);
  color: var(--ink);
  font-family: system-ui, sans-serif;
  font-size: 1rem;
  line-height: 1.5;
}
main { max-width: 960px; margin: 0 auto; padding: 1.5rem 1rem 2rem; }
h1 { font-size: 1.75rem; margin: 0 0 0.25rem; }
h2 { font-size: 1.25rem; margin: 2rem 0 0.5rem; }
.summary, .muted, footer { color: var(--muted); }
footer { margin-top: 2rem; }
.tiles {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(9rem, 1fr));
  gap: 0.75rem;
  margin-top: 1.5rem;
}
.tile {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  padding: 1rem;
  background: var(--card);
  border: 1px solid var(--line);
  border-radius: 0.75rem;
}
.tile__value { font-size: 1.5rem; font-weight: 700; }
.tile__label { color: var(--muted); }
.table-wrap {
  overflow-x: auto;
  background: var(--card);
  border: 1px solid var(--line);
  border-radius: 0.75rem;
}
table { width: 100%; min-width: 30rem; border-collapse: collapse; }
th, td {
  padding: 0.5rem 0.75rem;
  text-align: left;
  vertical-align: top;
  border-bottom: 1px solid var(--line);
}
th { color: var(--muted); font-weight: 600; }
tr:last-child td { border-bottom: none; }
.status {
  display: inline-flex;
  align-items: center;
  gap: 0.4em;
  white-space: nowrap;
  font-weight: 600;
}
.status__mark { display: inline-block; width: 1em; text-align: center; }
.status--verified { color: var(--ok); }
.status--partial { color: var(--warn); }
.status--blocked, .status--failed { color: var(--stop); }
.status--skipped, .status--unknown { color: var(--muted); }
.stop-sign {
  display: inline-block;
  width: 0.9em;
  height: 0.9em;
  background: currentColor;
  clip-path: polygon(30% 0, 70% 0, 100% 30%, 100% 70%, 70% 100%, 30% 100%, 0 70%, 0 30%);
}
.bar { height: 0.75rem; background: var(--track); border-radius: 999px; overflow: hidden; }
.bar__fill { height: 100%; background: var(--bar); }
.cost-note { margin: 0.5rem 0; }
.paths code { overflow-wrap: anywhere; }
.notice {
  padding: 1rem;
  background: var(--card);
  border: 1px solid var(--line);
  border-radius: 0.75rem;
}
";

/// One row of the task table in the report.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskRow {
    id: String,
    status: String,
    reason: String,
    cost: String,
}

/// Writes the page for the repository's last run and returns the path it went to. A relative
/// `target` is taken from the current folder, as the shell gave it.
pub(crate) fn write_page(repo: &Path, target: Option<PathBuf>) -> Result<PathBuf> {
    let report_path = repo.join(".sentinel/report.md");
    let report = std::fs::read_to_string(&report_path)
        .with_context(|| format!("no report yet at {}", report_path.display()))?;
    let evidence = read_evidence(&repo.join(".sentinel/evidence.json"))?;
    let path = target.unwrap_or_else(|| repo.join(DEFAULT_PAGE));
    std::fs::write(&path, render_page(&report, evidence.as_ref()))
        .with_context(|| format!("could not write {}", path.display()))?;
    Ok(path)
}

/// The evidence file, or `None` when it is missing. A run can end before the file is written.
fn read_evidence(path: &Path) -> Result<Option<Value>> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str::<Value>(&text)
            .map(Some)
            .with_context(|| format!("could not parse {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("could not read {}", path.display())),
    }
}

/// The whole page as one HTML document. `evidence` is `None` when the file was not found.
pub(crate) fn render_page(report: &str, evidence: Option<&Value>) -> String {
    let rows = parse_task_rows(report);
    let mut body = header(&rows, evidence);
    body.push_str(&tiles(&rows, evidence));
    body.push_str(&task_table(&rows));
    body.push_str(&evidence_part(evidence));
    body.push_str(FOOTER);
    shell(&body)
}

/// The task rows of the report. Only the table under the task header counts.
fn parse_task_rows(report: &str) -> Vec<TaskRow> {
    let mut rows = Vec::new();
    let mut in_table = false;
    for line in report.lines().map(str::trim) {
        if line == TASK_HEADER {
            in_table = true;
        } else if !line.starts_with('|') {
            in_table = false;
        } else if in_table && !line.starts_with("|-") {
            rows.extend(task_row(line));
        }
    }
    rows
}

/// Reads `| id | status | reason | receipt | cost |`. A reason may hold `\|`, so the cells
/// between the status and the receipt are joined back before the escape is removed.
fn task_row(line: &str) -> Option<TaskRow> {
    let inner = line.strip_prefix('|')?.strip_suffix('|')?;
    let cells: Vec<&str> = inner.split('|').collect();
    if cells.len() < 5 {
        return None;
    }
    let id = cells[0].trim();
    if id.is_empty() {
        return None;
    }
    let receipt = cells.len() - 2;
    Some(TaskRow {
        id: id.to_owned(),
        status: cells[1].trim().to_owned(),
        reason: cells[2..receipt]
            .join("|")
            .replace("\\|", "|")
            .trim()
            .to_owned(),
        cost: cells[cells.len() - 1].trim().to_owned(),
    })
}

/// Escapes text from the run so that it shows as text and never as markup.
fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

fn verified_count(rows: &[TaskRow]) -> usize {
    rows.iter().filter(|row| row.status == "Verified").count()
}

fn header(rows: &[TaskRow], evidence: Option<&Value>) -> String {
    format!(
        "<h1>Sentinel run results</h1>\n<p class=\"summary\">{}</p>\n",
        esc(&summary_line(rows, evidence))
    )
}

fn summary_line(rows: &[TaskRow], evidence: Option<&Value>) -> String {
    let mut parts = vec![format!(
        "{} of {} tasks verified",
        verified_count(rows),
        rows.len()
    )];
    parts.extend(spend_part(evidence));
    let waves = value_at(evidence, "/waves/count").and_then(Value::as_u64);
    parts.extend(counted(waves, "wave", "waves"));
    let agents = value_at(evidence, "/agents")
        .and_then(Value::as_array)
        .and_then(|list| u64::try_from(list.len()).ok());
    parts.extend(counted(agents, "agent", "agents"));
    format!("{}.", parts.join(", "))
}

/// `$0.17 est of $2.00 spent (8 percent)`. Empty when the evidence has no cost.
fn spend_part(evidence: Option<&Value>) -> Option<String> {
    let spent = money_text(cost_of(value_at(evidence, "/cost/spent"))?);
    let cap = money_text(cost_of(value_at(evidence, "/cost/cap"))?);
    let percent = value_at(evidence, "/cost/percent_of_cap").and_then(Value::as_u64);
    Some(match percent {
        Some(percent) => format!("{spent} of {cap} spent ({percent} percent)"),
        None => format!("{spent} of {cap} spent"),
    })
}

fn counted(count: Option<u64>, singular: &str, plural: &str) -> Option<String> {
    let count = count?;
    let noun = if count == 1 { singular } else { plural };
    Some(format!("{count} {noun}"))
}

fn tiles(rows: &[TaskRow], evidence: Option<&Value>) -> String {
    let verified = verified_count(rows);
    let cells = [
        tile("Verified", &verified.to_string()),
        tile(
            "Not verified",
            &rows.len().saturating_sub(verified).to_string(),
        ),
        tile(
            "Writes blocked",
            &count_shown(value_at(evidence, "/collisions/writes_blocked")),
        ),
        tile(
            "Parallel share",
            &share_shown(value_at(evidence, "/waves/parallel_share")),
        ),
    ];
    format!(
        "<section class=\"tiles\" aria-label=\"Totals\">\n{}</section>\n",
        cells.concat()
    )
}

fn tile(label: &str, value: &str) -> String {
    format!(
        "<div class=\"tile\"><span class=\"tile__value\">{}</span><span class=\"tile__label\">{label}</span></div>\n",
        esc(value)
    )
}

fn evidence_part(evidence: Option<&Value>) -> String {
    match evidence {
        Some(evidence) => evidence_sections(evidence),
        None => format!("<p class=\"notice\">{EVIDENCE_MISSING}</p>\n"),
    }
}

fn shell(body: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>Sentinel run results</title>\n<style>{STYLE}</style>\n</head>\n\
         <body>\n<main>\n{body}</main>\n</body>\n</html>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SAMPLE_REPORT: &str = "# Verification Report\n\n\
        1 of 3 tasks verified (1 Partial, 1 Blocked)\n\n\
        Spend: $0.10 of $2.00 est\n\n\
        | Task | Status | Why | Receipt | Cost |\n\
        |------|--------|-----|---------|------|\n\
        | T-1 | Verified | all four checks passed | abc12345 (opencode) | $0.05 |\n\
        | T-2 | Partial | no new test added | none | $0.03 est |\n\
        | T-3 | Blocked | a \\| b | none | $0.02 |\n\n\
        ## Evidence\n\n\
        ### Agents\n\n\
        | Agent | Tasks started | Verified | Spend | Spend source |\n\
        |-------|---------------|----------|-------|--------------|\n\
        | claude | 2 | 1 | $0.12 est | estimate |\n";

    fn sample_evidence() -> Value {
        json!({
            "agents": [
                {"agent": "claude", "tasks_started": 2, "tasks_verified": 1,
                 "spend": {"micro_usd": 120_000, "estimated": true}, "spend_source": "estimate"},
                {"agent": "codex", "tasks_started": 1, "tasks_verified": 0,
                 "spend": {"micro_usd": 50_000, "estimated": false}, "spend_source": "reported"}
            ],
            "waves": {"count": 1, "most_tasks": 2, "parallel_share": 1.08},
            "collisions": {
                "writes_blocked": 2,
                "blocked_paths": ["src/x.rs", "src/y.rs"],
                "writes_restored": 1,
                "tampers_blocked": 1
            },
            "cost": {
                "spent": {"micro_usd": 170_000, "estimated": true},
                "cap": {"micro_usd": 2_000_000, "estimated": false},
                "percent_of_cap": 8,
                "tasks_verified": 1,
                "per_verified": {"micro_usd": 170_000, "estimated": true},
                "highest_threshold": 70
            },
            "claims": [
                {"task": "T-1", "finished": true, "tests_pass": true, "new_test": true,
                 "reviewer_ok": true, "tamper_clear": true, "status": "verified",
                 "reason": "All four checks passed."},
                {"task": "T-2", "finished": true, "tests_pass": true, "new_test": false,
                 "reviewer_ok": true, "tamper_clear": true, "status": "partial",
                 "reason": "no new test added"},
                {"task": "T-3", "finished": false, "tests_pass": null, "new_test": null,
                 "reviewer_ok": null, "tamper_clear": false, "status": "blocked",
                 "reason": "Tampered with a protected file"}
            ],
            "honesty": {"verified": 1, "partial": 1, "blocked": 1, "skipped": 0,
                        "failed": 0, "claims_not_confirmed": 1}
        })
    }

    #[test]
    fn esc_turns_markup_and_quotes_into_text() {
        assert_eq!(
            esc("<script>alert('x') & \"y\"</script>"),
            "&lt;script&gt;alert(&#39;x&#39;) &amp; &quot;y&quot;&lt;/script&gt;"
        );
    }

    #[test]
    fn parse_task_rows_reads_only_the_task_table() {
        let rows = parse_task_rows(SAMPLE_REPORT);
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[1],
            TaskRow {
                id: "T-2".to_owned(),
                status: "Partial".to_owned(),
                reason: "no new test added".to_owned(),
                cost: "$0.03 est".to_owned(),
            }
        );
        assert_eq!(rows[2].reason, "a | b");
        assert!(rows.iter().all(|row| row.id != "claude"));
    }

    #[test]
    fn render_page_shows_the_summary_and_one_status_per_task() {
        let html = render_page(SAMPLE_REPORT, Some(&sample_evidence()));
        assert!(html.contains(
            "1 of 3 tasks verified, $0.17 est of $2.00 spent (8 percent), 1 wave, 2 agents."
        ));
        assert!(html.contains("</span>Verified</span>"));
        assert!(html.contains("</span>Partial</span>"));
        assert!(html.contains("</span>Blocked</span>"));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn render_page_escapes_the_reason_from_the_run() {
        let report = "| Task | Status | Why | Receipt | Cost |\n\
            |------|--------|-----|---------|------|\n\
            | T-9 | Blocked | <script>alert(\"x\")</script> | none | $0.01 |\n";
        let html = render_page(report, None);
        assert!(html.contains("&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;"));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn render_page_without_evidence_says_the_file_was_not_found() {
        let html = render_page(SAMPLE_REPORT, None);
        assert!(html.contains("The evidence file was not found"));
        assert!(!html.contains("<h2>Agents</h2>"));
    }

    #[test]
    fn render_page_with_evidence_shows_collisions_claims_and_the_cost_bar() {
        let html = render_page(SAMPLE_REPORT, Some(&sample_evidence()));
        assert!(html.contains("src/y.rs"));
        assert!(html.contains("width: 8%"));
        assert!(html.contains("not checked"));
    }
}
