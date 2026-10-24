//! Brain logic on a real vault and index: selection, lessons, journal links, and proposals.

use sentinel_brain::journal::{RunSummary, TaskSummary, journal_for};
use sentinel_brain::lesson::{self, Outcome};
use sentinel_brain::mode::may_write;
use sentinel_brain::note::{Author, Note};
use sentinel_brain::proposal::{Applied, Proposal, ProposalState, apply};
use sentinel_brain::schedule::Date;
use sentinel_brain::select::{SelectRequest, Why, select_notes};
use sentinel_brain::{Index, Vault, replace_generated};
use sentinel_core::{BrainMode, LessonState, NoteId, NoteType, ProposalKind, RunId, TaskId};

fn today() -> Date {
    Date::new(2026, 10, 24).expect("valid date")
}

fn note(id: &str, note_type: NoteType, by: Author, body: &str) -> Note {
    Note {
        id: NoteId::new(id),
        note_type,
        title: format!("Title {id}"),
        tags: Vec::new(),
        by,
        strength: 0.5,
        confirmed: None,
        pinned: false,
        lesson: None,
        body: body.to_string(),
        extra: Vec::new(),
    }
}

fn open_vault() -> (tempfile::TempDir, Vault) {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = Vault::open(dir.path().join("brain")).expect("vault opens");
    (dir, vault)
}

/// A vault with a pinned note, a proven lesson, a file note for `src/auth.rs`, and a search hit.
fn seeded() -> (tempfile::TempDir, Vault, Index) {
    let (dir, vault) = open_vault();
    let mut pinned = note(
        "pinned-rule",
        NoteType::Convention,
        Author::You,
        "Always pin versions.",
    );
    pinned.pinned = true;
    vault.write(&pinned, None).expect("write pinned");

    let mut lesson =
        lesson::new_lesson("pattern", "retry-timeout", "raise timeout to 30s", today());
    lesson::record_use(&mut lesson, Outcome::Helped);
    lesson::record_use(&mut lesson, Outcome::Helped);
    vault.write(&lesson, None).expect("write lesson");

    let mut file = note(
        "file-src-auth-rs",
        NoteType::File,
        Author::Run,
        "Touched by auth work.",
    );
    file.extra
        .push(("files".to_string(), "src/auth.rs".to_string()));
    vault.write(&file, None).expect("write file note");

    let hit = note(
        "retry-rules",
        NoteType::Convention,
        Author::Run,
        "Backoff rules for retries.",
    );
    vault.write(&hit, None).expect("write search hit");

    let mut index = Index::open_in_memory().expect("index opens");
    index.rebuild(&vault).expect("index rebuilds");
    (dir, vault, index)
}

#[test]
fn selection_orders_pinned_then_lesson_then_file_then_search() {
    let (_dir, vault, index) = seeded();
    let request = SelectRequest {
        task_files: vec!["src/auth.rs".to_string()],
        query: "retries".to_string(),
        token_cap: 500,
    };
    let selection = select_notes(&index, &vault, &request).expect("select runs");

    let reasons: Vec<Why> = selection.notes.iter().map(|n| n.reason).collect();
    assert_eq!(
        reasons,
        vec![Why::Pinned, Why::Lesson, Why::LinkedFile, Why::SearchHit]
    );
    assert_eq!(selection.skipped, 0);
    assert_eq!(
        selection.summary_line(),
        "Brain loaded 4 notes: 1 pinned, 1 lesson, 1 about these files, 1 search hit."
    );
}

#[test]
fn prompt_text_renders_lessons_as_typed_lines() {
    let (_dir, vault, index) = seeded();
    let request = SelectRequest {
        task_files: Vec::new(),
        query: String::new(),
        token_cap: 500,
    };
    let selection = select_notes(&index, &vault, &request).expect("select runs");
    let prompt = selection.prompt_text(&vault);

    assert!(prompt.starts_with("## Project memory"));
    assert!(prompt.contains("pattern retry-timeout: raise timeout to 30s"));
    assert!(prompt.contains("Always pin versions."));
}

#[test]
fn small_cap_skips_notes_and_stays_under_the_cap() {
    let (_dir, vault, index) = seeded();
    let request = SelectRequest {
        task_files: Vec::new(),
        query: String::new(),
        token_cap: 12,
    };
    let selection = select_notes(&index, &vault, &request).expect("select runs");

    assert!(selection.skipped > 0);
    assert!(selection.tokens <= 12);
    assert!(selection.prompt_text(&vault).len().div_ceil(4) <= 12);
}

#[test]
fn default_cap_is_500_tokens() {
    assert_eq!(SelectRequest::default().token_cap, 500);
}

