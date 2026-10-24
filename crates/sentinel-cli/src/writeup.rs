//! The one-page write-up of the last run (P10-04, `sentinel report --writeup`). It reads the
//! report table and the run log, and drafts Markdown for the user to edit. No network, no model.

use sentinel_core::{Event, EventRecord, TaskStatus};

/// Longer reasons are cut, so each task stays on one line.
const MAX_REASON_CHARS: usize = 120;
const SHARE_NOTE: &str =
    "Draft from `.sentinel/report.md` and `.sentinel/log.jsonl`. Edit before you share it.";

/// One row of the report table.
#[derive(Debug, PartialEq, Eq)]
struct TaskRow {
    id: String,
    status: String,
    reason: String,
}

/// What the run log says about the last run. A new `RunStarted` resets it.
#[derive(Debug, Default, PartialEq, Eq)]
struct LogCounts {
    run_finished: Option<(usize, usize)>,
    blocked_writes: usize,
    tampered: usize,
    partial: usize,
}

/// The write-up as Markdown, without a trailing newline.
pub fn build(report: &str, log: &str) -> String {
    let rows = report_rows(report);
    let counts = log_counts(log);
    let mut sections = vec![
        "# What Sentinel did".to_owned(),
        summary_sentence(&rows, counts.run_finished),
    ];
    if let Some(spend) = report.lines().find_map(|line| line.strip_prefix("Spend: ")) {
        sections.push(format!("## Spend\n\nSpent {spend}."));
    }
    let bullets = honesty_bullets(&counts);
    if !bullets.is_empty() {
        sections.push(format!("## How it stayed honest\n\n{}", bullets.join("\n")));
    }
    if !rows.is_empty() {
        let lines: Vec<String> = rows.iter().map(task_line).collect();
        sections.push(format!("## Tasks\n\n{}", lines.join("\n")));
    }
    sections.push(SHARE_NOTE.to_owned());
    sections.join("\n\n")
}

fn report_rows(report: &str) -> Vec<TaskRow> {
    report
        .lines()
        .filter(|line| {
            line.starts_with('|') && !line.starts_with("|-") && !line.starts_with("| Task |")
        })
        .filter_map(task_row)
        .collect()
}

/// Reads `| id | status | reason | receipt | cost |`. The reason may hold `\|`, so the middle
/// cells are joined back before the escape is removed.
fn task_row(line: &str) -> Option<TaskRow> {
    let inner = line.trim().strip_prefix('|')?.strip_suffix('|')?;
    let cells: Vec<&str> = inner.split('|').collect();
    if cells.len() < 5 {
        return None;
    }
    let reason = cells[2..cells.len() - 2].join("|").replace("\\|", "|");
    Some(TaskRow {
        id: cells[0].trim().to_owned(),
        status: cells[1].trim().to_owned(),
        reason: shorten(&reason.trim().replace('\u{2014}', "-")),
    })
}

fn shorten(text: &str) -> String {
    if text.chars().count() <= MAX_REASON_CHARS {
        return text.to_owned();
    }
    let kept: String = text.chars().take(MAX_REASON_CHARS - 3).collect();
    format!("{}...", kept.trim_end())
}

fn count_status(rows: &[TaskRow], label: &str) -> usize {
    rows.iter().filter(|row| row.status == label).count()
}

/// Verified and total come from the run's `RunFinished` event. The table counts the rest.
fn summary_sentence(rows: &[TaskRow], finished: Option<(usize, usize)>) -> String {
    let (verified, total) =
        finished.unwrap_or_else(|| (count_status(rows, "Verified"), rows.len()));
    format!(
        "{verified} of {total} tasks verified, {} partial, {} blocked, {} skipped.",
        count_status(rows, "Partial"),
        count_status(rows, "Blocked"),
        count_status(rows, "Skipped"),
    )
}

fn task_line(row: &TaskRow) -> String {
    format!("- {}, {}: {}", row.id, row.status, row.reason)
}

fn log_counts(log: &str) -> LogCounts {
    let mut counts = LogCounts::default();
    for line in log.lines() {
        // A crash can leave a half-written last line. Skip it and keep the rest.
        if let Ok(record) = serde_json::from_str::<EventRecord>(line) {
            count_event(&mut counts, &record.event);
        }
    }
    counts
}

fn count_event(counts: &mut LogCounts, event: &Event) {
    if matches!(event, Event::RunStarted { .. }) {
        *counts = LogCounts::default();
    }
    if let Event::RunFinished { verified, total } = event {
        counts.run_finished = Some((to_count(*verified), to_count(*total)));
    }
    if matches!(event, Event::WriteBlocked { .. }) {
        counts.blocked_writes += 1;
    }
    if let Event::Verdict { status, reason, .. } = event {
        count_verdict(counts, *status, reason);
    }
}

fn count_verdict(counts: &mut LogCounts, status: TaskStatus, reason: &str) {
    if status == TaskStatus::Partial {
        counts.partial += 1;
    }
    if reason.to_lowercase().contains("tamper") {
        counts.tampered += 1;
    }
}

fn to_count(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn honesty_bullets(counts: &LogCounts) -> Vec<String> {
    let mut bullets = Vec::new();
    if counts.blocked_writes > 0 {
        bullets.push(format!(
            "- The write guard stopped {} outside reserved files and restored them.",
            counted(counts.blocked_writes, "write", "writes")
        ));
    }
    if counts.tampered > 0 {
        bullets.push(format!(
            "- {} edited an existing test and {} failed.",
            counted(counts.tampered, "task", "tasks"),
            if counts.tampered == 1 { "was" } else { "were" }
        ));
    }
    if counts.partial > 0 {
        bullets.push(format!(
            "- {} passed the tests but {} not marked Verified because no second agent approved {}.",
            counted(counts.partial, "task", "tasks"),
            if counts.partial == 1 { "was" } else { "were" },
            if counts.partial == 1 { "it" } else { "them" }
        ));
    }
    bullets
}

fn counted(count: usize, singular: &str, plural: &str) -> String {
    let noun = if count == 1 { singular } else { plural };
    format!("{count} {noun}")
}
