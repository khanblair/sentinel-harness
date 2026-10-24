//! `Sidebar`: the run list grouped by repository, the New run button, search, and the footer
//! (`docs/ui-rules.md`, Layout).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::RunRequest;
use sentinel_core::{Event, EventRecord, HarnessState, Snapshot, TaskStatus};

use crate::api;
use crate::components::icon::Icon;
use crate::components::top_bar::settle;
use crate::prefs::{Resolved, Theme};
use crate::state::{Connection, Modal, ToastKind, Ui, use_ui};
use crate::theme;

const NO_REPO_GROUP: &str = "No repository";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Running,
    Paused,
    Done,
    Blocked,
    Stopped,
}

/// One row of the run list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRow {
    pub id: String,
    pub title: String,
    pub group: String,
    pub count: String,
    pub status: RunStatus,
}

/// True while the harness runs or is paused.
pub fn is_live(snapshot: &Snapshot) -> bool {
    matches!(
        snapshot.harness,
        HarnessState::Running | HarnessState::Paused
    )
}

/// The backlog file name, without its folders.
pub fn backlog_title(backlog: &str) -> String {
    backlog
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(backlog)
        .to_owned()
}

/// A run seen in the event log, before its status is known.
struct SeenRun {
    id: String,
    backlog: String,
    finished: Option<(u32, u32)>,
}

fn seen_runs(events: &[EventRecord]) -> Vec<SeenRun> {
    let mut seen: Vec<SeenRun> = Vec::new();
    for record in events {
        if let Event::RunStarted { run, backlog } = &record.event {
            seen.push(SeenRun {
                id: run.as_str().to_owned(),
                backlog: backlog.clone(),
                finished: None,
            });
        } else if let Event::RunFinished { verified, total } = &record.event {
            let pending = seen.iter_mut().rev().find(|run| run.finished.is_none());
            if let Some(open) = pending {
                open.finished = Some((*verified, *total));
            }
        }
    }
    seen
}

/// Every run in the event log, plus the live run when the log no longer holds its start.
pub fn runs_from(snapshot: &Snapshot) -> Vec<RunRow> {
    let group = snapshot
        .repo
        .as_ref()
        .map_or_else(|| NO_REPO_GROUP.to_owned(), |repo| repo.name.clone());
    let mut seen = seen_runs(&snapshot.events);
    let missing_live = snapshot
        .run
        .as_ref()
        .filter(|run| !seen.iter().any(|known| known.id == run.id.as_str()));
    if let Some(run) = missing_live {
        seen.push(SeenRun {
            id: run.id.as_str().to_owned(),
            backlog: run.backlog.clone(),
            finished: None,
        });
    }
    seen.into_iter()
        .map(|known| {
            let live = snapshot
                .run
                .as_ref()
                .is_some_and(|run| run.id.as_str() == known.id);
            build_row(snapshot, &group, known, live)
        })
        .collect()
}

fn build_row(snapshot: &Snapshot, group: &str, known: SeenRun, live: bool) -> RunRow {
    let (status, count) = if live {
        (live_status(snapshot), live_count(snapshot))
    } else if let Some((verified, total)) = known.finished {
        (RunStatus::Done, format!("{verified}/{total}"))
    } else {
        (RunStatus::Stopped, String::new())
    };
    RunRow {
        id: known.id,
        title: backlog_title(&known.backlog),
        group: group.to_owned(),
        count,
        status,
    }
}

fn live_status(snapshot: &Snapshot) -> RunStatus {
    if snapshot.harness == HarnessState::Running {
        RunStatus::Running
    } else if snapshot.harness == HarnessState::Paused {
        RunStatus::Paused
    } else if snapshot
        .tasks
        .iter()
        .any(|task| task.status == TaskStatus::Blocked)
    {
        RunStatus::Blocked
    } else {
        RunStatus::Stopped
    }
}

fn live_count(snapshot: &Snapshot) -> String {
    if snapshot.tasks.is_empty() {
        return String::new();
    }
    let verified = snapshot
        .tasks
        .iter()
        .filter(|task| task.status == TaskStatus::Verified)
        .count();
    format!("{verified}/{}", snapshot.tasks.len())
}

/// Rows grouped by repository, in the order each group first appears. Rows whose title does not
/// contain `needle` (lower case) are left out. An empty needle keeps every row.
pub fn grouped(rows: &[RunRow], needle: &str) -> Vec<(String, Vec<RunRow>)> {
    let mut groups: Vec<(String, Vec<RunRow>)> = Vec::new();
    for row in rows
        .iter()
        .filter(|row| needle.is_empty() || row.title.to_lowercase().contains(needle))
    {
        match groups.iter_mut().find(|(name, _)| *name == row.group) {
            Some((_, members)) => members.push(row.clone()),
            None => groups.push((row.group.clone(), vec![row.clone()])),
        }
    }
    groups
}

