//! Honest report generation.

use std::path::Path;

use sentinel_core::cost::Cost;
use sentinel_core::ids::TaskId;
use sentinel_core::task::TaskStatus;
use time::OffsetDateTime;

use crate::Result;

/// A single task's report entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskReport {
    pub id: TaskId,
    pub title: String,
    pub status: TaskStatus,
    pub reason: String,
    pub receipt: Option<Receipt>,
    pub cost: Cost,
    pub attempts: u32,
    pub quality_notes: usize,
}

/// Receipt for a verified task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub commit: String,
    pub suite_log: String,
    pub reviewer: String,
}

/// Full report for a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub tasks: Vec<TaskReport>,
    pub total_cost: Cost,
    pub cap: Cost,
    pub generated_at: OffsetDateTime,
}

/// Validates a report, returning a list of problems.
///
/// Problems checked:
/// - A task with an empty reason.
/// - A Verified task without a receipt.
/// - A receipt on a task that is not Verified.
pub fn validate(report: &Report) -> Vec<String> {
    let mut problems = Vec::new();

    for task in &report.tasks {
        // Empty reason
        if task.reason.trim().is_empty() {
            problems.push(format!("Task {} has empty reason", task.id));
        }

        // Verified without receipt
        if task.status == TaskStatus::Verified && task.receipt.is_none() {
            problems.push(format!("Task {} is Verified but has no receipt", task.id));
        }

        // Receipt on non-Verified task
        if task.status != TaskStatus::Verified && task.receipt.is_some() {
            problems.push(format!(
                "Task {} has a receipt but status is {}",
                task.id,
                task.status.label()
            ));
        }
    }

    problems
}

/// Renders a report as Markdown.
///
/// Format:
/// - Title
/// - Summary line: "N of M tasks verified" with counts per status
/// - Spend as "$X of $CAP" with "est" when estimated
/// - Table: | Task | Status | Why | Receipt | Cost |
/// - Quality notes section (counts only, never code)
pub fn render_markdown(report: &Report) -> String {
    let mut out = String::from("# Verification Report\n\n");
    push_summary(&mut out, report);
    push_spend(&mut out, report);
    push_task_table(&mut out, report);
    push_quality_notes(&mut out, report);
    out
}

fn count_where(report: &Report, keep: impl Fn(TaskStatus) -> bool) -> usize {
    report.tasks.iter().filter(|t| keep(t.status)).count()
}

fn push_summary(out: &mut String, report: &Report) {
    let total = report.tasks.len();
    let verified = count_where(report, |s| s == TaskStatus::Verified);
    let partial = count_where(report, |s| s == TaskStatus::Partial);
    let blocked = count_where(report, |s| {
        s == TaskStatus::Blocked || s == TaskStatus::Failed
    });
    let contradiction = count_where(report, |s| s == TaskStatus::Contradiction);
    let skipped = count_where(report, |s| s == TaskStatus::Skipped);

    out.push_str(&format!("{} of {} tasks verified", verified, total));
    let counted = [
        ("Partial", partial),
        ("Blocked", blocked),
        ("Contradiction", contradiction),
        ("Skipped", skipped),
    ];
    let parts: Vec<String> = counted
        .iter()
        .filter(|(_, count)| *count > 0)
        .map(|(label, count)| format!("{} {}", count, label))
        .collect();
    if !parts.is_empty() {
        out.push_str(&format!(" ({})", parts.join(", ")));
    }
    out.push_str("\n\n");
}

fn push_spend(out: &mut String, report: &Report) {
    let est_marker = if report.total_cost.is_estimated() {
        " est"
    } else {
        ""
    };
    out.push_str(&format!(
        "Spend: {} of {}{}\n\n",
        report.total_cost, report.cap, est_marker
    ));
}

fn push_task_table(out: &mut String, report: &Report) {
    out.push_str("| Task | Status | Why | Receipt | Cost |\n");
    out.push_str("|------|--------|-----|---------|------|\n");
    for task in &report.tasks {
        out.push_str(&task_row(task));
    }
    out.push('\n');
}

fn task_row(task: &TaskReport) -> String {
    let status_label = match task.status {
        TaskStatus::Failed => "Blocked", // Failed shown as Blocked in final report
        _ => task.status.label(),
    };
    // Escape pipes in reason
    let reason = task.reason.replace('|', "\\|");
    format!(
        "| {} | {} | {} | {} | {} |\n",
        task.id,
        status_label,
        reason,
        receipt_cell(task.receipt.as_ref()),
        cost_cell(task.cost)
    )
}

fn receipt_cell(receipt: Option<&Receipt>) -> String {
    match receipt {
        Some(r) => format!(
            "{} ({})",
            r.commit.chars().take(8).collect::<String>(),
            r.reviewer
        ),
        None => "—".to_string(),
    }
}

fn cost_cell(cost: Cost) -> String {
    let cost_str = cost.to_string();
    if cost.is_estimated() {
        format!("{} est", cost_str)
    } else {
        cost_str
    }
}

fn push_quality_notes(out: &mut String, report: &Report) {
    out.push_str("## Quality notes\n\n");
    let mut any_notes = false;
    for task in &report.tasks {
        if task.quality_notes > 0 {
            any_notes = true;
            out.push_str(&format!("- {}: {} note(s)\n", task.id, task.quality_notes));
        }
    }
    if !any_notes {
        out.push_str("No quality notes.\n");
    }
}

