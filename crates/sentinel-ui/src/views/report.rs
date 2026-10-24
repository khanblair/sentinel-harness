//! The Report panel: the honest per-task report, reasons for tasks not done, and an export
//! (`docs/ui-rules.md`, Views).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::{Event, EventRecord, Task, TaskId, TaskStatus};

use crate::api;
use crate::components::icon::Icon;
use crate::components::receipt_block::ReceiptBlock;
use crate::components::status_badge::StatusBadge;
use crate::components::task_card::CardModel;
use crate::state::{ToastKind, Ui, use_ui};
use crate::views::board::{columns, owner_of};
use crate::views::changes::short_sha;

/// One task that is not done, with the verifier's reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasonRow {
    pub id: String,
    pub title: String,
    pub reason: String,
}

/// Badge status and count per status group, in board order.
pub fn status_counts(tasks: &[Task]) -> Vec<(TaskStatus, usize)> {
    columns(tasks, &[])
        .into_iter()
        .filter_map(|column| {
            let first = column.cards.first().map(|card: &CardModel| card.status)?;
            Some((first, column.cards.len()))
        })
        .collect()
}

/// The commit of the latest checkpoint for a task, short form.
pub fn receipt_for(task: &TaskId, events: &[EventRecord]) -> Option<String> {
    events.iter().rev().find_map(|record| {
        let Event::Checkpoint {
            task: from, commit, ..
        } = &record.event
        else {
            return None;
        };
        (from == task).then(|| short_sha(commit))
    })
}

/// The reason from the latest verdict for a task.
pub fn verdict_reason(task: &TaskId, events: &[EventRecord]) -> Option<String> {
    events.iter().rev().find_map(|record| {
        let Event::Verdict {
            task: from, reason, ..
        } = &record.event
        else {
            return None;
        };
        (from == task).then(|| reason.clone())
    })
}

/// Tasks that are not verified and have a reason, in backlog order.
pub fn reason_rows(tasks: &[Task], events: &[EventRecord]) -> Vec<ReasonRow> {
    tasks
        .iter()
        .filter(|task| task.status != TaskStatus::Verified)
        .filter_map(|task| {
            let reason = verdict_reason(&task.id, events)?;
            Some(ReasonRow {
                id: task.id.to_string(),
                title: task.title.clone(),
                reason,
            })
        })
        .collect()
}

