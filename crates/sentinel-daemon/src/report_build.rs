//! Build the honest report from what the store knows, and write `.sentinel/report.md`.

use sentinel_core::{Cost, Event, EventRecord, TaskStatus};
use sentinel_store::{Store, VerificationRecord};
use sentinel_verify::report::{Receipt, Report, TaskReport, render_markdown, validate};

use crate::engine::{Engine, EngineError, Result};
use crate::evidence::{
    Evidence, EvidenceInput, TaskRow, VerificationRow, build_evidence, render_evidence,
};

/// Events are read this many at a time, so no single query holds the whole log.
const EVENT_PAGE: u32 = 500;

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

    /// The evidence of the last run, read from the store and the event log.
    pub fn evidence(&self) -> Result<Evidence> {
        let store = self.store();
        let mut tasks = Vec::new();
        for task in store.list_tasks()? {
            tasks.push(TaskRow {
                cost: store.task_cost(&task.id)?,
                id: task.id,
                status: task.status,
            });
        }
        let input = EvidenceInput {
            tasks,
            verifications: store
                .list_verifications()?
                .into_iter()
                .map(verification_row)
                .collect(),
            events: all_events(store)?,
            total_cost: store.total_cost()?,
            cap: self.config().cap().unwrap_or(Cost::ZERO),
        };
        Ok(build_evidence(&input))
    }

    /// The report as markdown, for the Report panel. The evidence follows it when it can be built.
    pub fn report_markdown(&self) -> Result<String> {
        let mut markdown = render_markdown(&self.build_report()?);
        if let Some(evidence) = self.evidence_or_skip() {
            markdown.push_str(&render_evidence(&evidence));
        }
        Ok(markdown)
    }

    /// Write `.sentinel/report.md`, with the evidence after it. A report that breaks its own rules
    /// is a bug and is refused.
    pub async fn write_report_file(&self) -> Result<()> {
        let report = self.build_report()?;
        let problems = validate(&report);
        if !problems.is_empty() {
            return Err(EngineError::Invalid(format!(
                "The report is not honest: {}",
                problems.join("; ")
            )));
        }
        let mut markdown = render_markdown(&report);
        if let Some(evidence) = self.evidence_or_skip() {
            markdown.push_str(&render_evidence(&evidence));
            self.write_evidence_json(&evidence).await;
        }
        let path = self.repo_root().join(".sentinel/report.md");
        tokio::fs::write(&path, markdown)
            .await
            .map_err(|e| EngineError::Invalid(e.to_string()))?;
        self.emit(&Event::ReportReady)?;
        Ok(())
    }

    /// The evidence, or `None` when it cannot be built. The report is written either way.
    fn evidence_or_skip(&self) -> Option<Evidence> {
        match self.evidence() {
            Ok(evidence) => Some(evidence),
            Err(error) => {
                eprintln!("sentinel: the evidence section was left out: {error}");
                None
            }
        }
    }

    /// Write `.sentinel/evidence.json` next to the report. A failure is printed, not raised.
    async fn write_evidence_json(&self, evidence: &Evidence) {
        let path = self.repo_root().join(".sentinel/evidence.json");
        let outcome = match serde_json::to_string_pretty(evidence) {
            Ok(json) => tokio::fs::write(&path, json)
                .await
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        if let Err(error) = outcome {
            eprintln!("sentinel: could not write the evidence file: {error}");
        }
    }
}

fn verification_row(record: VerificationRecord) -> VerificationRow {
    VerificationRow {
        task: record.task,
        tests_pass: record.tests_pass,
        new_test: record.new_test,
        reviewer_ok: record.reviewer_ok,
        tamper_clear: record.tamper_clear,
        reason: record.reason,
    }
}

/// Every event, oldest first, read a page at a time.
fn all_events(store: &Store) -> Result<Vec<EventRecord>> {
    let mut events = Vec::new();
    let mut after: i64 = 0;
    loop {
        let page = store.events_after(after, EVENT_PAGE)?;
        let Some(last_id) = page.last().map(|record| record.id) else {
            return Ok(events);
        };
        after = last_id;
        events.extend(page);
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
        TaskStatus::Planned
        | TaskStatus::Ready
        | TaskStatus::Waiting
        | TaskStatus::Reserved
        | TaskStatus::Running
        | TaskStatus::Checkpointed
        | TaskStatus::Verifying
        | TaskStatus::Failed
        | TaskStatus::Blocked => "The run ended before this task finished.",
    }
    .to_owned()
}
