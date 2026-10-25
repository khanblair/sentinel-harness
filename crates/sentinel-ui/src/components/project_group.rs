//! `ProjectGroup`: one project in the sidebar, with its header, its chats, and its archived chats.
//! The pure model below decides which projects show and which chats each one holds
//! (`docs/ui-rules.md`, Layout).

use leptos::prelude::*;
use sentinel_core::ChatInfo;
use sentinel_core::api::ProjectInfo;

use crate::api::Then;
use crate::chats::{ARCHIVED_GROUP, ChatRow as RowData, project_rows};
use crate::components::add_project::{ProjectDialog, switch_project};
use crate::components::chat_row::{ChatRow, PlainChatRow};
use crate::components::icon::Icon;
use crate::components::menus::MenuItem;
use crate::components::sidebar::new_chat;
use crate::state::{Ui, use_ui};

/// One project's group: its header data and the rows it shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectGroupModel {
    pub id: String,
    pub name: String,
    pub primary: bool,
    pub live: bool,
    /// True for the project the page is showing. Its rows are live and have menus.
    pub active: bool,
    /// The id a switch saves for this project. `None` for the primary project.
    pub stored: Option<String>,
    pub chats: Vec<RowData>,
    pub archived: Vec<RowData>,
}

/// True for the project the sidebar treats as active: the saved id, or the primary project when
/// no id is saved.
pub fn is_active(project: &ProjectInfo, active: Option<&str>) -> bool {
    active.map_or(project.primary, |id| project.id == id)
}

/// The id a switch saves for project `id`. The primary project saves nothing.
pub fn stored_id(id: &str, primary: bool) -> Option<String> {
    (!primary).then(|| id.to_owned())
}

/// One group per project, in the order of `projects`. The active project shows the live snapshot
/// chats. The others show the chats the list gave. While a search runs, a project with no match
/// is dropped.
pub fn sidebar_groups(
    projects: &[ProjectInfo],
    active: Option<&str>,
    snapshot_chats: &[ChatInfo],
    query: &str,
) -> Vec<ProjectGroupModel> {
    let searching = !query.trim().is_empty();
    projects
        .iter()
        .filter_map(|project| {
            let is_current = is_active(project, active);
            let source = if is_current {
                snapshot_chats
            } else {
                project.chats.as_slice()
            };
            let (chats, archived) = project_rows(source, query);
            let no_match = chats.is_empty() && archived.is_empty();
            (!(searching && no_match)).then(|| ProjectGroupModel {
                id: project.id.clone(),
                name: project.name.clone(),
                primary: project.primary,
                live: project.live,
                active: is_current,
                stored: stored_id(&project.id, project.primary),
                chats,
                archived,
            })
        })
        .collect()
}

/// Which groups are folded. Each list holds project ids. The lists are only read and written
/// through `Folds`, so the sidebar can pass them as one value.
#[derive(Debug, Clone, Copy)]
pub struct Folds {
    /// Projects whose chats are hidden.
    pub closed: RwSignal<Vec<String>>,
    /// Projects whose archived chats are shown. Archived chats start hidden.
    pub archived_open: RwSignal<Vec<String>>,
}

fn toggle(list: RwSignal<Vec<String>>, key: &str) {
    list.update(|keys| match keys.iter().position(|open| open == key) {
        Some(index) => {
            keys.remove(index);
        }
        None => keys.push(key.to_owned()),
    });
}

/// One project: its header, its chats, and its archived chats. The caller decides `closed` and
/// `archived_open`, so a search opens every group.
#[component]
pub fn ProjectGroupView(
    group: ProjectGroupModel,
    folds: Folds,
    closed: bool,
    archived_open: bool,
) -> impl IntoView {
    let header = header_view(&group, folds, closed);
    let archived = archived_view(&group, folds, archived_open);
    let ProjectGroupModel {
        stored,
        active,
        chats,
        ..
    } = group;
    let rows = (!closed).then(|| {
        chats
            .into_iter()
            .map(|row| row_view(row, stored.clone(), active))
            .collect_view()
    });
    view! {
        {header}
        {rows}
        {archived}
    }
}

fn header_view(group: &ProjectGroupModel, folds: Folds, closed: bool) -> AnyView {
    let ui = use_ui();
    let key = group.id.clone();
    let label = format!("New chat in {}", group.name);
    let (active, stored) = (group.active, group.stored.clone());
    view! {
        <div class="grp" class:closed=closed on:click=move |_| toggle(folds.closed, &key)>
            {group.name.clone()}
            <Icon name="down" size=12 />
            {group.live.then(|| view! { <span class="dot" title="A run is live here"></span> })}
            <button class="ibtn plus" aria-label=label.clone() data-tip=label
                on:click=move |event| {
                    event.stop_propagation();
                    new_chat_in(ui, active, stored.as_deref());
                }>
                <Icon name="plus" size=14 />
            </button>
            <RemoveMenu id=group.id.clone() name=group.name.clone() primary=group.primary />
        </div>
    }
    .into_any()
}

/// Makes a chat in the project. The active project makes it here. Another project is switched to
/// first, and the page opens on the new chat.
fn new_chat_in(ui: Ui, active: bool, stored: Option<&str>) {
    if active {
        new_chat(ui);
    } else {
        switch_project(stored, &Then::NewChat);
    }
}

