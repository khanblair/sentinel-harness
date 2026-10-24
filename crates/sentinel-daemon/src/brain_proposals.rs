//! Proposals from care runs and their answers. A proposal file in `_proposals/` waits until you
//! accept it, which applies it, or dismiss it. A phone may do both.

use sentinel_brain::note::Author;
use sentinel_brain::proposal::{
    Applied, Proposal, ProposalState, apply, read_proposals, write_proposal,
};
use sentinel_core::brain_api::{AnswerProposal, ProposalView};
use sentinel_core::{Event, NoteId};

use crate::brain::{BrainState, note_written};
use crate::engine::{Engine, EngineError, Result, lock};

impl Engine {
    /// The proposals still waiting for an answer.
    pub fn brain_proposals(&self) -> Result<Vec<ProposalView>> {
        let state = self.brain()?;
        read_proposals(state.vault.root())?
            .iter()
            .filter(|proposal| proposal.state == ProposalState::Open)
            .map(|proposal| proposal_view(&state, proposal))
            .collect()
    }

    /// Accept an open proposal, which applies it, or dismiss it.
    pub fn answer_proposal(&self, id: &str, answer: AnswerProposal) -> Result<()> {
        let state = self.brain()?;
        let mut proposal = read_proposals(state.vault.root())?
            .into_iter()
            .find(|proposal| proposal.id.as_str() == id && proposal.state == ProposalState::Open)
            .ok_or_else(|| EngineError::NotFound(format!("open proposal {id}")))?;
        if answer.accept {
            let applied = apply(&state.vault, &proposal, false)?;
            proposal.state = ProposalState::Accepted;
            write_proposal(state.vault.root(), &proposal)?;
            lock(&state.index).rebuild(&state.vault)?;
            self.note_applied(&applied);
        } else {
            proposal.state = ProposalState::Dismissed;
            write_proposal(state.vault.root(), &proposal)?;
        }
        Ok(())
    }

    fn note_applied(&self, applied: &Applied) {
        if let Applied::Added(id)
        | Applied::Updated(id)
        | Applied::Retired(id)
        | Applied::Merged(id) = applied
        {
            note_written(self, id);
        }
    }
}

fn proposal_view(state: &BrainState, proposal: &Proposal) -> Result<ProposalView> {
    let needs_accept = match &proposal.target {
        Some(target) => state
            .vault
            .read(target)?
            .is_some_and(|note| note.by == Author::You),
        None => false,
    };
    let title = proposal
        .note
        .as_ref()
        .map(|note| note.title.clone())
        .or_else(|| proposal.target.as_ref().map(NoteId::to_string))
        .unwrap_or_default();
    Ok(ProposalView {
        id: proposal.id.to_string(),
        kind: proposal.kind,
        target: proposal.target.clone(),
        title,
        why: proposal.why.clone(),
        needs_accept,
    })
}

/// The event a care run sends for each new proposal.
pub(crate) fn proposal_created(engine: &Engine, id: &NoteId) {
    engine.emit_quiet(&Event::ProposalCreated {
        proposal: id.clone(),
    });
}
