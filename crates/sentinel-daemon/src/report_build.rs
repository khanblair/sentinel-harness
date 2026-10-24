//! Build the honest report from what the store knows, and write `.sentinel/report.md`.

use sentinel_core::{Event, TaskStatus};
use sentinel_verify::report::{
    Receipt, Report, TaskReport, render_markdown, validate, write_report,
};

use crate::engine::{Engine, EngineError, Result};

impl Engine {
    /// The report of the last run, built from the store.
    pub fn build_report(&self) -> Result<Report> {
        let store = self.store();
        let verifications = store.list_verifications()?;
        let mut tasks = Vec::new();
        for task in store.list_tasks()? {
            let record = verifications.iter().find(|v| v.task == task.id);
            let reason = record
                .map(|v| v.reason.clone())
                .filter(|r| !r.is_empty())
                .unwrap_or_else(|| default_reason(task.status));
            let receipt = record
                .filter(|_| task.status == TaskStatus::Verified)
                .and_then(|v| receipt_from(&v.receipt_json));
            tasks.push(TaskReport {
                id: task.id.clone(),
                title: task.title.clone(),
                status: task.status,
                reason,
                receipt,
                cost: store.task_cost(&task.id)?,
                attempts: task.attempts,
                quality_notes: store.list_quality_notes(Some(&task.id))?.len(),
            });
        }
        Ok(Report {
            tasks,
            total_cost: store.total_cost()?,
            cap: self.config().cap().unwrap_or(sentinel_core::Cost::ZERO),
            generated_at: self.now(),
        })
    }

    /// The report as markdown, for the Report panel.
    pub fn report_markdown(&self) -> Result<String> {
        Ok(render_markdown(&self.build_report()?))
    }

    /// Write `.sentinel/report.md`. A report that breaks its own rules is a bug and is refused.
    pub async fn write_report_file(&self) -> Result<()> {
        let report = self.build_report()?;
        let problems = validate(&report);
        if !problems.is_empty() {
            return Err(EngineError::Invalid(format!(
                "The report is not honest: {}",
                problems.join("; ")
            )));
        }
        let path = self.repo_root().join(".sentinel/report.md");
        write_report(&report, &path)
            .await
            .map_err(|e| EngineError::Invalid(e.to_string()))?;
        self.emit(Event::ReportReady)?;
        Ok(())
    }
}

fn receipt_from(json: &str) -> Option<Receipt> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let field = |name: &str| value.get(name).and_then(|v| v.as_str()).map(str::to_owned);
    Some(Receipt {
        commit: field("commit")?,
        suite_log: field("suite_log")?,
        reviewer: field("reviewer")?,
    })
}

fn default_reason(status: TaskStatus) -> String {
    match status {
        TaskStatus::Verified => "All four checks passed.",
        TaskStatus::Partial => "Some checks passed.",
        TaskStatus::Skipped => "Skipped.",
        TaskStatus::Contradiction => "Contradicts another task.",
        _ => "The run ended before this task finished.",
    }
    .to_owned()
}
