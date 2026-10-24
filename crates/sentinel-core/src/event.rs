//! Events. The daemon stores each one, and the WebSocket sends it to every device.
//! The list matches `docs/architecture.md`, Events. Payloads hold ids and short values, never
//! whole files, keys, or note text.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::cost::Cost;
use crate::ids::{ApprovalId, DeviceId, NoteId, RunId, TaskId, ToolId};
use crate::note::LessonState;
use crate::permission::{Approval, ApprovalState, PermissionMode};
use crate::quality::QualityNote;
use crate::state::{ConnectionState, Decision, HarnessState, TailscaleState};
use crate::task::TaskStatus;
use crate::tool::{ToolInfo, ToolStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Event {
    RunStarted {
        run: RunId,
        backlog: String,
    },
    PlanReady {
        tasks: u32,
        contradictions: u32,
        already_done: u32,
    },
    WaveStarted {
        wave: u32,
        tasks: Vec<TaskId>,
    },
    /// The verifier's decision. Only the verifier may produce `Verified`.
    Verdict {
        task: TaskId,
        status: TaskStatus,
        reason: String,
    },
    RunFinished {
        verified: u32,
        total: u32,
    },
    ReportReady,
    /// Text in the stream: a user instruction, a Sentinel note, or a message to one agent.
    Message {
        from: String,
        to: Option<ToolId>,
        text: String,
    },
    TaskStatus {
        task: TaskId,
        status: TaskStatus,
    },
    FilesReserved {
        task: TaskId,
        paths: Vec<String>,
    },
    FilesReleased {
        task: TaskId,
        paths: Vec<String>,
    },
    /// A write to a file the task had not reserved. `restored` is true once the file is back.
    WriteBlocked {
        task: Option<TaskId>,
        path: String,
        restored: bool,
    },
    Checkpoint {
        task: TaskId,
        commit: String,
        paths: Vec<String>,
    },
    BarrierStart {
        wave: u32,
    },
    BarrierEnd {
        wave: u32,
        passed: bool,
    },
    CostUpdate {
        total: Cost,
        cap: Cost,
    },
    /// 70 or 95.
    BudgetThreshold {
        percent: u8,
    },
    ToolDetected {
        tool: ToolInfo,
    },
    AgentOutput {
        task: TaskId,
        agent: ToolId,
        line: String,
    },
    DecisionRequested {
        decision: Decision,
    },
    HarnessState {
        state: HarnessState,
    },
    DeviceAction {
        device: DeviceId,
        action: String,
    },
    NoteWritten {
        note: NoteId,
    },
    LessonStateChanged {
        note: NoteId,
        state: LessonState,
    },
    ProposalCreated {
        proposal: NoteId,
    },
    CareFinished {
        summary: String,
    },
    QualityNote {
        note: QualityNote,
    },
    ContextCompacted {
        task: TaskId,
        tokens_before: u32,
        tokens_after: u32,
    },
    TaskCeilingHit {
        task: TaskId,
        cost: Cost,
        ceiling: Cost,
    },
    ApprovalRequested {
        approval: Approval,
    },
    ApprovalResolved {
        approval: ApprovalId,
        state: ApprovalState,
        decided_by: String,
    },
    PermissionChanged {
        mode: PermissionMode,
    },
    ToolStatusChanged {
        tool: ToolId,
        status: ToolStatus,
    },
    ConnectionChanged {
        name: String,
        state: ConnectionState,
    },
    TailscaleChanged {
        state: TailscaleState,
    },
    TelegramPaired {
        user_id: i64,
    },
    TelegramSent {
        summary: String,
    },
    TelegramReceived {
        accepted: bool,
        command: String,
    },
    HostHeartbeat,
    HostBackOnline {
        gap_seconds: u64,
    },
}

impl Event {
    /// The variant name, such as `TaskStatus`. Used as the `kind` column in the store.
    pub fn kind(&self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.get("kind").and_then(|k| k.as_str().map(str::to_owned)))
            .unwrap_or_default()
    }

    /// The task this event is about, when it has one.
    #[allow(
        clippy::wildcard_enum_match_arm,
        reason = "only some events name a task"
    )]
    pub fn task(&self) -> Option<&TaskId> {
        match self {
            Event::TaskStatus { task, .. }
            | Event::FilesReserved { task, .. }
            | Event::FilesReleased { task, .. }
            | Event::Checkpoint { task, .. }
            | Event::AgentOutput { task, .. }
            | Event::ContextCompacted { task, .. }
            | Event::TaskCeilingHit { task, .. }
            | Event::Verdict { task, .. } => Some(task),
            Event::WriteBlocked { task, .. } => task.as_ref(),
            _ => None,
        }
    }
}

/// An event with its place in the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub at: OffsetDateTime,
    pub event: Event,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_is_the_variant_name() {
        let e = Event::BarrierStart { wave: 2 };
        assert_eq!(e.kind(), "BarrierStart");
        assert_eq!(Event::HostHeartbeat.kind(), "HostHeartbeat");
    }

    #[test]
    fn task_returns_the_subject_task() {
        let e = Event::Checkpoint {
            task: TaskId::new("T-01"),
            commit: "abc".into(),
            paths: vec![],
        };
        assert_eq!(e.task(), Some(&TaskId::new("T-01")));
        assert_eq!(
            Event::BarrierEnd {
                wave: 1,
                passed: true
            }
            .task(),
            None
        );
    }

    #[test]
    fn event_round_trips_with_a_kind_tag() {
        let e = Event::WriteBlocked {
            task: None,
            path: "src/x.rs".into(),
            restored: true,
        };
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"kind\":\"WriteBlocked\""), "{json}");
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), e);
    }

    #[test]
    fn verdict_names_its_task() {
        let e = Event::Verdict {
            task: TaskId::new("T-02"),
            status: TaskStatus::Verified,
            reason: "ok".into(),
        };
        assert_eq!(e.task(), Some(&TaskId::new("T-02")));
        assert_eq!(e.kind(), "Verdict");
    }

    #[test]
    fn cost_update_round_trips() {
        let e = Event::CostUpdate {
            total: Cost::from_micro_usd(5, true),
            cap: Cost::from_micro_usd(2_000_000, false),
        };
        let back: Event = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back, e);
    }
}
