use std::fs;

use sentinel_brain::Vault;
use sentinel_brain::note::{note_type_dir, parse_note};
use sentinel_brain::proposal::read_proposals;
use sentinel_core::{NoteType, ProposalKind};
use serde_json::json;
use tempfile::TempDir;

use super::support::{call_tool, exchange, is_error, tool_text};

const CONVENTION: &str = "---\nid: retry-policy\ntype: convention\ntitle: Retry policy\nby: you\n---\nRetry failed installs twice before asking.\n";

/// A vault folder under a temp repo, opened once so its folders exist.
fn vault_root(repo: &TempDir) -> std::path::PathBuf {
    let root = repo.path().join(".sentinel/brain");
    Vault::open(&root).expect("vault opens");
    root
}

fn brain_env<'a>(root: &'a str, mode: &'a str) -> [(&'a str, &'a str); 2] {
    [("SENTINEL_BRAIN", root), ("SENTINEL_BRAIN_MODE", mode)]
}

#[test]
fn brain_propose_saves_a_proposal_and_no_note() {
    let repo = TempDir::new().expect("temp repo");
    let root = vault_root(&repo);
    let root_text = root.display().to_string();
    let arguments = json!({
        "title": "Use pnpm",
        "body": "This repo uses pnpm.",
        "note_type": "convention",
    });
    let replies = exchange(
        &brain_env(&root_text, "learn"),
        &[call_tool(1, "brain_propose", arguments)],
    );
    assert!(!is_error(&replies[0]), "{}", tool_text(&replies[0]));

    let proposals = read_proposals(&root).expect("proposals read");
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].kind, ProposalKind::Add);
    let vault = Vault::open(&root).expect("vault opens");
    assert_eq!(vault.list().expect("notes list").len(), 0);
    let convention_files = fs::read_dir(root.join(note_type_dir(NoteType::Convention)))
        .expect("convention folder")
        .count();
    assert_eq!(convention_files, 0);
}

#[test]
fn brain_search_finds_a_note_written_with_the_vault() {
    let repo = TempDir::new().expect("temp repo");
    let root = vault_root(&repo);
    let root_text = root.display().to_string();
    let note = parse_note(CONVENTION, "retry-policy.md").expect("note parses");
    Vault::open(&root)
        .expect("vault opens")
        .write(&note, None)
        .expect("note written");

    let replies = exchange(
        &brain_env(&root_text, "read-only"),
        &[
            call_tool(1, "brain_search", json!({"query": "installs"})),
            call_tool(2, "brain_read", json!({"id": "retry-policy"})),
        ],
    );
    assert!(tool_text(&replies[0]).contains("retry-policy"));
    assert!(tool_text(&replies[0]).contains("Retry policy"));
    assert!(tool_text(&replies[1]).contains("Retry failed installs twice"));
    assert!(!tool_text(&replies[1]).contains("type: convention"));
}

#[test]
fn brain_tools_answer_that_the_brain_is_off() {
    let repo = TempDir::new().expect("temp repo");
    let root = vault_root(&repo);
    let root_text = root.display().to_string();
    let replies = exchange(
        &brain_env(&root_text, "off"),
        &[call_tool(1, "brain_search", json!({"query": "installs"}))],
    );
    assert!(is_error(&replies[0]));
    assert_eq!(tool_text(&replies[0]), "The Brain is off.");
}

#[test]
fn brain_propose_is_refused_in_read_only_mode() {
    let repo = TempDir::new().expect("temp repo");
    let root = vault_root(&repo);
    let root_text = root.display().to_string();
    let arguments = json!({"title": "Use pnpm", "body": "x", "note_type": "convention"});
    let replies = exchange(
        &brain_env(&root_text, "read-only"),
        &[call_tool(1, "brain_propose", arguments)],
    );
    assert!(is_error(&replies[0]));
    assert_eq!(read_proposals(&root).expect("proposals read").len(), 0);
}
