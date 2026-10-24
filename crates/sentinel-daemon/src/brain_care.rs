//! The care schedule and the care run. The run calls no agent. It lowers the strength of unused
//! notes, proposes retirements and merges, and in learn mode writes a digest note.

use std::collections::HashSet;

use sentinel_brain::mode::may_write;
use sentinel_brain::note::{Author, Note};
use sentinel_brain::proposal::{Proposal, ProposalState, read_proposals, write_proposal};
use sentinel_brain::replace_generated;
use sentinel_brain::schedule::{Date, days_between};
use sentinel_brain::strength;
use sentinel_core::brain_api::CareSettings;
use sentinel_core::{BrainMode, Cost, Event, NoteId, NoteType, ProposalKind};

use crate::brain::{BrainState, parse_date, require_pc, today, write_note};
use crate::brain_proposals::proposal_created;
use crate::controls::Caller;
use crate::engine::{Engine, EngineError, Result};

const CARE_KEY: &str = "brain.care";
/// Days of no use assumed for a note with no recorded use.
const UNUSED_DAYS: u32 = 30;

fn default_care() -> CareSettings {
    CareSettings {
        frequency: sentinel_core::brain_api::CareFrequency::Weekly,
        budget: "0.10".to_owned(),
        auto_apply: false,
        last_run: None,
    }
}

impl Engine {
    /// The care schedule. The defaults hold until a change is saved.
    pub fn care_settings(&self) -> Result<CareSettings> {
        let stored = self.store().get_setting(CARE_KEY)?;
        Ok(stored
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_else(default_care))
    }

    /// Save the care schedule. The last run date is kept as it was. The computer only.
    pub fn set_care_settings(
        &self,
        mut settings: CareSettings,
        caller: &Caller,
    ) -> Result<CareSettings> {
        require_pc(caller)?;
        Cost::parse_usd(&settings.budget).map_err(|e| EngineError::Invalid(e.to_string()))?;
        settings.last_run = self.care_settings()?.last_run;
        self.save_care(&settings)?;
        Ok(settings)
    }

    /// Run care now. Returns the number of proposals written. The computer only.
    pub fn run_care(&self, caller: &Caller) -> Result<usize> {
        require_pc(caller)?;
        let mode = self.config().brain_mode;
        if mode == BrainMode::Off {
            return Err(EngineError::Refused("The Brain is off.".to_owned()));
        }
        let state = self.brain()?;
        let today = today();
        let mut notes = state.vault.list()?;
        if may_write(mode) {
            decay_notes(&state, &mut notes, today)?;
        }
        let found = care_proposals(&state, &notes, today)?;
        for proposal in &found {
            write_proposal(state.vault.root(), proposal)?;
            proposal_created(self, &proposal.id);
        }
        if may_write(mode) {
            write_digest(&state, today, found.len())?;
        }
        self.record_care_run(today)?;
        self.emit_quiet(&Event::CareFinished {
            summary: care_summary(found.len()),
        });
        Ok(found.len())
    }

    fn save_care(&self, settings: &CareSettings) -> Result<()> {
        let text =
            serde_json::to_string(settings).map_err(|e| EngineError::Invalid(e.to_string()))?;
        self.store().set_setting(CARE_KEY, &text)?;
        Ok(())
    }

    fn record_care_run(&self, today: Date) -> Result<()> {
        let mut settings = self.care_settings()?;
        settings.last_run = Some(today.to_string());
        self.save_care(&settings)
    }
}

/// Lower the strength of each note that Sentinel wrote and you did not pin.
fn decay_notes(state: &BrainState, notes: &mut [Note], today: Date) -> Result<()> {
    for note in notes
        .iter_mut()
        .filter(|note| !note.pinned && note.by != Author::You)
    {
        let lowered = strength::decay(note.strength, unused_days(note, today));
        if lowered < note.strength {
            note.strength = lowered;
            write_note(state, note, None)?;
        }
    }
    Ok(())
}

/// Days a note has gone unused. A note with no recorded use counts as `UNUSED_DAYS`.
fn unused_days(note: &Note, today: Date) -> u32 {
    let uses = note.lesson.as_ref().map_or(0, |lesson| lesson.evidence);
    if uses == 0 {
        UNUSED_DAYS
    } else {
        days_since_confirmed(note, today)
    }
}

