//! Events. The daemon stores each one, and the WebSocket sends it to every device.
//! The list matches `docs/architecture.md`, Events. Payloads hold ids and short values, never
//! whole files, keys, or note text.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::chat::{AttachmentRef, ChatChange};
use crate::cost::Cost;
use crate::dry_run::{DryRunStage, StageState};
use crate::ids::{ApprovalId, ChatId, DeviceId, NoteId, RoutineId, RunId, TaskId, ToolId};
use crate::intent::Intent;
use crate::note::LessonState;
use crate::permission::{Approval, ApprovalState, PermissionMode};
use crate::quality::QualityNote;
use crate::routines::RoutineKind;
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
    /// The type of a message, and who chose it: `router` or `you`.
    IntentChosen {
        chat: ChatId,
        intent: Intent,
        reason: String,
        by: String,
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
    /// A chat was created, renamed, pinned, archived, or deleted.
    ChatChanged {
        chat: ChatId,
        #[serde(flatten)]
        change: ChatChange,
    },
    /// The attachments that went with the message just before this event.
    AttachmentsSent {
        attachments: Vec<AttachmentRef>,
    },
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
    /// One stage of the dry run changed. The dry run uses a sample project, never the repository.
    DryRunStage {
        stage: DryRunStage,
        state: StageState,
    },
    /// The dry run ended. `verified` of `total` tasks passed on the sample project.
    DryRunFinished {
        verified: u32,
        total: u32,
    },
    /// A routine fired.
    RoutineFired {
        routine: RoutineId,
        routine_kind: RoutineKind,
    },
    /// A routine did not run, and why.
    RoutineSkipped {
        routine: RoutineId,
        why: String,
    },
    /// A brief was built. `text` is what the app and Telegram show.
    BriefReady {
        routine_kind: RoutineKind,
        title: String,
        text: String,
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
    /// The chat the event belongs to. Events of no chat, such as a tool scan, have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat: Option<ChatId>,
    #[serde(with = "time::serde::rfc3339")]
    pub at: OffsetDateTime,
    pub event: Event,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chat_change_event_round_trips_through_json() {
        let event = Event::ChatChanged {
            chat: ChatId::new("c1"),
            change: ChatChange::Renamed {
                title: "Rate limit".to_owned(),
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"kind\":\"ChatChanged\""), "{json}");
        assert!(json.contains("\"change\":\"renamed\""), "{json}");
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), event);
    }

    #[test]
    fn an_event_record_without_a_chat_omits_the_field() {
        let record = EventRecord {
            id: 1,
            chat: None,
            at: OffsetDateTime::UNIX_EPOCH,
            event: Event::HostHeartbeat,
        };
        assert!(!serde_json::to_string(&record).unwrap().contains("chat"));
    }

    #[test]
    fn a_dry_run_stage_event_round_trips_with_snake_case_names() {
        let e = Event::DryRunStage {
            stage: DryRunStage::AgentsWork,
            state: StageState::Running,
        };
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"kind\":\"DryRunStage\""), "{json}");
        assert!(json.contains("\"stage\":\"agents_work\""), "{json}");
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), e);
    }

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

    #[test]
    fn a_brief_event_round_trips_with_snake_case_names() {
        let event = Event::BriefReady {
            routine_kind: RoutineKind::Sundown,
            title: "Sundown wrap-up".to_owned(),
            text: "2 of 3 tasks verified.".to_owned(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(
            json.contains(r#""kind":"BriefReady""#) && json.contains(r#""routine_kind":"sundown""#)
        );
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), event);
    }
}