#[test]
fn a_task_title_seeded_into_a_lesson_is_refused() {
    let titles = vec!["Fix the login race in auth".to_string()];
    let seeded_lesson = lesson::new_lesson("fix", "login", "Fix the login race in auth", today());
    let fields = seeded_lesson.lesson.expect("lesson fields");
    assert!(lesson::check_lesson_text(&fields, &titles).is_err());
}

#[test]
fn candidate_lesson_becomes_proven_after_two_helped_uses() {
    let mut candidate =
        lesson::new_lesson("pattern", "cache-key", "include the repo root", today());
    lesson::record_use(&mut candidate, Outcome::Helped);
    lesson::record_use(&mut candidate, Outcome::Helped);
    let state = candidate.lesson.as_ref().map(|l| l.state);
    assert_eq!(state, Some(LessonState::Proven));
}

#[test]
fn journal_links_a_shared_file_to_both_tasks_after_rebuild() {
    let (_dir, vault) = open_vault();
    let run = RunSummary {
        id: RunId::new("R-9"),
        date: today(),
        verified: 2,
        total: 2,
        task_ids: vec![TaskId::new("T-01"), TaskId::new("T-02")],
    };
    let files = vec!["src/auth.rs".to_string()];
    let tasks: Vec<TaskSummary> = ["T-01", "T-02"]
        .into_iter()
        .map(|id| TaskSummary {
            id: TaskId::new(id),
            title: format!("Task {id}"),
            status_label: "verified".to_string(),
            files: files.clone(),
            run_id: RunId::new("R-9"),
        })
        .collect();

    for journal_note in journal_for(BrainMode::Learn, &run, &tasks) {
        vault
            .write(&journal_note, None)
            .expect("write journal note");
    }
    let mut index = Index::open_in_memory().expect("index opens");
    index.rebuild(&vault).expect("index rebuilds");

    let backlinks = index
        .backlinks(&NoteId::new("file-src-auth-rs"))
        .expect("backlinks");
    assert!(backlinks.contains(&NoteId::new("task-R-9-T-01")));
    assert!(backlinks.contains(&NoteId::new("task-R-9-T-02")));
}

#[test]
fn read_only_mode_writes_no_journal_notes() {
    assert!(!may_write(BrainMode::ReadOnly));
    let run = RunSummary {
        id: RunId::new("R-10"),
        date: today(),
        verified: 0,
        total: 0,
        task_ids: Vec::new(),
    };
    assert_eq!(journal_for(BrainMode::ReadOnly, &run, &[]).len(), 0);
}

#[test]
fn hand_edit_outside_the_markers_survives_an_accepted_update() {
    let (_dir, vault) = open_vault();
    let body = format!(
        "My hand-written line.\n\n{}",
        replace_generated("", "old agent text")
    );
    vault
        .write(
            &note("handmade", NoteType::Convention, Author::Care, &body),
            None,
        )
        .expect("write");

    let proposal = Proposal {
        id: NoteId::new("p-hand"),
        kind: ProposalKind::Update,
        target: Some(NoteId::new("handmade")),
        why: "refresh".to_string(),
        note: Some(note(
            "proposed",
            NoteType::Convention,
            Author::Care,
            &replace_generated("", "new agent text"),
        )),
        state: ProposalState::Accepted,
    };
    let applied = apply(&vault, &proposal, false).expect("apply runs");
    assert_eq!(applied, Applied::Updated(NoteId::new("handmade")));

    let stored = vault
        .read(&NoteId::new("handmade"))
        .expect("read")
        .expect("exists");
    assert!(stored.body.starts_with("My hand-written line."));
    assert!(stored.body.contains("new agent text"));
}

#[test]
fn a_note_you_wrote_is_not_changed_by_an_auto_proposal() {
    let (_dir, vault) = open_vault();
    let mine = note(
        "mine",
        NoteType::Convention,
        Author::You,
        "Keep this exact text.",
    );
    vault.write(&mine, None).expect("write");

    let proposal = Proposal {
        id: NoteId::new("p-auto"),
        kind: ProposalKind::Retire,
        target: Some(NoteId::new("mine")),
        why: "unused".to_string(),
        note: None,
        state: ProposalState::Open,
    };
    assert_eq!(
        apply(&vault, &proposal, true).expect("apply runs"),
        Applied::NeedsAccept
    );
    let stored = vault
        .read(&NoteId::new("mine"))
        .expect("read")
        .expect("exists");
    assert_eq!(stored.body, "Keep this exact text.");
    assert!((stored.strength - 0.5).abs() < 1e-6);
}
