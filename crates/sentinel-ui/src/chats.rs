//! Pure logic of the chat list: grouping by project, row icons, counts, and which chat is open.
//! Nothing here touches the DOM or a signal, so each rule has a unit test.

use sentinel_core::chat::NEW_CHAT_TITLE;
use sentinel_core::{ChatId, ChatInfo, ChatRunState, Snapshot};

/// The group that holds archived chats. It comes last and starts collapsed.
pub const ARCHIVED_GROUP: &str = "Archived";

/// Chats of one project, or the archived group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectGroup {
    pub project: String,
    pub chats: Vec<ChatRow>,
}

/// One chat row of the sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRow {
    pub id: ChatId,
    pub title: String,
    pub pinned: bool,
    pub archived: bool,
    pub icon: RowIcon,
    pub count: String,
    pub needs_you: bool,
}

/// The status icon at the end of a chat row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowIcon {
    Spinner,
    Paused,
    Done,
    Blocked,
    Planned,
    Stopped,
    None,
}

/// The icon for a chat's run state. An idle chat has no icon.
pub fn row_icon(state: ChatRunState) -> RowIcon {
    match state {
        ChatRunState::Idle => RowIcon::None,
        ChatRunState::Planned => RowIcon::Planned,
        ChatRunState::Running => RowIcon::Spinner,
        ChatRunState::Paused => RowIcon::Paused,
        ChatRunState::Done => RowIcon::Done,
        ChatRunState::Blocked => RowIcon::Blocked,
        ChatRunState::Stopped => RowIcon::Stopped,
    }
}

/// `verified/total` once the chat has tasks, else nothing.
pub fn count_text(info: &ChatInfo) -> String {
    if info.total > 0 {
        format!("{}/{}", info.verified, info.total)
    } else {
        String::new()
    }
}

fn row_from(info: &ChatInfo) -> ChatRow {
    ChatRow {
        id: info.id.clone(),
        title: info.title.clone(),
        pinned: info.pinned,
        archived: info.archived,
        icon: row_icon(info.run),
        count: count_text(info),
        needs_you: info.needs_you,
    }
}

/// A title matches when the search text (already trimmed and lower case) is inside it.
/// An empty search matches every title.
fn matches_query(title: &str, needle: &str) -> bool {
    needle.is_empty() || title.to_lowercase().contains(needle)
}

/// Chats grouped by project, in the order each project first appears. Archived chats go to one
/// `Archived` group at the end. The query filters by title, ignoring case and outer spaces.
/// Groups with no matching chat are left out.
pub fn group_chats(chats: &[ChatInfo], query: &str) -> Vec<ProjectGroup> {
    let needle = query.trim().to_lowercase();
    let mut groups: Vec<ProjectGroup> = Vec::new();
    let mut archived: Vec<ChatRow> = Vec::new();
    for info in chats
        .iter()
        .filter(|info| matches_query(&info.title, &needle))
    {
        if info.archived {
            archived.push(row_from(info));
        } else {
            push_into_project(&mut groups, &info.project, row_from(info));
        }
    }
    if !archived.is_empty() {
        groups.push(ProjectGroup {
            project: ARCHIVED_GROUP.to_owned(),
            chats: archived,
        });
    }
    groups
}

fn push_into_project(groups: &mut Vec<ProjectGroup>, project: &str, row: ChatRow) {
    match groups.iter_mut().find(|group| group.project == project) {
        Some(group) => group.chats.push(row),
        None => groups.push(ProjectGroup {
            project: project.to_owned(),
            chats: vec![row],
        }),
    }
}

fn has_chat(snapshot: &Snapshot, id: &ChatId) -> bool {
    snapshot.chats.iter().any(|chat| chat.id == *id)
}

/// The chat the stream opens on. An open chat that still exists stays open, archived or not.
/// Otherwise the chat of the live run, then the first chat that is not archived.
pub fn pick_open_chat(snapshot: &Snapshot, open: Option<&ChatId>) -> Option<ChatId> {
    open.filter(|id| has_chat(snapshot, id))
        .cloned()
        .or_else(|| snapshot.run.as_ref().map(|run| run.chat.clone()))
        .or_else(|| {
            snapshot
                .chats
                .iter()
                .find(|chat| !chat.archived)
                .map(|chat| chat.id.clone())
        })
}

/// The title of a chat. A chat the snapshot does not hold, or no chat, shows "New chat".
pub fn chat_title(snapshot: &Snapshot, id: Option<&ChatId>) -> String {
    id.and_then(|id| snapshot.chats.iter().find(|chat| chat.id == *id))
        .map_or_else(|| NEW_CHAT_TITLE.to_owned(), |chat| chat.title.clone())
}

