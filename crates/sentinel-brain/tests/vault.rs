use sentinel_brain::{
    Author, BrainError, LessonFields, NOTE_TYPES, Note, Vault, generated_block, parse_note,
    render_note, replace_generated, wikilinks,
};
use sentinel_core::{LessonState, NoteId, NoteType};
use tempfile::tempdir;

fn convention(id: &str, body: &str) -> Note {
    Note {
        id: NoteId::new(id),
        note_type: NoteType::Convention,
        title: "Error handling".to_string(),
        tags: vec!["rust".to_string(), "errors".to_string()],
        by: Author::Run,
        strength: 0.62,
        confirmed: Some("2026-10-24".to_string()),
        pinned: false,
        lesson: None,
        body: body.to_string(),
        extra: vec![],
    }
}

fn lesson(id: &str) -> Note {
    Note {
        id: NoteId::new(id),
        note_type: NoteType::Lesson,
        title: "Use Result".to_string(),
        tags: vec!["rust".to_string()],
        by: Author::Care,
        strength: 0.8,
        confirmed: None,
        pinned: true,
        lesson: Some(LessonFields {
            kind: "pattern".to_string(),
            key: "error-handling".to_string(),
            value: "use Result: not unwrap".to_string(),
            evidence: 5,
            helped: 3,
            hurt: 0,
            state: LessonState::Proven,
        }),
        body: String::new(),
        extra: vec![("files".to_string(), "src/a.rs, src/b.rs".to_string())],
    }
}

#[test]
fn round_trip_covers_every_note_type_including_lesson() {
    let dir = tempdir().unwrap();
    let vault = Vault::open(dir.path()).unwrap();
    for (index, t) in NOTE_TYPES.into_iter().enumerate() {
        let mut note = convention(&format!("note-{index}"), "Body line.\n\nSecond line.");
        note.note_type = t;
        if t == NoteType::Lesson {
            note = lesson("note-lesson");
        }
        vault.write(&note, None).unwrap();
        assert_eq!(vault.read(&note.id).unwrap(), Some(note));
    }
}

#[test]
fn render_then_parse_gives_the_same_note() {
    let note = lesson("lesson-round");
    let parsed = parse_note(&render_note(&note), "lesson-round.md").unwrap();
    assert_eq!(parsed, note);
}

#[test]
fn hand_edit_keeps_extra_keys_and_user_text_byte_for_byte() {
    let dir = tempdir().unwrap();
    let vault = Vault::open(dir.path()).unwrap();
    let mut note = convention(
        "edit-me",
        "Top user text.\n<!-- sentinel:start -->\nold generated\n<!-- sentinel:end -->\nMore user text.",
    );
    note.extra = vec![("source".to_string(), "obsidian: edited".to_string())];
    vault.write(&note, None).unwrap();
    let path = vault.path_for(&note);

    let hand_edited = std::fs::read_to_string(&path)
        .unwrap()
        .replace("Top user text.", "Top user text, edited in Obsidian.");
    std::fs::write(&path, &hand_edited).unwrap();

    let read = vault.read(&note.id).unwrap().unwrap();
    assert_eq!(read.extra, note.extra);
    let expected_hash = Vault::content_hash(&hand_edited);
    let mut changed = read.clone();
    changed.body = replace_generated(&read.body, "new generated");
    vault.write(&changed, Some(expected_hash)).unwrap();

    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        on_disk,
        hand_edited.replace("old generated", "new generated")
    );
}

#[test]
fn replace_generated_leaves_user_text_untouched() {
    let body = "User A.\n<!-- sentinel:start -->\nold\n<!-- sentinel:end -->\nUser B.";
    let replaced = replace_generated(body, "new");
    assert!(replaced.starts_with("User A.\n<!-- sentinel:start -->\n"));
    assert!(replaced.ends_with("\n<!-- sentinel:end -->\nUser B."));
    assert_eq!(generated_block(&replaced).map(str::trim), Some("new"));
}