/// The markdown text from `GET /api/report`.
pub fn markdown_from(value: &serde_json::Value) -> Option<String> {
    value
        .get("markdown")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

/// Percent-encodes everything except unreserved characters, so the text fits in a data URL.
pub fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

/// A download link for the markdown report. It needs no Blob or object URL.
pub fn export_href(markdown: &str) -> String {
    format!(
        "data:text/markdown;charset=utf-8,{}",
        percent_encode(markdown)
    )
}

#[component]
fn ReportRows(tasks: Memo<Vec<Task>>) -> impl IntoView {
    let ui = use_ui();
    view! {
        <table class="dl">
            <tr><th>"Task"</th><th>"Status"</th><th>"Agent"</th><th>"Receipt"</th></tr>
            {move || {
                ui.snapshot.with(|s| {
                    tasks.get().into_iter().map(|task| {
                        let owner = owner_of(&task.id, &s.events).map_or("-".to_owned(), |tool| tool.to_string());
                        let receipt = (task.status == TaskStatus::Verified)
                            .then(|| receipt_for(&task.id, &s.events))
                            .flatten();
                        view! {
                            <tr>
                                <td><span class="mono">{task.id.to_string()}</span><div class="sub">{task.title.clone()}</div></td>
                                <td><StatusBadge status=task.status /></td>
                                <td>{owner}</td>
                                <td class="mono"><ReceiptBlock receipt=receipt /></td>
                            </tr>
                        }
                    }).collect_view()
                })
            }}
        </table>
    }
}

#[component]
fn ReasonList(tasks: Memo<Vec<Task>>) -> impl IntoView {
    let ui = use_ui();
    view! {
        {move || {
            let rows = ui.snapshot.with(|s| reason_rows(&tasks.get(), &s.events));
            (!rows.is_empty()).then(|| view! {
                <div class="psec" style="margin-top:1em">
                    <h5>"Why not done"</h5>
                    {rows.into_iter().map(|row| view! {
                        <div class="tcard"><span class="mono">{row.id}</span>" "{row.title}<div class="sub">{row.reason}</div></div>
                    }).collect_view()}
                </div>
            })
        }}
    }
}

#[component]
fn ExportLink(markdown: RwSignal<Option<String>>) -> impl IntoView {
    view! {
        <div class="row" style="margin-top:1em">
            {move || match markdown.get() {
                Some(text) => view! {
                    <a class="btn" download="report.md" href=export_href(&text)>
                        <Icon name="download" size=15 /> " Export report"
                    </a>
                }
                .into_any(),
                None => view! { <button class="btn" disabled=true>"Export report"</button> }.into_any(),
            }}
        </div>
    }
}

#[component]
pub fn ReportPanel() -> impl IntoView {
    let ui = use_ui();
    let tasks = Memo::new(move |_| ui.snapshot.with(|s| s.tasks.clone()));
    let counts = move || ui.snapshot.with(|s| status_counts(&s.tasks));
    let markdown: RwSignal<Option<String>> = RwSignal::new(None);
    load_markdown(ui, markdown);
    view! {
        <Show when=move || !tasks.get().is_empty()
            fallback=|| view! {
                <div class="note">
                    <Icon name="info" size=14 />
                    <span>"The report fills in as tasks finish. Status comes from tests, not from agent claims."</span>
                </div>
            }>
            <div class="row wrap" style="gap:.9em;margin-bottom:.8em">
                {move || counts().into_iter().map(|(status, count)| view! {
                    <span><StatusBadge status=status /> <b>{count}</b></span>
                }).collect_view()}
            </div>
            <ReportRows tasks=tasks />
            <ReasonList tasks=tasks />
            <ExportLink markdown=markdown />
        </Show>
    }
}

fn load_markdown(ui: Ui, markdown: RwSignal<Option<String>>) {
    spawn_local(async move {
        match api::get::<serde_json::Value>("/api/report").await {
            Ok(value) => markdown.set(markdown_from(&value)),
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::TaskId;
    use time::OffsetDateTime;

    fn record(id: i64, event: Event) -> EventRecord {
        EventRecord {
            id,
            at: OffsetDateTime::UNIX_EPOCH,
            event,
        }
    }

    fn task(id: &str, status: TaskStatus) -> Task {
        let mut task = Task::new(TaskId::new(id), format!("{id} title"), "body");
        task.status = status;
        task
    }

    #[test]
    fn status_counts_group_by_board_column() {
        let tasks = vec![
            task("T-01", TaskStatus::Planned),
            task("T-02", TaskStatus::Ready),
            task("T-03", TaskStatus::Verified),
        ];
        let counts = status_counts(&tasks);
        assert_eq!(
            counts,
            vec![(TaskStatus::Planned, 2), (TaskStatus::Verified, 1)]
        );
    }

    #[test]
    fn receipt_is_the_short_commit_of_the_latest_checkpoint() {
        let events = vec![
            record(
                1,
                Event::Checkpoint {
                    task: TaskId::new("T-01"),
                    commit: "1111111aaa".to_owned(),
                    paths: vec![],
                },
            ),
            record(
                2,
                Event::Checkpoint {
                    task: TaskId::new("T-01"),
                    commit: "2222222bbb".to_owned(),
                    paths: vec![],
                },
            ),
        ];
        assert_eq!(
            receipt_for(&TaskId::new("T-01"), &events).as_deref(),
            Some("2222222")
        );
    }

    #[test]
    fn reasons_skip_verified_tasks() {
        let tasks = vec![
            task("T-01", TaskStatus::Verified),
            task("T-02", TaskStatus::Blocked),
        ];
        let events = vec![
            record(
                1,
                Event::Verdict {
                    task: TaskId::new("T-01"),
                    status: TaskStatus::Verified,
                    reason: "tests pass".to_owned(),
                },
            ),
            record(
                2,
                Event::Verdict {
                    task: TaskId::new("T-02"),
                    status: TaskStatus::Blocked,
                    reason: "tests fail".to_owned(),
                },
            ),
        ];
        let rows = reason_rows(&tasks, &events);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].reason, "tests fail");
    }

    #[test]
    fn markdown_is_read_from_the_report_body() {
        let body = serde_json::json!({ "markdown": "# Report" });
        assert_eq!(markdown_from(&body).as_deref(), Some("# Report"));
        assert_eq!(markdown_from(&serde_json::json!({})), None);
    }

    #[test]
    fn export_href_encodes_spaces_and_newlines() {
        assert_eq!(percent_encode("a b\n"), "a%20b%0A");
        assert_eq!(
            export_href("# R"),
            "data:text/markdown;charset=utf-8,%23%20R"
        );
    }
}