/// The toast shown when a message is sent while another chat holds the live run.
pub fn busy_notice(snapshot: &Snapshot, live: Option<&ChatId>) -> String {
    format!(
        "Another chat is working in this project: {}. Open it or stop it.",
        chat_title(snapshot, live)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::RunId;
    use sentinel_core::RunInfo;
    use time::OffsetDateTime;

    fn chat(id: &str, project: &str, title: &str) -> ChatInfo {
        ChatInfo {
            id: ChatId::new(id),
            project: project.to_owned(),
            title: title.to_owned(),
            pinned: false,
            archived: false,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
            run: ChatRunState::Idle,
            verified: 0,
            total: 0,
            needs_you: false,
        }
    }

    fn ids(groups: &[ProjectGroup]) -> Vec<(String, Vec<String>)> {
        groups
            .iter()
            .map(|group| {
                (
                    group.project.clone(),
                    group
                        .chats
                        .iter()
                        .map(|row| row.id.as_str().to_owned())
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn projects_keep_the_order_they_first_appear_in() {
        let chats = vec![
            chat("c1", "api", "Fees"),
            chat("c2", "web", "Login"),
            chat("c3", "api", "Tax"),
        ];
        assert_eq!(
            ids(&group_chats(&chats, "")),
            vec![
                ("api".to_owned(), vec!["c1".to_owned(), "c3".to_owned()]),
                ("web".to_owned(), vec!["c2".to_owned()]),
            ]
        );
    }

    #[test]
    fn the_daemon_order_inside_a_project_is_kept() {
        let mut pinned = chat("c2", "api", "Pinned");
        pinned.pinned = true;
        let chats = vec![pinned, chat("c1", "api", "Newer")];
        let groups = group_chats(&chats, "");
        assert!(groups[0].chats[0].pinned);
        assert_eq!(groups[0].chats[1].title, "Newer");
    }

    #[test]
    fn archived_chats_only_appear_in_the_last_archived_group() {
        let mut old = chat("c2", "api", "Old");
        old.archived = true;
        let chats = vec![old, chat("c1", "api", "Fees")];
        let groups = group_chats(&chats, "");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].project, "api");
        assert_eq!(groups[0].chats.len(), 1);
        assert_eq!(groups[1].project, ARCHIVED_GROUP);
        assert!(groups[1].chats[0].archived);
    }

    #[test]
    fn an_archived_chat_that_matches_the_query_still_shows() {
        let mut old = chat("c1", "api", "Tax report");
        old.archived = true;
        let groups = group_chats(&[old], "tax");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].project, ARCHIVED_GROUP);
    }

    #[test]
    fn the_query_ignores_case_and_outer_spaces_and_drops_empty_groups() {
        let chats = vec![chat("c1", "api", "Fees"), chat("c2", "web", "Login")];
        assert_eq!(
            ids(&group_chats(&chats, "  FEE ")),
            vec![("api".to_owned(), vec!["c1".to_owned()])]
        );
        assert_eq!(
            group_chats(&chats, "nothing here"),
            Vec::<ProjectGroup>::new()
        );
    }

    #[test]
    fn a_chat_without_tasks_has_no_count() {
        assert_eq!(count_text(&chat("c1", "api", "Fees")), "");
    }

    #[test]
    fn a_chat_with_tasks_shows_verified_over_total() {
        let mut info = chat("c1", "api", "Fees");
        info.verified = 2;
        info.total = 5;
        assert_eq!(count_text(&info), "2/5");
    }

    #[test]
    fn every_run_state_maps_to_its_row_icon() {
        assert_eq!(row_icon(ChatRunState::Idle), RowIcon::None);
        assert_eq!(row_icon(ChatRunState::Planned), RowIcon::Planned);
        assert_eq!(row_icon(ChatRunState::Running), RowIcon::Spinner);
        assert_eq!(row_icon(ChatRunState::Paused), RowIcon::Paused);
        assert_eq!(row_icon(ChatRunState::Done), RowIcon::Done);
        assert_eq!(row_icon(ChatRunState::Blocked), RowIcon::Blocked);
        assert_eq!(row_icon(ChatRunState::Stopped), RowIcon::Stopped);
    }

    fn snapshot_with(chats: Vec<ChatInfo>) -> Snapshot {
        Snapshot {
            chats,
            ..Snapshot::empty()
        }
    }

    #[test]
    fn an_open_chat_stays_open_even_when_archived() {
        let mut old = chat("c1", "api", "Old");
        old.archived = true;
        let snapshot = snapshot_with(vec![chat("c2", "api", "New"), old]);
        assert_eq!(
            pick_open_chat(&snapshot, Some(&ChatId::new("c1"))),
            Some(ChatId::new("c1"))
        );
    }

    #[test]
    fn a_deleted_open_chat_gives_way_to_the_live_run_chat() {
        let mut snapshot = snapshot_with(vec![chat("c2", "api", "New")]);
        snapshot.run = Some(RunInfo {
            id: RunId::new("r1"),
            chat: ChatId::new("c3"),
            backlog: "backlog.md".to_owned(),
            wave: 1,
            started_at: OffsetDateTime::UNIX_EPOCH,
        });
        assert_eq!(
            pick_open_chat(&snapshot, Some(&ChatId::new("gone"))),
            Some(ChatId::new("c3"))
        );
    }

    #[test]
    fn without_a_run_the_first_unarchived_chat_opens() {
        let mut old = chat("c1", "api", "Old");
        old.archived = true;
        let snapshot = snapshot_with(vec![old, chat("c2", "api", "New")]);
        assert_eq!(pick_open_chat(&snapshot, None), Some(ChatId::new("c2")));
    }

    #[test]
    fn no_chats_means_no_open_chat() {
        assert_eq!(pick_open_chat(&Snapshot::empty(), None), None);
    }

    #[test]
    fn the_title_falls_back_to_new_chat() {
        let snapshot = snapshot_with(vec![chat("c1", "api", "Fees")]);
        assert_eq!(chat_title(&snapshot, Some(&ChatId::new("c1"))), "Fees");
        assert_eq!(chat_title(&snapshot, Some(&ChatId::new("c9"))), "New chat");
        assert_eq!(chat_title(&snapshot, None), "New chat");
    }

    #[test]
    fn the_busy_notice_names_the_live_chat() {
        let snapshot = snapshot_with(vec![chat("c1", "api", "Fees")]);
        assert_eq!(
            busy_notice(&snapshot, Some(&ChatId::new("c1"))),
            "Another chat is working in this project: Fees. Open it or stop it."
        );
    }
}
