//! The care schedule and the care run. The run calls no agent. It lowers the strength of unused
//! notes, proposes retirements and merges, and in learn mode writes a digest note.

use std::collections::HashSet;
use std::path::Path;

use sentinel_brain::mode::may_write;
use sentinel_brain::note::{Author, Note};
use sentinel_brain::proposal::{Proposal, ProposalState, read_proposals, write_proposal};
use sentinel_brain::schedule::{Date, days_between};
use sentinel_brain::strength;
use sentinel_brain::{generated_block, replace_generated, wikilinks};
use sentinel_core::brain_api::CareSettings;
use sentinel_core::{BrainMode, Cost, Event, NoteId, NoteType, ProposalKind};

use crate::brain::{BrainState, parse_date, require_pc, today, write_note};
use crate::brain_proposals::proposal_created;
use crate::brain_seed::{Project, area_id, area_note};
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
        let found = care_proposals(&state, &notes, &self.repo_path(), today)?;
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
fn care_proposals(
    state: &BrainState,
    notes: &[Note],
    repo: &Path,
    today: Date,
) -> Result<Vec<Proposal>> {
    let existing: HashSet<NoteId> = read_proposals(state.vault.root())?
        .into_iter()
        .map(|proposal| proposal.id)
        .collect();
    let project = Project::read(repo);
    let mut found = retirement_proposals(notes, today);
    found.extend(merge_proposals(notes));
    found.extend(link_proposals(notes, &project.id));
    found.extend(area_proposals(notes, &project));
    found.retain(|proposal| !existing.contains(&proposal.id));
    Ok(found)
}

/// Proposals to link each note that nothing links to or from. Needs the project note to exist.
fn link_proposals(notes: &[Note], project: &NoteId) -> Vec<Proposal> {
    if !notes.iter().any(|note| note.id == *project) {
        return Vec::new();
    }
    let mut linked: HashSet<String> = HashSet::new();
    for note in notes {
        linked.extend(wikilinks(&note.body));
    }
    notes
        .iter()
        .filter(|note| note.id != *project)
        .filter(|note| !linked.contains(note.id.as_str()) && wikilinks(&note.body).is_empty())
        .map(|note| link_proposal(note, project))
        .collect()
}

/// Keeps the generated text the note has and adds one link line. Accepting the proposal replaces
/// only the generated block, so the text outside the markers stays.
fn link_proposal(note: &Note, project: &NoteId) -> Proposal {
    let link = format!("Part of [[{project}]].");
    let text = match generated_block(&note.body).map(str::trim_end) {
        Some(existing) if !existing.is_empty() => format!("{existing}\n\n{link}"),
        _ => link,
    };
    let mut updated = note.clone();
    updated.body = replace_generated(&note.body, &text);
    Proposal {
        id: NoteId::new(format!("care-link-{}", note.id)),
        kind: ProposalKind::Update,
        target: Some(note.id.clone()),
        why: format!("Link {} to the project note", note.title),
        note: Some(updated),
        state: ProposalState::Open,
    }
}

/// Proposals to add a note for each top-level folder that has none.
fn area_proposals(notes: &[Note], project: &Project) -> Vec<Proposal> {
    project
        .areas
        .iter()
        .filter(|folder| !notes.iter().any(|note| note.id == area_id(folder)))
        .map(|folder| Proposal {
            id: NoteId::new(format!("care-{}", area_id(folder))),
            kind: ProposalKind::Add,
            target: None,
            why: format!("Add a note for {folder}"),
            note: Some(area_note(project, folder)),
            state: ProposalState::Open,
        })
        .collect()
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
        why: format!("Merge the notes titled {}", first.title),
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
    use crate::brain_seed::fixtures::{engine_for, sample_repo};
    use sentinel_core::brain_api::{AnswerProposal, ProposalView};

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

    fn loose(id: &str, title: &str) -> Note {
        Note {
            id: NoteId::new(id),
            note_type: NoteType::Convention,
            title: title.to_owned(),
            tags: Vec::new(),
            by: Author::Run,
            strength: strength::INITIAL,
            confirmed: None,
            pinned: false,
            lesson: None,
            body: replace_generated("", "Nothing links here."),
            extra: Vec::new(),
        }
    }

    fn care_then_find(engine: &Engine, why: &str) -> ProposalView {
        engine.run_care(&Caller::pc("test")).unwrap();
        engine
            .brain_proposals()
            .unwrap()
            .into_iter()
            .find(|proposal| proposal.why == why)
            .unwrap_or_else(|| panic!("no proposal says {why:?}"))
    }

    #[test]
    fn an_orphan_note_gets_a_link_proposal_and_accepting_adds_the_link() {
        let (_dir, repo) = sample_repo();
        let engine = engine_for(&repo);
        engine.seed_brain().unwrap();
        let state = engine.brain().unwrap();
        write_note(&state, &loose("loose-note", "Loose note"), None).unwrap();

        let proposal = care_then_find(&engine, "Link Loose note to the project note");
        assert_eq!(proposal.target, Some(NoteId::new("loose-note")));
        engine
            .answer_proposal(&proposal.id, AnswerProposal { accept: true })
            .unwrap();
        let linked = state
            .vault
            .read(&NoteId::new("loose-note"))
            .unwrap()
            .unwrap();
        assert!(
            linked.body.contains("[[project-sample]]"),
            "{}",
            linked.body
        );
    }

    #[test]
    fn a_folder_without_an_area_note_gets_an_add_proposal() {
        let (_dir, repo) = sample_repo();
        let engine = engine_for(&repo);
        engine.seed_brain().unwrap();
        let state = engine.brain().unwrap();
        state.vault.delete(&NoteId::new("area-docs")).unwrap();

        let proposal = care_then_find(&engine, "Add a note for docs");
        assert!(matches!(proposal.kind, ProposalKind::Add));
        engine
            .answer_proposal(&proposal.id, AnswerProposal { accept: true })
            .unwrap();
        assert!(
            state
                .vault
                .read(&NoteId::new("area-docs"))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn two_notes_with_one_title_get_a_merge_proposal() {
        let (_dir, repo) = sample_repo();
        let engine = engine_for(&repo);
        engine.seed_brain().unwrap();
        let state = engine.brain().unwrap();
        write_note(&state, &loose("first-copy", "Same title"), None).unwrap();
        write_note(&state, &loose("second-copy", "Same title"), None).unwrap();

        let proposal = care_then_find(&engine, "Merge the notes titled Same title");
        assert!(matches!(proposal.kind, ProposalKind::Merge));
    }
}