fn days_since_confirmed(note: &Note, today: Date) -> u32 {
    note.confirmed
        .as_deref()
        .and_then(parse_date)
        .map_or(0, |confirmed| {
            u32::try_from(days_between(confirmed, today)).unwrap_or(0)
        })
}

/// The new proposals of this run. A proposal id that already has a file is left out, so a
/// dismissed proposal does not come back.
fn care_proposals(state: &BrainState, notes: &[Note], today: Date) -> Result<Vec<Proposal>> {
    let existing: HashSet<NoteId> = read_proposals(state.vault.root())?
        .into_iter()
        .map(|proposal| proposal.id)
        .collect();
    let mut found = retirement_proposals(notes, today);
    found.extend(merge_proposals(notes));
    found.retain(|proposal| !existing.contains(&proposal.id));
    Ok(found)
}

fn retirement_proposals(notes: &[Note], today: Date) -> Vec<Proposal> {
    notes
        .iter()
        .filter(|note| !note.pinned)
        .filter(|note| {
            let days = days_since_confirmed(note, today);
            strength::propose_retirement(note.strength, days, note.by)
        })
        .map(|note| Proposal {
            id: NoteId::new(format!("care-retire-{}", note.id)),
            kind: ProposalKind::Retire,
            target: Some(note.id.clone()),
            why: "Weak and unused for a long time.".to_owned(),
            note: None,
            state: ProposalState::Open,
        })
        .collect()
}

fn merge_proposals(notes: &[Note]) -> Vec<Proposal> {
    let mut found = Vec::new();
    for (index, first) in notes.iter().enumerate() {
        for second in &notes[index + 1..] {
            let same = first.note_type == second.note_type
                && first.note_type != NoteType::Lesson
                && first.title.eq_ignore_ascii_case(&second.title);
            if same {
                found.push(merge_proposal(first, second));
            }
        }
    }
    found
}

/// A merge needs your accept when one of the pair is a note you wrote, so that note is the target.
fn merge_proposal(first: &Note, second: &Note) -> Proposal {
    let target = if second.by == Author::You && first.by != Author::You {
        second
    } else {
        first
    };
    let body = format!(
        "Possible duplicates: [[{}]] and [[{}]]. Keep one and remove the other.",
        first.id, second.id
    );
    let merged = Note {
        id: NoteId::new(format!("{}-merged-{}", first.id, second.id)),
        note_type: first.note_type,
        title: first.title.clone(),
        tags: Vec::new(),
        by: Author::Care,
        strength: strength::INITIAL,
        confirmed: None,
        pinned: false,
        lesson: None,
        body: replace_generated("", &body),
        extra: Vec::new(),
    };
    Proposal {
        id: NoteId::new(format!("care-merge-{}-{}", first.id, second.id)),
        kind: ProposalKind::Merge,
        target: Some(target.id.clone()),
        why: "Two notes share a type and a title.".to_owned(),
        note: Some(merged),
        state: ProposalState::Open,
    }
}

/// A digest note of the run. Only learn mode writes notes, so the digest is learn only.
fn write_digest(state: &BrainState, today: Date, proposals: usize) -> Result<()> {
    let text = format!("{proposals} proposals from the care run on {today}.");
    let note = Note {
        id: NoteId::new(format!("digest-{today}")),
        note_type: NoteType::Run,
        title: format!("Care digest {today}"),
        tags: vec!["care".to_owned()],
        by: Author::Care,
        strength: strength::INITIAL,
        confirmed: Some(today.to_string()),
        pinned: false,
        lesson: None,
        body: replace_generated("", &text),
        extra: Vec::new(),
    };
    write_note(state, &note, None)
}

fn care_summary(proposals: usize) -> String {
    let noun = if proposals == 1 {
        "proposal is"
    } else {
        "proposals are"
    };
    format!("{proposals} {noun} waiting. Nothing you wrote was changed.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summary_counts_proposals_in_plain_words() {
        assert_eq!(
            care_summary(1),
            "1 proposal is waiting. Nothing you wrote was changed."
        );
        assert_eq!(
            care_summary(3),
            "3 proposals are waiting. Nothing you wrote was changed."
        );
    }

    #[test]
    fn default_care_runs_weekly_with_no_auto_apply() {
        let settings = default_care();
        assert_eq!(settings.budget, "0.10");
        assert!(!settings.auto_apply);
        assert_eq!(settings.last_run, None);
    }
}