#[component]
pub fn Sidebar() -> impl IntoView {
    let search_open = RwSignal::new(false);
    let query = RwSignal::new(String::new());
    view! {
        <aside class="sb" id="sb" aria-label="Runs">
            <SidebarHeader search_open=search_open />
            <NewRunButton />
            <SearchRow search_open=search_open query=query />
            <RunList query=query />
            <SidebarFooter />
        </aside>
    }
}

#[component]
fn SidebarHeader(search_open: RwSignal<bool>) -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="sb-hd">
            <button class="ibtn" aria-label="Toggle sidebar" data-tip="Hide sidebar"
                on:click=move |_| ui.sidebar_open.update(|open| *open = !*open)>
                <Icon name="sidebar" size=18 />
            </button>
            <button class="ibtn" aria-label="Back" data-tip="Back" disabled=true>
                <Icon name="back" size=18 />
            </button>
            <button class="ibtn" aria-label="Forward" data-tip="Forward" disabled=true>
                <Icon name="fwd" size=18 />
            </button>
            <span class="sp"></span>
            <span class="segi">
                <button class="ibtn" aria-label="Find a run" data-tip="Find a run"
                    aria-pressed=move || search_open.get()
                    on:click=move |_| search_open.update(|open| *open = !*open)>
                    <Icon name="search" size=16 />
                </button>
                <button class="ibtn on" aria-label="Workspace">
                    <Icon name="code" size=16 />
                </button>
            </span>
        </div>
    }
}

#[component]
fn NewRunButton() -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="sb-new">
            <button class="btn pri" style="width:100%"
                disabled=move || ui.connection.get() != Connection::Live
                on:click=move |_| start_run(ui)>
                <Icon name="plus" size=15 />
                <span>"New run"</span>
            </button>
        </div>
    }
}

fn start_run(ui: Ui) {
    if ui.snapshot.with_untracked(is_live) {
        ui.show_toast("A run is already live.", ToastKind::Info);
        return;
    }
    spawn_local(async move {
        settle(ui, api::start_run(&RunRequest::default()).await);
    });
}

#[component]
fn SearchRow(search_open: RwSignal<bool>, query: RwSignal<String>) -> impl IntoView {
    view! {
        <div class="sb-search" hidden=move || !search_open.get()>
            <input placeholder="Search runs" aria-label="Search runs"
                prop:value=move || query.get()
                on:input=move |ev| query.set(event_target_value(&ev)) />
        </div>
    }
}

#[component]
fn RunList(query: RwSignal<String>) -> impl IntoView {
    let ui = use_ui();
    let closed_groups = RwSignal::new(Vec::<String>::new());
    view! {
        <div class="sb-scroll">
            {move || {
                let needle = query.get().trim().to_lowercase();
                let rows = ui.snapshot.with(runs_from);
                let groups = grouped(&rows, &needle);
                if groups.is_empty() {
                    let text = if rows.is_empty() { "No runs yet." } else { "No runs match." };
                    return view! { <div class="note" style="padding:1em">{text}</div> }.into_any();
                }
                groups
                    .into_iter()
                    .map(|(group, members)| group_view(group, members, closed_groups))
                    .collect_view()
                    .into_any()
            }}
        </div>
    }
}

fn group_view(group: String, rows: Vec<RunRow>, closed: RwSignal<Vec<String>>) -> impl IntoView {
    let is_closed = closed.with(|list| list.contains(&group));
    let toggle_name = group.clone();
    view! {
        <div class="grp" class:closed=is_closed
            on:click=move |_| toggle_group(closed, &toggle_name)>
            {group}
            <Icon name="down" size=12 />
        </div>
        {(!is_closed).then(|| rows.into_iter().map(run_row).collect_view())}
    }
}

fn toggle_group(closed: RwSignal<Vec<String>>, group: &str) {
    closed.update(|list| {
        if let Some(index) = list.iter().position(|name| name == group) {
            list.remove(index);
        } else {
            list.push(group.to_owned());
        }
    });
}

fn run_row(row: RunRow) -> impl IntoView {
    let active = matches!(row.status, RunStatus::Running | RunStatus::Paused);
    let icon = status_icon(row.status);
    view! {
        <button class="run" class:active=active>
            <span class="t">{row.title}</span>
            <span class="meta">
                <span>{row.count}</span>
                {icon}
            </span>
        </button>
    }
}

fn status_icon(status: RunStatus) -> AnyView {
    match status {
        RunStatus::Running => view! { <span class="spin" title="Running"></span> }.into_any(),
        RunStatus::Paused => view! {
            <span style="color:var(--warn)" title="Paused"><Icon name="pause" size=14 /></span>
        }
        .into_any(),
        RunStatus::Done => view! {
            <span style="color:var(--ok)" title="Finished"><Icon name="check" size=14 /></span>
        }
        .into_any(),
        RunStatus::Blocked => view! {
            <span style="color:var(--danger)" title="Has blocked tasks"><Icon name="alert" size=14 /></span>
        }
        .into_any(),
        RunStatus::Stopped => view! {
            <span title="Stopped"><Icon name="stop" size=13 /></span>
        }
        .into_any(),
    }
}

