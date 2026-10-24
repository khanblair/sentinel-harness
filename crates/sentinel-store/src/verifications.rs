//! Verification records: the four checks and the verdict for one task.

use rusqlite::{OptionalExtension, Row, params};
use sentinel_core::{CoreError, TaskId, TaskStatus};
use serde::{Deserialize, Serialize};

use crate::{Result, Store, StoreError, enum_from_text, enum_text};

/// The four checks plus the verdict for one task. The verdict may be anything
/// the verifier can state, including a failed verdict with a reason.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationRecord {
    pub task: TaskId,
    pub tests_pass: bool,
    pub new_test: bool,
    pub reviewer_ok: bool,
    pub tamper_clear: bool,
    pub verdict: TaskStatus,
    pub reason: String,
    pub receipt_json: String,
}

const VERIFICATION_COLUMNS: &str =
    "task_id, tests_pass, new_test, reviewer_ok, tamper_clear, verdict, reason, receipt_json";

impl Store {
    /// Writes the record for one task, replacing an older one. A `Verified`
    /// verdict needs all four checks true; the store repeats that rule so a
    /// bad caller cannot stamp a task as done.
    pub fn upsert_verification(&self, record: &VerificationRecord) -> Result<()> {
        checks_cover_verified(record)?;
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO verifications
                 (task_id, tests_pass, new_test, reviewer_ok, tamper_clear, verdict, reason, \
                  receipt_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(task_id) DO UPDATE SET
                tests_pass = excluded.tests_pass,
                new_test = excluded.new_test,
                reviewer_ok = excluded.reviewer_ok,
                tamper_clear = excluded.tamper_clear,
                verdict = excluded.verdict,
                reason = excluded.reason,
                receipt_json = excluded.receipt_json",
            params![
                record.task.as_str(),
                record.tests_pass,
                record.new_test,
                record.reviewer_ok,
                record.tamper_clear,
                enum_text(&record.verdict)?,
                record.reason,
                record.receipt_json
            ],
        )?;
        Ok(())
    }

    pub fn get_verification(&self, task: &TaskId) -> Result<Option<VerificationRecord>> {
        let connection = self.connection()?;
        let row = connection
            .query_row(
                &format!("SELECT {VERIFICATION_COLUMNS} FROM verifications WHERE task_id = ?1"),
                params![task.as_str()],
                read_verification_row,
            )
            .optional()?;
        row.map(verification_from_row).transpose()
    }

    /// Every verification, oldest first.
    pub fn list_verifications(&self) -> Result<Vec<VerificationRecord>> {
        let connection = self.connection()?;
        let rows = {
            let mut statement = connection.prepare(&format!(
                "SELECT {VERIFICATION_COLUMNS} FROM verifications ORDER BY rowid"
            ))?;
            let mapped = statement.query_map([], read_verification_row)?;
            mapped.collect::<rusqlite::Result<Vec<VerificationRow>>>()?
        };
        rows.into_iter().map(verification_from_row).collect()
    }
}

fn checks_cover_verified(record: &VerificationRecord) -> Result<()> {
    if record.verdict == TaskStatus::Verified {
        let complete =
            record.tests_pass && record.new_test && record.reviewer_ok && record.tamper_clear;
        if !complete {
            return Err(StoreError::Core(CoreError::InvalidTransition {
                from: "incomplete checks",
                to: "verified",
            }));
        }
    }
    Ok(())
}

fn read_verification_row(row: &Row<'_>) -> rusqlite::Result<VerificationRow> {
    columns!(row; 0, 1, 2, 3, 4, 5, 6, 7)
}

fn verification_from_row(row: VerificationRow) -> Result<VerificationRecord> {
    let (task, tests_pass, new_test, reviewer_ok, tamper_clear, verdict, reason, receipt_json) =
        row;
    Ok(VerificationRecord {
        task: TaskId::new(task),
        tests_pass,
        new_test,
        reviewer_ok,
        tamper_clear,
        verdict: enum_from_text(&verdict)?,
        reason,
        receipt_json,
    })
}

type VerificationRow = (String, bool, bool, bool, bool, String, String, String);
