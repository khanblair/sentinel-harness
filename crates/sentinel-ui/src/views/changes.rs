//! The Changes panel: checkpoint commits, blocked writes, and quality notes, from the event log
//! (`docs/ui-rules.md`, Views).

use leptos::prelude::*;
use sentinel_core::{Event, EventRecord, QualityKind};

use crate::components::icon::Icon;
use crate::state::use_ui;
use crate::views::board::owner_of;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointView {
    pub task: String,
    pub owner: Option<String>,
    pub short_sha: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedView {
    pub path: String,
    pub task: Option<String>,
    pub restored: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityView {
    pub task: String,
    pub kind: &'static str,
    pub location: String,
    pub detail_code: String,
}

/// The first seven characters of a commit id, as `git` shows it.
pub fn short_sha(commit: &str) -> String {
    commit.chars().take(7).collect()
}

pub fn kind_label(kind: QualityKind) -> &'static str {
    match kind {
        QualityKind::Size => "Size",
        QualityKind::Duplication => "Duplication",
        QualityKind::Slop => "Slop",
    }
}

/// Checkpoints, newest first.
pub fn checkpoints(events: &[EventRecord]) -> Vec<CheckpointView> {
    events
        .iter()
        .rev()
        .filter_map(|record| {
            let Event::Checkpoint {
                task,
                commit,
                paths,
            } = &record.event
            else {
                return None;
            };
            Some(CheckpointView {
                task: task.to_string(),
                owner: owner_of(task, events).map(|tool| tool.to_string()),
                short_sha: short_sha(commit),
                paths: paths.clone(),
            })
        })
        .collect()
}

/// Blocked writes, newest first.
pub fn blocked_writes(events: &[EventRecord]) -> Vec<BlockedView> {
    events
        .iter()
        .rev()
        .filter_map(|record| {
            let Event::WriteBlocked {
                task,
                path,
                restored,
            } = &record.event
            else {
                return None;
            };
            Some(BlockedView {
                path: path.clone(),
                task: task.as_ref().map(ToString::to_string),
                restored: *restored,
            })
        })
        .collect()
}

/// Quality notes, newest first.
pub fn quality_views(events: &[EventRecord]) -> Vec<QualityView> {
    events
        .iter()
        .rev()
        .filter_map(|record| {
            let Event::QualityNote { note } = &record.event else {
                return None;
            };
            Some(QualityView {
                task: note.task.to_string(),
                kind: kind_label(note.kind),
                location: note.location.clone(),
                detail_code: note.detail_code.clone(),
            })
        })
        .collect()
}

/// The text that tells the user how to see a diff, since the daemon has no diff route yet.
pub fn diff_hint(commit: &str) -> String {
    format!("Open the diff in your editor with `git show {commit}`.")
}

#[component]
fn CheckpointCard(view: CheckpointView) -> impl IntoView {
    let CheckpointView {
        task,
        owner,
        short_sha,
        paths,
    } = view;
    let hint = diff_hint(&short_sha);
    view! {
        <div class="tcard">
            <div class="row sp">
                <span class="mono">{short_sha}</span>
                <span class="row" style="gap:.4em">
                    {owner.map(|agent| view! { <span>{agent}</span> })}
                    <span class="mono">{task}</span>
                </span>
            </div>
            <div class="tree" style="margin-top:.4em">
                {paths.into_iter().map(|path| view! { <div class="tr"><span class="nm">{path}</span></div> }).collect_view()}
            </div>
            <div class="sub" style="margin-top:.4em">{hint}</div>
        </div>
    }
}

#[component]
fn BlockedCard(view: BlockedView) -> impl IntoView {
    let state = if view.restored { "restored" } else { "blocked" };
    let who = view.task.unwrap_or_else(|| "no task".to_owned());
    view! {
        <div class="tcard">
            <div class="row sp">
                <span class="mono">{view.path}</span>
                <span class="stb" style="color:var(--danger)"><Icon name="shield" size=12 /> {state}</span>
            </div>
            <div class="meta">{who}</div>
        </div>
    }
}

#[component]
fn QualityCard(view: QualityView) -> impl IntoView {
    view! {
        <div class="tcard">
            <div class="row sp">
                <span class="mono">{view.task}</span>
                <span class="chip">{view.kind}</span>
            </div>
            <div class="meta"><span class="mono">{view.location}</span><span>{view.detail_code}</span></div>
        </div>
    }
}

#[component]
pub fn ChangesPanel() -> impl IntoView {
    let ui = use_ui();
    let checks = move || ui.snapshot.with(|s| checkpoints(&s.events));
    let blocked = move || ui.snapshot.with(|s| blocked_writes(&s.events));
    let quality = move || ui.snapshot.with(|s| quality_views(&s.events));
    view! {
        <Show when=move || checks().is_empty() fallback=|| ()>
            <div class="note">
                <Icon name="info" size=14 />
                <span>"No checkpoints yet. Each finished task becomes one commit, made without a checkout."</span>
            </div>
        </Show>
        {move || checks().into_iter().map(|view| view! { <CheckpointCard view=view /> }).collect_view()}
        <Show when=move || !blocked().is_empty() fallback=|| ()>
            <div class="psec" style="margin-top:1em">
                <h5>{move || format!("Blocked writes · {}", blocked().len())}</h5>
                {move || blocked().into_iter().map(|view| view! { <BlockedCard view=view /> }).collect_view()}
            </div>
        </Show>
        <Show when=move || !quality().is_empty() fallback=|| ()>
            <div class="psec" style="margin-top:1em">
                <h5>{move || format!("Quality notes · {}", quality().len())}</h5>
                {move || quality().into_iter().map(|view| view! { <QualityCard view=view /> }).collect_view()}
            </div>
        </Show>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{QualityNote, TaskId, ToolId};
    use time::OffsetDateTime;

    fn record(id: i64, event: Event) -> EventRecord {
        EventRecord {
            id,
            at: OffsetDateTime::UNIX_EPOCH,
            event,
        }
    }

    #[test]
    fn short_sha_keeps_seven_characters() {
        assert_eq!(short_sha("0123456789abcdef"), "0123456");
        assert_eq!(short_sha("abc"), "abc");
    }

    #[test]
    fn checkpoints_are_newest_first_with_owner() {
        let events = vec![
            record(
                1,
                Event::Checkpoint {
                    task: TaskId::new("T-01"),
                    commit: "aaaaaaa1".to_owned(),
                    paths: vec!["src/a.rs".to_owned()],
                },
            ),
            record(
                2,
                Event::Checkpoint {
                    task: TaskId::new("T-02"),
                    commit: "bbbbbbb2".to_owned(),
                    paths: vec![],
                },
            ),
        ];
        let views = checkpoints(&events);
        assert_eq!(views[0].task, "T-02");
        assert_eq!(views[0].short_sha, "bbbbbbb");
        assert_eq!(views[1].paths, vec!["src/a.rs".to_owned()]);
        assert_eq!(views[0].owner, None);
    }

    #[test]
    fn blocked_writes_keep_the_restored_flag() {
        let events = vec![record(
            1,
            Event::WriteBlocked {
                task: None,
                path: "src/x.rs".to_owned(),
                restored: true,
            },
        )];
        let views = blocked_writes(&events);
        assert_eq!(views[0].task, None);
        assert!(views[0].restored);
    }

    #[test]
    fn quality_notes_name_their_kind() {
        let events = vec![record(
            1,
            Event::QualityNote {
                note: QualityNote {
                    task: TaskId::new("T-03"),
                    kind: QualityKind::Duplication,
                    location: "src/b.rs:9".to_owned(),
                    detail_code: "copy_paste".to_owned(),
                },
            },
        )];
        assert_eq!(quality_views(&events)[0].kind, "Duplication");
    }

    #[test]
    fn diff_hint_names_the_commit() {
        assert_eq!(
            diff_hint("abc1234"),
            "Open the diff in your editor with `git show abc1234`."
        );
    }

    #[test]
    fn owner_comes_from_the_goes_to_message() {
        let events = vec![
            record(
                1,
                Event::Message {
                    from: "sentinel".to_owned(),
                    to: Some(ToolId::new("claude")),
                    text: "T-01 goes to Claude (easy task).".to_owned(),
                },
            ),
            record(
                2,
                Event::Checkpoint {
                    task: TaskId::new("T-01"),
                    commit: "ccccccc".to_owned(),
                    paths: vec![],
                },
            ),
        ];
        assert_eq!(checkpoints(&events)[0].owner.as_deref(), Some("claude"));
    }
}
