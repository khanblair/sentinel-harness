//! The Brain during a run: the notes an agent reads before a task, and the journal a run writes
//! when it ends. `read-only` reads and never writes. `learn` also writes. `off` does neither.

use std::collections::BTreeMap;

use sentinel_brain::journal::{RunSummary, TaskSummary, file_note, file_note_id, journal_for};
use sentinel_brain::mode::may_read;
use sentinel_brain::select::{SelectRequest, select_notes};
use sentinel_brain::{Note, wikilinks};
use sentinel_core::{Event, RunId, Task};

use crate::brain::{BrainState, note_written, today, write_note};
use crate::engine::{Engine, Result};

/// Search text for a task: its title words. The notes it finds are read, never copied into a lesson.
fn query_for(task: &Task) -> String {
    task.title.clone()
}

impl Engine {
    /// The `## Project memory` text for one task, or an empty string when the Brain is off or has
    /// nothing for it. The stream says how many notes were loaded.
    pub(crate) fn brain_memory(&self, task: &Task) -> String {
        if !may_read(self.config().brain_mode) {
            return String::new();
        }
        match self.select_memory(task) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("sentinel: the Brain memory was skipped: {error}");
                String::new()
            }
        }
    }

    fn select_memory(&self, task: &Task) -> Result<String> {
        let state = self.brain()?;
        let request = SelectRequest {
            task_files: task.scope.entries().map(str::to_owned).collect(),
            query: query_for(task),
            ..SelectRequest::default()
        };
        let selection = {
            let index = crate::engine::lock(&state.index);
            select_notes(&index, &state.vault, &request)?
        };
        if selection.notes.is_empty() {
            return Ok(String::new());
        }
        self.emit_quiet(&Event::Message {
            from: "sentinel".to_owned(),
            to: None,
            text: selection.summary_line(),
        });
        Ok(selection.prompt_text(&state.vault))
    }

    /// Write the run, task, and file notes. Only in `learn` mode. A failure is logged and never
    /// changes how the run ended.
    pub(crate) fn write_run_journal(&self, run: &RunId, verified: u32, total: u32) {
        let mode = self.config().brain_mode;
        let Ok(tasks) = self.store().list_tasks() else {
            return;
        };
        let summary = RunSummary {
            id: run.clone(),
            date: today(),
            verified,
            total,
            task_ids: tasks.iter().map(|t| t.id.clone()).collect(),
        };
        let touched = self.touched_files();
        let summaries: Vec<TaskSummary> = tasks
            .iter()
            .map(|task| TaskSummary {
                id: task.id.clone(),
                title: task.title.clone(),
                status_label: task.status.label().to_owned(),
                files: touched.get(task.id.as_str()).cloned().unwrap_or_default(),
                run_id: run.clone(),
            })
            .collect();
        let notes = journal_for(mode, &summary, &summaries);
        if notes.is_empty() {
            return;
        }
        if let Err(error) = self.save_journal(&notes) {
            eprintln!("sentinel: the run journal was not written: {error}");
        }
    }

    /// The files each task checkpointed, from the checkpoint rows.
    fn touched_files(&self) -> BTreeMap<String, Vec<String>> {
        let mut by_task: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for record in self.store().list_checkpoints().unwrap_or_default() {
            let files = by_task.entry(record.task.to_string()).or_default();
            for path in record.paths {
                if !files.contains(&path) {
                    files.push(path);
                }
            }
        }
        by_task
    }

    fn save_journal(&self, notes: &[Note]) -> Result<()> {
        let state = self.brain()?;
        for note in notes {
            let note = merge_file_links(&state, note);
            write_note(&state, &note, None)?;
            note_written(self, &note.id);
        }
        Ok(())
    }
}

/// A file note from an earlier run keeps the task links it had, so backlinks grow across runs.
fn merge_file_links(state: &BrainState, note: &Note) -> Note {
    let Ok(Some(old)) = state.vault.read(&note.id) else {
        return note.clone();
    };
    let Some(path) = file_path_of(note) else {
        return note.clone();
    };
    let mut tasks: Vec<String> = wikilinks(&old.body)
        .into_iter()
        .filter(|id| id.starts_with("task-"))
        .collect();
    for id in wikilinks(&note.body) {
        if !tasks.contains(&id) {
            tasks.push(id);
        }
    }
    file_note(&path, &tasks)
}

fn file_path_of(note: &Note) -> Option<String> {
    note.extra
        .iter()
        .find(|(key, _)| key == "files")
        .map(|(_, value)| value.clone())
        .filter(|path| file_note_id(path) == note.id)
}

#[cfg(test)]
mod tests {
    use sentinel_core::TaskId;

    use super::*;

    #[test]
    fn the_search_text_is_the_task_title() {
        let task = Task::new(TaskId::new("T-01"), "Reject negative amounts", "body");
        assert_eq!(query_for(&task), "Reject negative amounts");
    }
}