#[component]
fn SidebarFooter() -> impl IntoView {
    let ui = use_ui();
    let dark = move || ui.prefs.get().resolve(theme::system_is_dark()) == Resolved::Dark;
    view! {
        <div class="sb-ft">
            <div class="me">
                <span class="av">"Y"</span>
                <span class="nm">"You · local daemon"</span>
                <button class="ibtn" data-tip=move || if dark() { "Light theme" } else { "Dark theme" }
                    aria-label=move || if dark() { "Switch to light theme" } else { "Switch to dark theme" }
                    on:click=move |_| toggle_theme(ui)>
                    {move || if dark() {
                        view! { <Icon name="sun" size=16 /> }.into_any()
                    } else {
                        view! { <Icon name="moon" size=16 /> }.into_any()
                    }}
                </button>
                <button class="ibtn" data-tip="Settings" aria-label="Settings"
                    on:click=move |_| ui.modal.set(Some(Modal::Settings { page: "general".to_owned() }))>
                    <Icon name="gear" size=16 />
                </button>
            </div>
        </div>
    }
}

fn toggle_theme(ui: Ui) {
    let dark = ui.prefs.get_untracked().resolve(theme::system_is_dark()) == Resolved::Dark;
    ui.prefs.update(|prefs| {
        prefs.theme = if dark { Theme::Light } else { Theme::Dark };
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{RepoInfo, RunId, RunInfo, Task, TaskId};
    use time::OffsetDateTime;

    fn record(id: i64, event: Event) -> EventRecord {
        EventRecord {
            chat: None,
            id,
            at: OffsetDateTime::UNIX_EPOCH,
            event,
        }
    }

    fn started(run: &str, backlog: &str) -> Event {
        Event::RunStarted {
            run: RunId::new(run),
            backlog: backlog.to_owned(),
        }
    }

    #[test]
    fn no_events_give_no_rows() {
        assert_eq!(runs_from(&Snapshot::empty()), Vec::<RunRow>::new());
    }

    #[test]
    fn a_started_then_finished_run_gives_one_done_row() {
        let snapshot = Snapshot {
            events: vec![
                record(1, started("r1", "docs/backlog.md")),
                record(
                    2,
                    Event::RunFinished {
                        verified: 2,
                        total: 3,
                    },
                ),
            ],
            ..Snapshot::empty()
        };
        let rows = runs_from(&snapshot);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "backlog.md");
        assert_eq!(rows[0].status, RunStatus::Done);
        assert_eq!(rows[0].count, "2/3");
    }

    #[test]
    fn a_live_run_gives_a_running_row_with_its_verified_count() {
        let mut verified = Task::new(TaskId::new("T1"), "One", "");
        verified.status = TaskStatus::Verified;
        let pending = Task::new(TaskId::new("T2"), "Two", "");
        let snapshot = Snapshot {
            harness: HarnessState::Running,
            run: Some(RunInfo {
                id: RunId::new("r1"),
                chat: sentinel_core::ChatId::new("c1"),
                backlog: "backlog.md".to_owned(),
                wave: 1,
                started_at: OffsetDateTime::UNIX_EPOCH,
            }),
            tasks: vec![verified, pending],
            repo: Some(RepoInfo {
                path: "/repo".to_owned(),
                name: "fee-ledger-api".to_owned(),
                is_git: true,
                has_backlog: true,
                clean: true,
                backlog_tasks: 2,
            }),
            ..Snapshot::empty()
        };
        let rows = runs_from(&snapshot);
        assert_eq!(rows[0].status, RunStatus::Running);
        assert_eq!(rows[0].count, "1/2");
        assert_eq!(rows[0].group, "fee-ledger-api");
    }

    #[test]
    fn backlog_title_drops_the_folders() {
        assert_eq!(backlog_title("a/b/backlog.md"), "backlog.md");
        assert_eq!(backlog_title("backlog.md"), "backlog.md");
    }

    #[test]
    fn grouping_keeps_first_seen_order_and_filters_by_title() {
        let row = |group: &str, title: &str| RunRow {
            id: title.to_owned(),
            title: title.to_owned(),
            group: group.to_owned(),
            count: String::new(),
            status: RunStatus::Stopped,
        };
        let rows = vec![
            row("api", "fees.md"),
            row("web", "login.md"),
            row("api", "tax.md"),
        ];
        let all = grouped(&rows, "");
        assert_eq!(
            all.iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["api", "web"]
        );
        assert_eq!(all[0].1.len(), 2);
        let filtered = grouped(&rows, "tax");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].1[0].title, "tax.md");
    }
}