/// The archived chats of a project, under their own folded header.
fn archived_view(group: &ProjectGroupModel, folds: Folds, archived_open: bool) -> AnyView {
    if group.archived.is_empty() {
        return ().into_any();
    }
    let key = group.id.clone();
    let stored = group.stored.clone();
    let active = group.active;
    let rows = archived_open.then(|| {
        group
            .archived
            .iter()
            .map(|row| row_view(row.clone(), stored.clone(), active))
            .collect_view()
    });
    view! {
        <div class="grp" class:closed=!archived_open on:click=move |_| toggle(folds.archived_open, &key)>
            {ARCHIVED_GROUP}
            <Icon name="down" size=12 />
        </div>
        {rows}
    }
    .into_any()
}

/// A live row for the active project, and a plain switching row for any other project.
fn row_view(row: RowData, stored: Option<String>, active: bool) -> AnyView {
    if active {
        view! { <ChatRow row=row /> }.into_any()
    } else {
        view! { <PlainChatRow row=row project=stored /> }.into_any()
    }
}

/// The project's small menu. It holds one item, "Remove from the list", for a computer and a
/// project that is not the primary one.
#[component]
fn RemoveMenu(id: String, name: String, primary: bool) -> impl IntoView {
    let ui = use_ui();
    let open = RwSignal::new(false);
    let remove = Callback::new(move |()| {
        open.set(false);
        ui.project_dialog.set(Some(ProjectDialog::Remove {
            id: id.clone(),
            name: name.clone(),
        }));
    });
    view! {
        <Show when=move || !primary && !ui.on_phone()>
            <span class="more-wrap" style="position:relative"
                on:click=move |event| event.stop_propagation()>
                <button class="ibtn more" aria-label="Project actions"
                    on:click=move |_| open.update(|shown| *shown = !*shown)>
                    <Icon name="more" size=14 />
                </button>
                <Show when=move || open.get()>
                    <div class="pop" style="top:100%;right:0">
                        <MenuItem icon="trash" label="Remove from the list" danger=true on_pick=remove />
                    </div>
                </Show>
            </span>
        </Show>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{ChatId, ChatRunState};
    use time::OffsetDateTime;

    fn chat(id: &str, title: &str, archived: bool) -> ChatInfo {
        ChatInfo {
            id: ChatId::new(id),
            project: String::new(),
            title: title.to_owned(),
            pinned: false,
            archived,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
            run: ChatRunState::Idle,
            verified: 0,
            total: 0,
            needs_you: false,
        }
    }

    fn project(id: &str, primary: bool, chats: Vec<ChatInfo>) -> ProjectInfo {
        ProjectInfo {
            id: id.to_owned(),
            name: format!("{id}-name"),
            path: format!("/work/{id}"),
            primary,
            live: false,
            chats,
        }
    }

    fn titles(rows: &[RowData]) -> Vec<&str> {
        rows.iter().map(|row| row.title.as_str()).collect()
    }

    #[test]
    fn no_saved_project_makes_the_primary_project_active() {
        let primary = project("main", true, Vec::new());
        let other = project("api", false, Vec::new());
        assert!(is_active(&primary, None));
        assert!(!is_active(&other, None));
    }

    #[test]
    fn the_saved_project_is_the_active_one() {
        let primary = project("main", true, Vec::new());
        let other = project("api", false, Vec::new());
        assert!(!is_active(&primary, Some("api")));
        assert!(is_active(&other, Some("api")));
    }

    #[test]
    fn the_primary_project_saves_no_id() {
        assert_eq!(stored_id("main", true), None);
        assert_eq!(stored_id("api", false), Some("api".to_owned()));
    }

    #[test]
    fn the_active_project_shows_the_snapshot_chats_not_the_listed_ones() {
        let projects = [project("main", true, vec![chat("old", "Old", false)])];
        let live = [chat("new", "Live one", false)];
        let groups = sidebar_groups(&projects, None, &live, "");
        assert_eq!(titles(&groups[0].chats), vec!["Live one"]);
        assert!(groups[0].active);
    }

    #[test]
    fn an_inactive_project_shows_its_listed_chats() {
        let projects = [
            project("main", true, Vec::new()),
            project("api", false, vec![chat("c9", "Fees", false)]),
        ];
        let groups = sidebar_groups(&projects, None, &[], "");
        assert_eq!(titles(&groups[1].chats), vec!["Fees"]);
        assert!(!groups[1].active);
        assert_eq!(groups[1].stored, Some("api".to_owned()));
    }

    #[test]
    fn groups_follow_the_order_of_the_project_list() {
        let projects = [
            project("api", false, Vec::new()),
            project("main", true, Vec::new()),
        ];
        let groups = sidebar_groups(&projects, None, &[], "");
        let ids: Vec<&str> = groups.iter().map(|group| group.id.as_str()).collect();
        assert_eq!(ids, vec!["api", "main"]);
    }

    #[test]
    fn a_search_drops_a_project_with_no_match() {
        let projects = [
            project("main", true, Vec::new()),
            project("api", false, vec![chat("c9", "Fees", false)]),
        ];
        let groups = sidebar_groups(&projects, None, &[], "fee");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].id, "api");
    }

    #[test]
    fn an_empty_project_stays_listed_when_not_searching() {
        let projects = [project("api", false, Vec::new())];
        assert_eq!(sidebar_groups(&projects, None, &[], "").len(), 1);
    }

    #[test]
    fn archived_chats_are_kept_apart_from_live_ones() {
        let projects = [project(
            "api",
            false,
            vec![chat("c1", "Live", false), chat("c2", "Old", true)],
        )];
        let groups = sidebar_groups(&projects, None, &[], "");
        assert_eq!(titles(&groups[0].chats), vec!["Live"]);
        assert_eq!(titles(&groups[0].archived), vec!["Old"]);
    }
}
