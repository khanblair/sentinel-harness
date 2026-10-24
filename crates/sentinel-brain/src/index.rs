//! The search index at `<repo>/.sentinel/brain.db`. It is rebuilt from the vault at any time.

use std::path::Path;

use rusqlite::{Connection, params};
use sentinel_core::NoteId;

use crate::body::wikilinks;
use crate::error::Result;
use crate::note::{Note, lesson_state_name, note_type_dir, note_type_name};
use crate::vault::Vault;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS notes (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    title TEXT NOT NULL,
    path TEXT NOT NULL,
    tags TEXT NOT NULL,
    by TEXT NOT NULL,
    strength REAL NOT NULL,
    confirmed TEXT,
    pinned INTEGER NOT NULL,
    state TEXT,
    files TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS links (
    from_id TEXT NOT NULL,
    to_id TEXT NOT NULL,
    PRIMARY KEY (from_id, to_id)
);
CREATE VIRTUAL TABLE IF NOT EXISTS fts USING fts5(id UNINDEXED, title, body, tags);
";

/// The Brain index: notes, links, and full text search.
pub struct Index {
    pub(crate) conn: Connection,
}

impl Index {
    /// Open or create the index file. Creates the parent folder when missing.
    pub fn open(path: &Path) -> Result<Index> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        Self::with_connection(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Index> {
        Self::with_connection(Connection::open_in_memory()?)
    }

    fn with_connection(conn: Connection) -> Result<Index> {
        conn.execute_batch(SCHEMA)?;
        Ok(Index { conn })
    }

    /// Drop all rows and index every note in the vault. Returns the note count.
    pub fn rebuild(&mut self, vault: &Vault) -> Result<usize> {
        let notes = vault.list()?;
        let tx = self.conn.transaction()?;
        tx.execute_batch("DELETE FROM notes; DELETE FROM links; DELETE FROM fts;")?;
        for note in &notes {
            insert_note(&tx, note)?;
        }
        tx.commit()?;
        Ok(notes.len())
    }

    /// Replace one note's rows.
    pub fn upsert(&mut self, note: &Note) -> Result<()> {
        let tx = self.conn.transaction()?;
        insert_note(&tx, note)?;
        tx.commit()?;
        Ok(())
    }

    /// Remove one note's rows. Links from other notes to it stay, and show as dead links.
    pub fn remove(&mut self, id: &NoteId) -> Result<()> {
        let tx = self.conn.transaction()?;
        delete_rows(&tx, id.as_str())?;
        tx.commit()?;
        Ok(())
    }
}

fn delete_rows(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM notes WHERE id = ?1", params![id])?;
    conn.execute("DELETE FROM links WHERE from_id = ?1", params![id])?;
    conn.execute("DELETE FROM fts WHERE id = ?1", params![id])?;
    Ok(())
}

fn insert_note(conn: &Connection, note: &Note) -> Result<()> {
    let id = note.id.as_str();
    delete_rows(conn, id)?;
    let tags = note.tags.join(", ");
    let state = note.lesson.as_ref().map(|l| lesson_state_name(l.state));
    conn.execute(
        "INSERT INTO notes (id, type, title, path, tags, by, strength, confirmed, pinned, state, files)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            id,
            note_type_name(note.note_type),
            note.title,
            relative_path(note),
            tags,
            note.by.as_str(),
            note.strength,
            note.confirmed,
            note.pinned,
            state,
            files_value(note),
        ],
    )?;
    for target in wikilinks(&note.body) {
        conn.execute(
            "INSERT OR IGNORE INTO links (from_id, to_id) VALUES (?1, ?2)",
            params![id, target],
        )?;
    }
    conn.execute(
        "INSERT INTO fts (id, title, body, tags) VALUES (?1, ?2, ?3, ?4)",
        params![id, note.title, note.body, tags],
    )?;
    Ok(())
}

fn relative_path(note: &Note) -> String {
    format!("{}/{}.md", note_type_dir(note.note_type), note.id.as_str())
}

/// The `files:` extra key, kept as text. Empty when the note has none.
fn files_value(note: &Note) -> String {
    note.extra
        .iter()
        .find(|(key, _)| key == "files")
        .map(|(_, value)| value.clone())
        .unwrap_or_default()
}
