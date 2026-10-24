//! Approval card persistence. Cards are created here with their time; answering
//! them is the only state change the store allows.

use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};
use sentinel_core::{Approval, ApprovalId, ApprovalState, CoreError, TaskId, ToolId};
use time::OffsetDateTime;

use crate::{Result, Store, StoreError, enum_from_text, enum_text, time_text};

const APPROVAL_COLUMNS: &str = "id, kind, task_id, agent_id, path, state, decided_by";

type ApprovalRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    Option<String>,
);

impl Store {
    pub fn insert_approval(&self, approval: &Approval, at: OffsetDateTime) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO approvals (id, kind, task_id, agent_id, path, state, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                approval.id.as_str(),
                enum_text(&approval.kind)?,
                approval.task.as_ref().map(TaskId::as_str),
                approval.agent.as_ref().map(ToolId::as_str),
                approval.path.as_deref(),
                enum_text(&approval.state)?,
                time_text(at)?
            ],
        )?;
        Ok(())
    }

    pub fn get_approval(&self, id: &ApprovalId) -> Result<Option<Approval>> {
        let connection = self.connection()?;
        let row = read_approval(&connection, id)?;
        row.map(approval_from_row).transpose()
    }

    /// Only a pending card can be answered. Answering twice, or answering an
    /// unknown card, is an error.
    // The store API fixes these arguments, and clippy counts `self`.
    #[allow(clippy::too_many_arguments)]
    pub fn answer_approval(
        &self,
        id: &ApprovalId,
        state: ApprovalState,
        decided_by: &str,
        at: OffsetDateTime,
    ) -> Result<Approval> {
        let connection = self.connection()?;
        let stored = connection
            .query_row(
                "SELECT state FROM approvals WHERE id = ?1",
                params![id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| StoreError::NotFound(format!("approval {}", id.as_str())))?;
        let current: ApprovalState = enum_from_text(&stored)?;
        reject_answer(current, state)?;
        connection.execute(
            "UPDATE approvals SET state = ?1, decided_by = ?2, decided_at = ?3 WHERE id = ?4",
            params![enum_text(&state)?, decided_by, time_text(at)?, id.as_str()],
        )?;
        let updated =
            read_approval(&connection, id)?.ok_or_else(|| StoreError::NotFound(id.to_string()))?;
        approval_from_row(updated)
    }

    /// An empty filter means every card, newest first.
    pub fn list_approvals(&self, states: &[ApprovalState]) -> Result<Vec<Approval>> {
        let connection = self.connection()?;
        let (sql, filters) = approval_filter(states)?;
        let rows = {
            let mut statement = connection.prepare(&sql)?;
            let mapped =
                statement.query_map(params_from_iter(filters.iter()), read_approval_row)?;
            mapped.collect::<rusqlite::Result<Vec<ApprovalRow>>>()?
        };
        rows.into_iter().map(approval_from_row).collect()
    }
}

fn read_approval(connection: &Connection, id: &ApprovalId) -> Result<Option<ApprovalRow>> {
    connection
        .query_row(
            &format!("SELECT {APPROVAL_COLUMNS} FROM approvals WHERE id = ?1"),
            params![id.as_str()],
            read_approval_row,
        )
        .optional()
        .map_err(StoreError::Sqlite)
}

fn reject_answer(current: ApprovalState, next: ApprovalState) -> Result<()> {
    let keeps_pending = next == ApprovalState::Pending;
    if current != ApprovalState::Pending || keeps_pending {
        return Err(StoreError::Core(CoreError::InvalidTransition {
            from: approval_state_name(current),
            to: approval_state_name(next),
        }));
    }
    Ok(())
}

fn approval_state_name(state: ApprovalState) -> &'static str {
    match state {
        ApprovalState::Pending => "pending",
        ApprovalState::Allowed => "allowed",
        ApprovalState::Denied => "denied",
        ApprovalState::Defaulted => "defaulted",
    }
}

fn approval_filter(states: &[ApprovalState]) -> Result<(String, Vec<String>)> {
    if states.is_empty() {
        return Ok((
            format!(
                "SELECT {APPROVAL_COLUMNS} FROM approvals \
                 ORDER BY created_at DESC, id DESC"
            ),
            Vec::new(),
        ));
    }
    let placeholders = vec!["?"; states.len()].join(",");
    let filters = states
        .iter()
        .map(enum_text)
        .collect::<Result<Vec<String>>>()?;
    Ok((
        format!(
            "SELECT {APPROVAL_COLUMNS} FROM approvals WHERE state IN ({placeholders}) \
             ORDER BY created_at DESC, id DESC"
        ),
        filters,
    ))
}

fn read_approval_row(row: &Row<'_>) -> rusqlite::Result<ApprovalRow> {
    columns!(row; 0, 1, 2, 3, 4, 5, 6)
}

fn approval_from_row(row: ApprovalRow) -> Result<Approval> {
    let (id, kind, task, agent, path, state, decided_by) = row;
    Ok(Approval {
        id: ApprovalId::new(id),
        kind: enum_from_text(&kind)?,
        task: task.map(TaskId::new),
        agent: agent.map(ToolId::new),
        path,
        state: enum_from_text(&state)?,
        decided_by,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{ApprovalKind, TaskId, ToolId};

    fn card() -> Approval {
        Approval {
            id: ApprovalId::new("A-1"),
            kind: ApprovalKind::Extension,
            task: Some(TaskId::new("T-01")),
            agent: Some(ToolId::new("claude")),
            path: Some("src/main.rs".to_owned()),
            state: ApprovalState::Pending,
            decided_by: None,
        }
    }

    fn when() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    #[test]
    fn pending_card_round_trips() {
        let store = Store::open_in_memory().unwrap();
        store.insert_approval(&card(), when()).unwrap();
        assert_eq!(
            store.get_approval(&ApprovalId::new("A-1")).unwrap(),
            Some(card())
        );
    }

    #[test]
    fn answered_card_keeps_the_decision() {
        let store = Store::open_in_memory().unwrap();
        store.insert_approval(&card(), when()).unwrap();
        let decided = store
            .answer_approval(
                &ApprovalId::new("A-1"),
                ApprovalState::Allowed,
                "phone",
                when(),
            )
            .unwrap();
        assert_eq!(decided.state, ApprovalState::Allowed);
        assert_eq!(decided.decided_by.as_deref(), Some("phone"));
    }

    #[test]
    fn second_answer_fails() {
        let store = Store::open_in_memory().unwrap();
        store.insert_approval(&card(), when()).unwrap();
        store
            .answer_approval(
                &ApprovalId::new("A-1"),
                ApprovalState::Denied,
                "phone",
                when(),
            )
            .unwrap();
        assert!(
            store
                .answer_approval(
                    &ApprovalId::new("A-1"),
                    ApprovalState::Allowed,
                    "phone",
                    when()
                )
                .is_err()
        );
    }
}
