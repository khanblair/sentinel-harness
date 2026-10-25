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
        agent: None,
    }
}

fn titles(rows: &[ChatRow]) -> Vec<String> {
    rows.iter().map(|row| row.title.clone()).collect()
}

fn snapshot_with(chats: Vec<ChatInfo>) -> Snapshot {
    Snapshot {
        chats,
        ..Snapshot::empty()
    }
}

fn attachment(kind: AttachmentKind, name: &str) -> Attachment {
    Attachment {
        id: AttachmentId::new(format!("a-{name}")),
        chat: ChatId::new("c1"),
        kind,
        name: name.to_owned(),
        path: None,
        text: None,
        size: 10,
        sent: false,
        created_at: OffsetDateTime::UNIX_EPOCH,
    }
}

#[test]
fn the_daemon_order_inside_a_project_is_kept() {
    let mut pinned = chat("c2", "api", "Pinned");
    pinned.pinned = true;
    let chats = vec![pinned, chat("c1", "api", "Newer")];
    let (live, _) = project_rows(&chats, "");
    assert!(live[0].pinned);
    assert_eq!(titles(&live), vec!["Pinned", "Newer"]);
}

#[test]
fn archived_chats_are_split_from_the_live_ones() {
    let mut old = chat("c2", "api", "Old");
    old.archived = true;
    let chats = vec![old, chat("c1", "api", "Fees")];
    let (live, archived) = project_rows(&chats, "");
    assert_eq!(titles(&live), vec!["Fees"]);
    assert_eq!(titles(&archived), vec!["Old"]);
    assert!(archived[0].archived);
}

#[test]
fn an_archived_chat_that_matches_the_query_still_shows() {
    let mut old = chat("c1", "api", "Tax report");
    old.archived = true;
    let (live, archived) = project_rows(&[old], "tax");
    assert_eq!(titles(&live), Vec::<String>::new());
    assert_eq!(titles(&archived), vec!["Tax report"]);
}

#[test]
fn the_query_ignores_case_and_outer_spaces_and_keeps_only_matches() {
    let chats = vec![chat("c1", "api", "Fees"), chat("c2", "api", "Login")];
    let (live, _) = project_rows(&chats, "  FEE ");
    assert_eq!(titles(&live), vec!["Fees"]);
    let (live, archived) = project_rows(&chats, "nothing here");
    assert!(live.is_empty() && archived.is_empty());
}

#[test]
fn a_chat_without_tasks_has_no_count() {
    assert_eq!(count_text(&chat("c1", "api", "Fees")), "");
}

#[test]
fn a_chat_with_tasks_shows_verified_over_total() {
    let mut sample = chat("c1", "api", "Fees");
    sample.verified = 2;
    sample.total = 5;
    assert_eq!(count_text(&sample), "2/5");
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

#[test]
fn a_rename_saves_a_trimmed_new_name() {
    assert_eq!(
        commit_rename("Fees", "  Fee ledger  "),
        Some("Fee ledger".to_owned())
    );
}

#[test]
fn an_empty_or_unchanged_rename_saves_nothing() {
    assert_eq!(commit_rename("Fees", "   "), None);
    assert_eq!(commit_rename("Fees", "Fees"), None);
    assert_eq!(commit_rename("Fees", " Fees "), None);
}

#[test]
fn base64_matches_the_standard_alphabet_with_padding() {
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"fo"), "Zm8=");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    assert_eq!(base64_encode(&[0xfb, 0xff]), "+/8=");
}

#[test]
fn folders_are_the_distinct_parents_sorted_and_capped() {
    let files: Vec<String> = [
        "src/lib.rs",
        "src/fees/rules.rs",
        "src/lib.rs",
        "README.md",
        "tests/a.rs",
    ]
    .iter()
    .map(|path| (*path).to_owned())
    .collect();
    assert_eq!(folders_of(&files), vec!["src", "src/fees", "tests"]);
    let many: Vec<String> = (0..100).map(|n| format!("d{n:03}/f.rs")).collect();
    assert_eq!(folders_of(&many).len(), FOLDER_LIMIT);
}

#[test]
fn the_file_filter_ignores_case_and_keeps_all_on_an_empty_query() {
    let files = vec!["src/Lib.rs".to_owned(), "README.md".to_owned()];
    assert_eq!(filter_files(&files, " lib "), vec!["src/Lib.rs"]);
    assert_eq!(filter_files(&files, ""), files);
}

#[test]
fn a_long_chip_name_is_cut_to_24_characters_with_an_ellipsis() {
    let short = attachment(AttachmentKind::File, "lib.rs");
    assert_eq!(chip_label(&short), "lib.rs");
    let long = attachment(AttachmentKind::File, "abcdefghijklmnopqrstuvwxyz.txt");
    let label = chip_label(&long);
    assert_eq!(label.chars().count(), CHIP_NAME_CHARS);
    assert!(label.ends_with('\u{2026}'));
}

#[test]
fn a_note_chip_shows_its_text() {
    let mut note = attachment(AttachmentKind::Note, "");
    note.text = Some("Use cents".to_owned());
    assert_eq!(chip_label(&note), "Use cents");
}

#[test]
fn each_chip_kind_has_its_icon() {
    assert_eq!(chip_icon(AttachmentKind::File), "file");
    assert_eq!(chip_icon(AttachmentKind::Folder), "folder");
    assert_eq!(chip_icon(AttachmentKind::Backlog), "list");
    assert_eq!(chip_icon(AttachmentKind::Note), "edit");
    assert_eq!(chip_icon(AttachmentKind::Upload), "upload");
}

#[test]
fn an_upload_over_one_megabyte_is_too_large() {
    assert!(!too_large(1_000_000));
    assert!(too_large(1_000_001));
}

#[test]
fn a_single_choice_replaces_only_the_same_kind() {
    let list = vec![
        attachment(AttachmentKind::Backlog, "backlog.md"),
        attachment(AttachmentKind::File, "lib.rs"),
    ];
    assert_eq!(
        same_kind_ids(&list, AttachmentKind::Backlog),
        vec![AttachmentId::new("a-backlog.md")]
    );
}

#[test]
fn the_backlog_picker_lists_only_markdown_files() {
    let files = vec!["backlog.md".to_owned(), "src/lib.rs".to_owned()];
    assert_eq!(PickerKind::Backlog.rows(&files), vec!["backlog.md"]);
}

#[test]
fn the_files_button_counts_its_picks() {
    assert_eq!(PickerKind::Files.confirm_label(0), "Add 0 files");
    assert_eq!(PickerKind::Files.confirm_label(1), "Add 1 file");
    assert_eq!(PickerKind::Files.confirm_label(3), "Add 3 files");
    assert_eq!(PickerKind::Folder.confirm_label(1), "Add folder");
    assert_eq!(PickerKind::Backlog.confirm_label(1), "Use file");
}

#[test]
fn only_files_take_many_choices_and_folders_have_no_device_tab() {
    assert!(!PickerKind::Files.single());
    assert!(PickerKind::Folder.single());
    assert!(PickerKind::Backlog.single());
    assert!(!PickerKind::Folder.has_device_tab());
    assert!(PickerKind::Files.has_device_tab());
}

#[test]
fn a_repository_folder_row_comes_from_the_file_list() {
    let files = vec!["src/lib.rs".to_owned()];
    assert_eq!(PickerKind::Folder.rows(&files), vec!["src"]);
    assert_eq!(PickerKind::Files.rows(&files), files);
}