#[test]
fn parse_rejects_missing_front_matter_bad_type_bad_strength_and_bad_lesson() {
    let no_front = parse_note("just text", "a.md");
    assert!(matches!(no_front, Err(BrainError::Format { .. })));

    let bad_type = "---\nid: a\ntype: nonsense\ntitle: A\n---\n";
    assert!(matches!(
        parse_note(bad_type, "a.md"),
        Err(BrainError::Format { .. })
    ));

    let bad_strength = "---\nid: a\ntype: convention\ntitle: A\nstrength: 1.5\n---\n";
    let error = parse_note(bad_strength, "a.md").unwrap_err();
    assert!(error.to_string().contains("a.md"));

    let lesson_missing = "---\nid: l\ntype: lesson\ntitle: L\n---\n";
    assert!(matches!(
        parse_note(lesson_missing, "l.md"),
        Err(BrainError::Format { .. })
    ));

    let lesson_text = "---\nid: l\ntype: lesson\ntitle: L\nkind: k\nkey: k\nvalue: v\nevidence: 0\nhelped: 0\nhurt: 0\nstate: candidate\n---\nFree text.\n";
    assert!(matches!(
        parse_note(lesson_text, "l.md"),
        Err(BrainError::Format { .. })
    ));
}

#[test]
fn wikilinks_follow_the_link_rules() {
    let body = "See [[alpha|Alpha]] and [[beta]].\n```\n[[in-code]]\n```\nAgain [[alpha]].";
    assert_eq!(wikilinks(body), vec!["alpha", "beta"]);
}

#[test]
fn atomic_write_leaves_no_temp_file() {
    let dir = tempdir().unwrap();
    let vault = Vault::open(dir.path()).unwrap();
    let note = convention("atomic", "Body.");
    vault.write(&note, None).unwrap();

    let names: Vec<String> = std::fs::read_dir(dir.path().join("conventions"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["atomic.md".to_string()]);
}

#[test]
fn stale_hash_gives_conflict_and_keeps_the_file() {
    let dir = tempdir().unwrap();
    let vault = Vault::open(dir.path()).unwrap();
    let note = convention("stale", "Original.");
    let hash = vault.write(&note, None).unwrap();

    let path = vault.path_for(&note);
    std::fs::write(&path, "changed outside").unwrap();

    let mut newer = note.clone();
    newer.body = "Sentinel text.".to_string();
    let result = vault.write(&newer, Some(hash));
    assert!(matches!(result, Err(BrainError::Conflict(_))));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "changed outside");
}

#[test]
fn read_finds_a_note_in_the_wrong_folder() {
    let dir = tempdir().unwrap();
    let vault = Vault::open(dir.path()).unwrap();
    let note = convention("misfiled", "Body.");
    std::fs::write(
        dir.path().join("decisions").join("misfiled.md"),
        render_note(&note),
    )
    .unwrap();

    let read = vault.read(&note.id).unwrap();
    assert_eq!(read, Some(note));
}

#[test]
fn list_skips_a_broken_file_and_list_errors_names_it() {
    let dir = tempdir().unwrap();
    let vault = Vault::open(dir.path()).unwrap();
    vault.write(&convention("good", "Body."), None).unwrap();
    let broken = vault.root().join("conventions").join("broken.md");
    std::fs::write(&broken, "no front matter").unwrap();

    let notes = vault.list().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, NoteId::new("good"));

    let errors = vault.list_errors().unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].0, broken);
}

#[test]
fn content_hash_of_empty_text_is_the_fnv_offset() {
    assert_eq!(Vault::content_hash(""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(Vault::content_hash("abc"), Vault::content_hash("abc"));
}

#[test]
fn delete_removes_a_note_once() {
    let dir = tempdir().unwrap();
    let vault = Vault::open(dir.path()).unwrap();
    let note = convention("gone", "Body.");
    vault.write(&note, None).unwrap();
    assert!(vault.delete(&note.id).unwrap());
    assert!(!vault.delete(&note.id).unwrap());
    assert_eq!(vault.read(&note.id).unwrap(), None);
}
