//! Decision prompt persistence. A decision is the contradiction question from
//! F15: one of two tasks is chosen, or both are skipped.

use rusqlite::{Connection, OptionalExtension, Row, params};
use sentinel_core::{CoreError, Decision, DecisionId, DecisionState};
use time::OffsetDateTime;

use crate::{Result, Store, StoreError, enum_from_text, enum_text, time_text};

const DECISION_COLUMNS: &str = "id, prompt, tasks_json, state, answer";

/// Decision prompts today are all contradiction questions (F15). The `kind`
/// column from migration 1 records that, so a later kind has a real value.
const DECISION_KIND: &str = "contradiction";

type DecisionRow = (String, String, String, String, Option<String>);

impl Store {
    pub fn insert_decision(&self, decision: &Decision, at: OffsetDateTime) -> Result<()> {
        let connection = self.connection()?;
        let tasks_json = serde_json::to_string(&decision.tasks)?;
        connection.execute(
            "INSERT INTO decisions (id, kind, prompt, answer, actor, at, tasks_json, state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                decision.id.as_str(),
                DECISION_KIND,
                decision.prompt,
                decision.answer.as_deref(),
                "", // actor is set when answered
                time_text(at)?,
                tasks_json,
                enum_text(&decision.state)?
            ],
        )?;
        Ok(())
    }

    /// Only a pending prompt can be answered. The answer is the chosen task id,
    /// or `skip_both`; the actor is `human` or `default`.
    // The store API fixes these arguments, and clippy counts `self`.
    #[allow(clippy::too_many_arguments)]
    pub fn answer_decision(
        &self,
        id: &DecisionId,
        state: DecisionState,
        answer: Option<&str>,
        actor: &str,
        at: OffsetDateTime,
    ) -> Result<Decision> {
        let connection = self.connection()?;
        let stored = connection
            .query_row(
                "SELECT state FROM decisions WHERE id = ?1",
                params![id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| StoreError::NotFound(format!("decision {}", id.as_str())))?;
        let current: DecisionState = enum_from_text(&stored)?;
        reject_answer(current, state)?;
        connection.execute(
            "UPDATE decisions SET state = ?1, answer = ?2, actor = ?3, at = ?4 WHERE id = ?5",
            params![
                enum_text(&state)?,
                answer,
                actor,
                time_text(at)?,
                id.as_str()
            ],
        )?;
        let updated =
            read_decision(&connection, id)?.ok_or_else(|| StoreError::NotFound(id.to_string()))?;
        decision_from_row(updated)
    }

    /// An empty filter means every prompt, oldest first.
    pub fn list_decisions(&self, states: &[DecisionState]) -> Result<Vec<Decision>> {
        let connection = self.connection()?;
        let (sql, filters) = decision_filter(states)?;
        let rows = {
            let mut statement = connection.prepare(&sql)?;
            let mapped = statement.query_map(
                rusqlite::params_from_iter(filters.iter()),
                read_decision_row,
            )?;
            mapped.collect::<rusqlite::Result<Vec<DecisionRow>>>()?
        };
        rows.into_iter().map(decision_from_row).collect()
    }
}

fn read_decision(connection: &Connection, id: &DecisionId) -> Result<Option<DecisionRow>> {
    connection
        .query_row(
            &format!("SELECT {DECISION_COLUMNS} FROM decisions WHERE id = ?1"),
            params![id.as_str()],
            read_decision_row,
        )
        .optional()
        .map_err(StoreError::Sqlite)
}

fn reject_answer(current: DecisionState, next: DecisionState) -> Result<()> {
    if current != DecisionState::Pending || next == DecisionState::Pending {
        return Err(StoreError::Core(CoreError::InvalidTransition {
            from: decision_state_name(current),
            to: decision_state_name(next),
        }));
    }
    Ok(())
}

fn decision_state_name(state: DecisionState) -> &'static str {
    match state {
        DecisionState::Pending => "pending",
        DecisionState::Answered => "answered",
        DecisionState::Defaulted => "defaulted",
    }
}

fn decision_filter(states: &[DecisionState]) -> Result<(String, Vec<String>)> {
    if states.is_empty() {
        return Ok((
            format!("SELECT {DECISION_COLUMNS} FROM decisions ORDER BY at, id"),
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
            "SELECT {DECISION_COLUMNS} FROM decisions WHERE state IN ({placeholders}) \
             ORDER BY at, id"
        ),
        filters,
    ))
}

fn read_decision_row(row: &Row<'_>) -> rusqlite::Result<DecisionRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
    ))
}

fn decision_from_row(row: DecisionRow) -> Result<Decision> {
    let (id, prompt, tasks_json, state, answer) = row;
    Ok(Decision {
        id: DecisionId::new(id),
        prompt,
        tasks: serde_json::from_str(&tasks_json)?,
        state: enum_from_text(&state)?,
        answer,
    })
}