/// Writes a report to a file.
pub async fn write_report(report: &Report, path: &Path) -> Result<()> {
    let markdown = render_markdown(report);
    tokio::fs::write(path, markdown).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::cost::Cost;
    use sentinel_core::ids::TaskId;
    use sentinel_core::task::TaskStatus;
    use time::OffsetDateTime;

    fn sample_report() -> Report {
        Report {
            tasks: vec![
                TaskReport {
                    id: TaskId::new("T-1"),
                    title: "Add feature".into(),
                    status: TaskStatus::Verified,
                    reason: "all four checks passed".into(),
                    receipt: Some(Receipt {
                        commit: "abc12345".into(),
                        suite_log: "logs/barrier1.log".into(),
                        reviewer: "opencode".into(),
                    }),
                    cost: Cost::from_micro_usd(50_000, false),
                    attempts: 1,
                    quality_notes: 2,
                },
                TaskReport {
                    id: TaskId::new("T-2"),
                    title: "Fix bug".into(),
                    status: TaskStatus::Partial,
                    reason: "no new test added".into(),
                    receipt: None,
                    cost: Cost::from_micro_usd(30_000, true),
                    attempts: 1,
                    quality_notes: 0,
                },
                TaskReport {
                    id: TaskId::new("T-3"),
                    title: "Refactor".into(),
                    status: TaskStatus::Failed,
                    reason: "test suite failed at the barrier".into(),
                    receipt: None,
                    cost: Cost::from_micro_usd(20_000, false),
                    attempts: 2,
                    quality_notes: 1,
                },
            ],
            total_cost: Cost::from_micro_usd(100_000, true),
            cap: Cost::from_micro_usd(2_000_000, false),
            generated_at: OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn validate_catches_empty_reason() {
        let mut report = sample_report();
        report.tasks[0].reason = "".into();

        let problems = validate(&report);
        assert!(problems.iter().any(|p| p.contains("empty reason")));
    }

    #[test]
    fn validate_catches_verified_without_receipt() {
        let mut report = sample_report();
        report.tasks[0].receipt = None;

        let problems = validate(&report);
        assert!(problems.iter().any(|p| p.contains("no receipt")));
    }

    #[test]
    fn validate_catches_receipt_on_non_verified() {
        let mut report = sample_report();
        report.tasks[1].receipt = Some(Receipt {
            commit: "abc".into(),
            suite_log: "log".into(),
            reviewer: "r".into(),
        });

        let problems = validate(&report);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("has a receipt but status"))
        );
    }

    #[test]
    fn validate_clean_report_no_problems() {
        let report = sample_report();
        let problems = validate(&report);
        assert!(problems.is_empty(), "problems: {:?}", problems);
    }

    #[test]
    fn render_markdown_has_title() {
        let report = sample_report();
        let md = render_markdown(&report);
        assert!(md.starts_with("# Verification Report"));
    }

    #[test]
    fn render_markdown_summary_line() {
        let report = sample_report();
        let md = render_markdown(&report);
        assert!(md.contains("1 of 3 tasks verified"));
        assert!(md.contains("1 Partial"));
        assert!(md.contains("1 Blocked"));
    }

    #[test]
    fn render_markdown_spend_line() {
        let report = sample_report();
        let md = render_markdown(&report);
        assert!(md.contains("Spend: $0.10 of $2.00 est"));
    }

    #[test]
    fn render_markdown_table_header() {
        let report = sample_report();
        let md = render_markdown(&report);
        assert!(md.contains("| Task | Status | Why | Receipt | Cost |"));
    }

    #[test]
    fn render_markdown_status_labels() {
        let report = sample_report();
        let md = render_markdown(&report);
        assert!(md.contains("| T-1 | Verified |"));
        assert!(md.contains("| T-2 | Partial |"));
        // Failed shown as Blocked
        assert!(md.contains("| T-3 | Blocked |"));
    }

    #[test]
    fn render_markdown_receipt_format() {
        let report = sample_report();
        let md = render_markdown(&report);
        assert!(md.contains("abc12345 (opencode)"));
    }

    #[test]
    fn render_markdown_cost_est_marker() {
        let report = sample_report();
        let md = render_markdown(&report);
        // T-2 has estimated cost
        assert!(md.contains("$0.03 est"));
    }

    #[test]
    fn render_markdown_quality_notes_section() {
        let report = sample_report();
        let md = render_markdown(&report);
        assert!(md.contains("## Quality notes"));
        assert!(md.contains("T-1: 2 note(s)"));
        assert!(md.contains("T-3: 1 note(s)"));
        // T-2 has 0 notes, should not appear
        assert!(!md.contains("T-2:"));
    }

    #[test]
    fn render_markdown_no_quality_notes() {
        let mut report = sample_report();
        for task in &mut report.tasks {
            task.quality_notes = 0;
        }
        let md = render_markdown(&report);
        assert!(md.contains("No quality notes."));
    }

    #[tokio::test]
    async fn write_report_creates_file() {
        let report = sample_report();
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("report.md");

        write_report(&report, &path).await.unwrap();

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("# Verification Report"));
    }
}
