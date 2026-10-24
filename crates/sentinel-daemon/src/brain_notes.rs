//! Reading the Brain: the notes list, one note with its links, and the graph.

use std::collections::{HashMap, HashSet};

use sentinel_brain::note::{LessonFields, Note};
use sentinel_brain::wikilinks;
use sentinel_core::brain_api::{
    GraphNodeView, GraphView, LessonView, NoteDetail, NoteLink, NoteList, NoteSummary,
};
use sentinel_core::{NoteId, NoteType};

use crate::brain::{BrainState, note_text, version_of};
use crate::engine::{Engine, EngineError, Result, lock};

/// The most search hits the list takes from the index.
const SEARCH_LIMIT: u32 = 500;

impl Engine {
    /// Every note, or the notes that match `query`. Pinned notes come first, then the strongest.
    pub fn brain_notes(
        &self,
        query: Option<&str>,
        note_type: Option<NoteType>,
    ) -> Result<NoteList> {
        let state = self.brain()?;
        let mut notes = state.vault.list()?;
        if let Some(text) = query.map(str::trim).filter(|text| !text.is_empty()) {
            let hits = lock(&state.index).search(text, note_type, SEARCH_LIMIT)?;
            let found: HashSet<NoteId> = hits.into_iter().map(|hit| hit.id).collect();
            notes.retain(|note| found.contains(&note.id));
        }
        notes.retain(|note| note_type.is_none_or(|wanted| note.note_type == wanted));
        let mut summaries: Vec<NoteSummary> = notes.iter().map(summary_of).collect();
        summaries.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then(b.strength.total_cmp(&a.strength))
                .then(a.title.cmp(&b.title))
        });
        Ok(NoteList {
            notes: summaries,
            vault_path: state.vault.root().to_string_lossy().into_owned(),
            mode: self.config().brain_mode,
        })
    }

    /// One note with its text, links, backlinks, and version.
    pub fn brain_note(&self, id: &NoteId) -> Result<NoteDetail> {
        let state = self.brain()?;
        let note = find_note(&state, id)?;
        note_detail(&state, &note)
    }

    /// Every note as a node, and every link between two notes as an edge.
    pub fn brain_graph(&self) -> Result<GraphView> {
        let state = self.brain()?;
        let graph = lock(&state.index).graph()?;
        Ok(GraphView {
            nodes: graph
                .nodes
                .into_iter()
                .map(|node| GraphNodeView {
                    id: node.id,
                    title: node.title,
                    note_type: node.note_type,
                    backlinks: node.links_in,
                })
                .collect(),
            links: graph.edges,
        })
    }
}

pub(crate) fn find_note(state: &BrainState, id: &NoteId) -> Result<Note> {
    state
        .vault
        .read(id)?
        .ok_or_else(|| EngineError::NotFound(format!("note {id}")))
}

pub(crate) fn note_detail(state: &BrainState, note: &Note) -> Result<NoteDetail> {
    let titles: HashMap<NoteId, String> = state
        .vault
        .list()?
        .into_iter()
        .map(|other| (other.id, other.title))
        .collect();
    let links = wikilinks(&note.body)
        .into_iter()
        .map(|target| note_link(&titles, NoteId::new(target)))
        .collect();
    let backlinks = lock(&state.index)
        .backlinks(&note.id)?
        .into_iter()
        .map(|id| note_link(&titles, id))
        .collect();
    let text = note_text(&state.vault, note);
    Ok(NoteDetail {
        summary: summary_of(note),
        body: note.body.clone(),
        confirmed: note.confirmed.clone(),
        uses: note.lesson.as_ref().map_or(0, |lesson| lesson.evidence),
        links,
        backlinks,
        lesson: note.lesson.as_ref().map(lesson_view),
        version: version_of(&text),
    })
}

pub(crate) fn summary_of(note: &Note) -> NoteSummary {
    NoteSummary {
        id: note.id.clone(),
        note_type: note.note_type,
        title: note.title.clone(),
        tags: note.tags.clone(),
        by: note.by.as_str().to_owned(),
        strength: note.strength,
        pinned: note.pinned,
        lesson_state: note.lesson.as_ref().map(|lesson| lesson.state),
    }
}

/// A link is dead when no note has that id.
fn note_link(titles: &HashMap<NoteId, String>, id: NoteId) -> NoteLink {
    NoteLink {
        dead: !titles.contains_key(&id),
        title: titles.get(&id).cloned().unwrap_or_else(|| id.to_string()),
        id,
    }
}

fn lesson_view(lesson: &LessonFields) -> LessonView {
    LessonView {
        kind: lesson.kind.clone(),
        key: lesson.key.clone(),
        value: lesson.value.clone(),
        evidence: lesson.evidence,
        helped: lesson.helped,
        hurt: lesson.hurt,
    }
}
