use sentinel_brain::{Author, Index, Note, SearchHit, Vault};
use sentinel_core::{NoteId, NoteType};
use tempfile::tempdir;

fn note(id: &str, note_type: NoteType, title: &str, body: &str) -> Note {
    Note {
        id: NoteId::new(id),
        note_type,
        title: title.to_string(),
        tags: vec![],
        by: Author::You,
        strength: 0.5,
        confirmed: None,
        pinned: false,
        lesson: None,
        body: body.to_string(),
        extra: vec![],
    }
}

fn tagged(mut note: Note, tags: &[&str]) -> Note {
    note.tags = tags.iter().copied().map(str::to_string).collect();
    note
}

fn pinned_rule() -> Note {
    let mut pinned = note(
        "pinned-rule",
        NoteType::Convention,
        "Pinned rule",
        "Always pin.",
    );
    pinned.pinned = true;
    pinned
}

fn main_file() -> Note {
    let mut file = tagged(
        note("src-main", NoteType::File, "Main file", "Entry point."),
        &["code"],
    );
    file.extra = vec![("files".to_string(), "src/main.rs, src/lib.rs".to_string())];
    file
}

fn sample_notes() -> Vec<Note> {
    vec![
        tagged(
            note(
                "error-handling",
                NoteType::Convention,
                "Error handling",
                "Use Result, not unwrap. See [[panic-fix]].",
            ),
            &["rust", "errors"],
        ),
        tagged(
            note(
                "panic-fix",
                NoteType::Fix,
                "Panic fix",
                "Fixed a crash. Related: [[missing-note]].",
            ),
            &["errors"],
        ),
        main_file(),
        note(
            "task-42",
            NoteType::Task,
            "Task 42",
            "Touched [[src-main]] and [[error-handling]].",
        ),
        pinned_rule(),
    ]
}

fn hit_ids(hits: &[SearchHit]) -> Vec<String> {
    hits.iter().map(|h| h.id.as_str().to_string()).collect()
}

fn indexed(notes: &[Note]) -> sentinel_brain::Result<Index> {
    let mut index = Index::open_in_memory()?;
    for n in notes {
        index.upsert(n)?;
    }
    Ok(index)
}

#[test]
fn search_finds_by_title_body_and_tag() {
    let index = indexed(&sample_notes()).unwrap();
    let by_title = hit_ids(&index.search("Error", None, 10).unwrap());
    assert!(by_title.contains(&"error-handling".to_string()));
    let by_body = hit_ids(&index.search("unwrap", None, 10).unwrap());
    assert!(by_body.contains(&"error-handling".to_string()));
    let by_tag = hit_ids(&index.search("rust", None, 10).unwrap());
    assert_eq!(by_tag, vec!["error-handling"]);
}

#[test]
fn search_filters_by_type() {
    let index = indexed(&sample_notes()).unwrap();
    let fixes = index.search("errors", Some(NoteType::Fix), 10).unwrap();
    assert_eq!(hit_ids(&fixes), vec!["panic-fix"]);
    assert_eq!(fixes[0].note_type, NoteType::Fix);
}

#[test]
fn search_with_fts_syntax_characters_does_not_error() {
    let index = indexed(&sample_notes()).unwrap();
    let hits = index.search("a:b \"c", None, 10).unwrap();
    assert_eq!(hits, Vec::new());
}

#[test]
fn empty_query_lists_notes_by_strength_then_title() {
    let index = indexed(&sample_notes()).unwrap();
    let all = index.search("   ", None, 10).unwrap();
    assert_eq!(all.len(), 5);
    assert_eq!(all[0].title, "Error handling");
}

#[test]
fn backlinks_and_links_from_follow_wikilinks() {
    let index = indexed(&sample_notes()).unwrap();
    let back = index.backlinks(&NoteId::new("error-handling")).unwrap();
    assert_eq!(back, vec![NoteId::new("task-42")]);
    let links = index.links_from(&NoteId::new("panic-fix")).unwrap();
    assert_eq!(links, vec![NoteId::new("missing-note")]);
}

#[test]
fn graph_counts_nodes_edges_and_dead_links() {
    let index = indexed(&sample_notes()).unwrap();
    let graph = index.graph().unwrap();
    assert_eq!(graph.nodes.len(), 5);
    assert_eq!(graph.edges.len(), 3);
    assert_eq!(
        graph.dead,
        vec![(NoteId::new("panic-fix"), "missing-note".to_string())]
    );
    let error_node = graph
        .nodes
        .iter()
        .find(|n| n.id.as_str() == "error-handling")
        .unwrap();
    assert_eq!(error_node.links_in, 1);
}

#[test]
fn rebuild_gives_the_same_results_as_incremental_upserts() {
    let notes = sample_notes();
    let dir = tempdir().unwrap();
    let vault = Vault::open(dir.path()).unwrap();
    for n in &notes {
        vault.write(n, None).unwrap();
    }
    let mut rebuilt = Index::open_in_memory().unwrap();
    assert_eq!(rebuilt.rebuild(&vault).unwrap(), notes.len());
    let incremental = indexed(&notes).unwrap();

    assert_eq!(
        rebuilt.search("", None, 50).unwrap(),
        incremental.search("", None, 50).unwrap()
    );
    assert_eq!(
        rebuilt.search("errors", None, 50).unwrap(),
        incremental.search("errors", None, 50).unwrap()
    );
    assert_eq!(rebuilt.graph().unwrap(), incremental.graph().unwrap());
}

#[test]
fn remove_drops_the_note_but_keeps_links_to_it_as_dead() {
    let mut index = indexed(&sample_notes()).unwrap();
    index.remove(&NoteId::new("panic-fix")).unwrap();
    assert_eq!(index.search("crash", None, 10).unwrap(), Vec::new());
    let graph = index.graph().unwrap();
    assert_eq!(
        graph.dead,
        vec![(NoteId::new("error-handling"), "panic-fix".to_string())]
    );
    assert_eq!(graph.nodes.len(), 4);
}

#[test]
fn pinned_lists_only_pinned_notes() {
    let index = indexed(&sample_notes()).unwrap();
    assert_eq!(index.pinned().unwrap(), vec![NoteId::new("pinned-rule")]);
}

#[test]
fn notes_linking_to_file_include_the_file_note_and_its_linkers() {
    let index = indexed(&sample_notes()).unwrap();
    let found = index.notes_linking_to_file("src/lib.rs").unwrap();
    assert_eq!(found, vec![NoteId::new("src-main"), NoteId::new("task-42")]);
    assert_eq!(
        index.notes_linking_to_file("src/none.rs").unwrap(),
        Vec::new()
    );
}

#[test]
fn empty_vault_gives_empty_results() {
    let dir = tempdir().unwrap();
    let vault = Vault::open(dir.path()).unwrap();
    let mut index = Index::open_in_memory().unwrap();
    assert_eq!(index.rebuild(&vault).unwrap(), 0);
    assert_eq!(index.search("anything", None, 10).unwrap(), Vec::new());
    assert_eq!(index.graph().unwrap().nodes, Vec::new());
    assert_eq!(index.pinned().unwrap(), Vec::new());
}
