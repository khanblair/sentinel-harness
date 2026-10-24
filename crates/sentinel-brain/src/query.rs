//! Read side of the index: search, backlinks, the link graph, and pinned notes.

use std::collections::BTreeSet;

use rusqlite::types::Type;
use rusqlite::{Connection, Params, Row, params};
use sentinel_core::{NoteId, NoteType};

use crate::error::Result;
use crate::index::Index;
use crate::note::{note_type_name, parse_note_type};

/// One search result.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub id: NoteId,
    pub title: String,
    pub note_type: NoteType,
    pub snippet: String,
    pub strength: f32,
}

/// One note in the graph.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphNode {
    pub id: NoteId,
    pub title: String,
    pub note_type: NoteType,
    /// Links that point at this note.
    pub links_in: u32,
}

/// The link graph. `dead` lists links whose target note does not exist.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<(NoteId, NoteId)>,
    pub dead: Vec<(NoteId, String)>,
}

const SEARCH_TERMS: &str =
    "SELECT n.id, n.title, n.type, snippet(fts, 2, '[', ']', '...', 12), n.strength
     FROM fts JOIN notes n ON n.id = fts.id
     WHERE fts MATCH ?1 AND (?2 IS NULL OR n.type = ?2)
     ORDER BY bm25(fts), n.strength DESC, n.title, n.id
     LIMIT ?3";

const SEARCH_ALL: &str = "SELECT n.id, n.title, n.type, substr(f.body, 1, 160), n.strength
     FROM notes n JOIN fts f ON f.id = n.id
     WHERE (?1 IS NULL OR n.type = ?1)
     ORDER BY n.strength DESC, n.title, n.id
     LIMIT ?2";

impl Index {
    /// Full text search over title, body, and tags. An empty query lists notes by strength.
    /// Query text is quoted term by term, so FTS syntax characters never cause an error.
    pub fn search(
        &self,
        query: &str,
        note_type: Option<NoteType>,
        limit: u32,
    ) -> Result<Vec<SearchHit>> {
        let type_name = note_type.map(note_type_name);
        let limit = i64::from(limit);
        let terms = quote_terms(query);
        if terms.is_empty() {
            return query_list(
                &self.conn,
                SEARCH_ALL,
                params![type_name, limit],
                hit_from_row,
            );
        }
        query_list(
            &self.conn,
            SEARCH_TERMS,
            params![terms, type_name, limit],
            hit_from_row,
        )
    }

    /// Notes that link to `id`, sorted.
    pub fn backlinks(&self, id: &NoteId) -> Result<Vec<NoteId>> {
        query_list(
            &self.conn,
            "SELECT l.from_id FROM links l JOIN notes n ON n.id = l.from_id
             WHERE l.to_id = ?1 ORDER BY l.from_id",
            params![id.as_str()],
            note_id_from_row,
        )
    }

    /// Link targets written in note `id`, sorted. Targets may not exist.
    pub fn links_from(&self, id: &NoteId) -> Result<Vec<NoteId>> {
        query_list(
            &self.conn,
            "SELECT to_id FROM links WHERE from_id = ?1 ORDER BY to_id",
            params![id.as_str()],
            note_id_from_row,
        )
    }

    /// Every note as a node, every link between two existing notes as an edge,
    /// and every link to a missing note as a dead link.
    pub fn graph(&self) -> Result<Graph> {
        let nodes = query_list(
            &self.conn,
            "SELECT n.id, n.title, n.type, COUNT(l.from_id)
             FROM notes n LEFT JOIN links l ON l.to_id = n.id
             GROUP BY n.id ORDER BY n.id",
            [],
            node_from_row,
        )?;
        let edges = query_list(
            &self.conn,
            "SELECT l.from_id, l.to_id FROM links l
             JOIN notes a ON a.id = l.from_id JOIN notes b ON b.id = l.to_id
             ORDER BY l.from_id, l.to_id",
            [],
            edge_from_row,
        )?;
        let dead = query_list(
            &self.conn,
            "SELECT from_id, to_id FROM links
             WHERE to_id NOT IN (SELECT id FROM notes) ORDER BY from_id, to_id",
            [],
            dead_from_row,
        )?;
        Ok(Graph { nodes, edges, dead })
    }

    /// Ids of pinned notes, sorted.
    pub fn pinned(&self) -> Result<Vec<NoteId>> {
        query_list(
            &self.conn,
            "SELECT id FROM notes WHERE pinned = 1 ORDER BY id",
            [],
            note_id_from_row,
        )
    }

    /// `file` notes whose `files:` list holds `path`, plus every note that links to one of them.
    pub fn notes_linking_to_file(&self, path: &str) -> Result<Vec<NoteId>> {
        let file_notes = self.file_notes_for(path)?;
        let mut found: BTreeSet<NoteId> = file_notes.iter().cloned().collect();
        for file_note in &file_notes {
            found.extend(self.backlinks(file_note)?);
        }
        Ok(found.into_iter().collect())
    }

    fn file_notes_for(&self, path: &str) -> Result<Vec<NoteId>> {
        let rows: Vec<(String, String)> = query_list(
            &self.conn,
            "SELECT id, files FROM notes WHERE type = 'file' AND files <> ''",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(rows
            .into_iter()
            .filter(|(_, files)| files.split(',').any(|f| f.trim() == path))
            .map(|(id, _)| NoteId::new(id))
            .collect())
    }
}

/// Quote each whitespace-separated term as an FTS phrase, joined with spaces (AND).
fn quote_terms(query: &str) -> String {
    query
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn query_list<T, P: Params>(
    conn: &Connection,
    sql: &str,
    params: P,
    map: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params, map)?;
    Ok(rows.collect::<rusqlite::Result<Vec<T>>>()?)
}

fn note_type_column(row: &Row<'_>, index: usize) -> rusqlite::Result<NoteType> {
    let raw: String = row.get(index)?;
    parse_note_type(&raw).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            Type::Text,
            format!("unknown type {raw}").into(),
        )
    })
}

fn hit_from_row(row: &Row<'_>) -> rusqlite::Result<SearchHit> {
    Ok(SearchHit {
        id: NoteId::new(row.get::<_, String>(0)?),
        title: row.get(1)?,
        note_type: note_type_column(row, 2)?,
        snippet: row.get(3)?,
        strength: row.get(4)?,
    })
}

fn note_id_from_row(row: &Row<'_>) -> rusqlite::Result<NoteId> {
    Ok(NoteId::new(row.get::<_, String>(0)?))
}

fn node_from_row(row: &Row<'_>) -> rusqlite::Result<GraphNode> {
    Ok(GraphNode {
        id: NoteId::new(row.get::<_, String>(0)?),
        title: row.get(1)?,
        note_type: note_type_column(row, 2)?,
        links_in: row.get(3)?,
    })
}

fn edge_from_row(row: &Row<'_>) -> rusqlite::Result<(NoteId, NoteId)> {
    Ok((
        NoteId::new(row.get::<_, String>(0)?),
        NoteId::new(row.get::<_, String>(1)?),
    ))
}

fn dead_from_row(row: &Row<'_>) -> rusqlite::Result<(NoteId, String)> {
    Ok((NoteId::new(row.get::<_, String>(0)?), row.get(1)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_terms_escapes_double_quotes() {
        assert_eq!(quote_terms(r#"a:b "c"#), r#""a:b" """c""#);
        assert_eq!(quote_terms("   "), "");
    }
}
