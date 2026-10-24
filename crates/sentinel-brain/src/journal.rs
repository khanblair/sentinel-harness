//! Run journal: run, task, and file notes. Pure builders. The daemon writes the notes.

use std::collections::BTreeMap;

use sentinel_core::{BrainMode, NoteId, NoteType, RunId, TaskId};

use crate::body::replace_generated;
use crate::mode::may_write;
use crate::note::{Author, Note};
use crate::schedule::Date;
use crate::strength;

/// One finished run, as the journal needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSummary {
    pub id: RunId,
    pub date: Date,
    pub verified: u32,
    pub total: u32,
    pub task_ids: Vec<TaskId>,
}

/// One task in a run, as the journal needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSummary {
    pub id: TaskId,
    pub title: String,
    pub status_label: String,
    pub files: Vec<String>,
    pub run_id: RunId,
}

pub fn run_note_id(run: &RunId) -> NoteId {
    NoteId::new(format!("run-{}", run.as_str()))
}

pub fn task_note_id(run: &RunId, task: &TaskId) -> NoteId {
    NoteId::new(format!("task-{}-{}", run.as_str(), task.as_str()))
}

pub fn file_note_id(path: &str) -> NoteId {
    NoteId::new(format!("file-{}", path.replace(['/', '.'], "-")))
}

/// The run note. It links to each of its task notes.
pub fn run_note(run: &RunSummary) -> Note {
    let mut lines = vec![format!("Verified {} of {} tasks.", run.verified, run.total)];
    lines.extend(
        run.task_ids
            .iter()
            .map(|task| format!("- [[{}]]", task_note_id(&run.id, task).as_str())),
    );
    let mut note = generated_note(
        run_note_id(&run.id),
        NoteType::Run,
        format!("Run {}", run.id.as_str()),
        &lines,
    );
    note.confirmed = Some(run.date.to_string());
    note
}

/// The task note. It links to its run and to each file it touched.
pub fn task_note(task: &TaskSummary) -> Note {
    let mut lines = vec![
        format!("Status: {}.", task.status_label),
        format!("Run: [[{}]]", run_note_id(&task.run_id).as_str()),
        "Files:".to_string(),
    ];
    lines.extend(
        task.files
            .iter()
            .map(|path| format!("- [[{}]]", file_note_id(path).as_str())),
    );
    generated_note(
        task_note_id(&task.run_id, &task.id),
        NoteType::Task,
        task.title.clone(),
        &lines,
    )
}

/// The file note for `path`. It links to the task notes that touched it, so backlinks show them.
/// Its `files:` key lets `notes_linking_to_file` find it.
pub fn file_note(path: &str, tasks_touching: &[String]) -> Note {
    let mut lines = vec!["Touched by:".to_string()];
    lines.extend(tasks_touching.iter().map(|task| format!("- [[{task}]]")));
    let mut note = generated_note(file_note_id(path), NoteType::File, path.to_string(), &lines);
    note.extra.push(("files".to_string(), path.to_string()));
    note
}

/// Every journal note for one run. Empty unless the mode is `learn`.
pub fn journal_for(mode: BrainMode, run: &RunSummary, tasks: &[TaskSummary]) -> Vec<Note> {
    if !may_write(mode) {
        return Vec::new();
    }
    let mut notes = vec![run_note(run)];
    notes.extend(tasks.iter().map(task_note));
    notes.extend(file_notes(tasks));
    notes
}

fn file_notes(tasks: &[TaskSummary]) -> Vec<Note> {
    let mut touching: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for task in tasks {
        let task_id = task_note_id(&task.run_id, &task.id).as_str().to_string();
        for path in &task.files {
            let ids = touching.entry(path.as_str()).or_default();
            if !ids.contains(&task_id) {
                ids.push(task_id.clone());
            }
        }
    }
    touching
        .into_iter()
        .map(|(path, ids)| file_note(path, &ids))
        .collect()
}

/// A generated note. Its text sits inside the sentinel markers, so user text can sit beside it.
fn generated_note(id: NoteId, note_type: NoteType, title: String, lines: &[String]) -> Note {
    Note {
        id,
        note_type,
        title,
        tags: Vec::new(),
        by: Author::Run,
        strength: strength::INITIAL,
        confirmed: None,
        pinned: false,
        lesson: None,
        body: replace_generated("", &lines.join("\n")),
        extra: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::wikilinks;

    fn run_id() -> RunId {
        RunId::new("R-7")
    }

    fn summary() -> RunSummary {
        RunSummary {
            id: run_id(),
            date: Date::new(2026, 10, 24).expect("valid date"),
            verified: 1,
            total: 2,
            task_ids: vec![TaskId::new("T-01"), TaskId::new("T-02")],
        }
    }

    fn tasks() -> Vec<TaskSummary> {
        vec![
            TaskSummary {
                id: TaskId::new("T-01"),
                title: "Add login".to_string(),
                status_label: "verified".to_string(),
                files: vec!["src/auth.rs".to_string()],
                run_id: run_id(),
            },
            TaskSummary {
                id: TaskId::new("T-02"),
                title: "Fix login".to_string(),
                status_label: "failed".to_string(),
                files: vec!["src/auth.rs".to_string()],
                run_id: run_id(),
            },
        ]
    }

    #[test]
    fn file_note_id_turns_slashes_and_dots_into_dashes() {
        assert_eq!(file_note_id("src/auth.rs").as_str(), "file-src-auth-rs");
    }

    #[test]
    fn run_note_links_its_task_notes() {
        let note = run_note(&summary());
        assert_eq!(note.id.as_str(), "run-R-7");
        assert_eq!(
            wikilinks(&note.body),
            vec!["task-R-7-T-01", "task-R-7-T-02"]
        );
    }

    #[test]
    fn task_note_links_its_run_and_its_files() {
        let note = task_note(&tasks()[0]);
        assert_eq!(note.id.as_str(), "task-R-7-T-01");
        assert_eq!(wikilinks(&note.body), vec!["run-R-7", "file-src-auth-rs"]);
    }

    #[test]
    fn journal_links_a_shared_file_to_both_tasks() {
        let notes = journal_for(BrainMode::Learn, &summary(), &tasks());
        let file = notes
            .iter()
            .find(|note| note.id.as_str() == "file-src-auth-rs")
            .expect("file note written");
        assert_eq!(
            wikilinks(&file.body),
            vec!["task-R-7-T-01", "task-R-7-T-02"]
        );
        assert_eq!(
            file.extra,
            vec![("files".to_string(), "src/auth.rs".to_string())]
        );
    }

    #[test]
    fn journal_writes_run_task_and_file_notes_in_learn_mode() {
        let notes = journal_for(BrainMode::Learn, &summary(), &tasks());
        assert_eq!(notes.len(), 1 + 2 + 1);
        assert!(notes.iter().all(|note| note.by == Author::Run));
    }

    #[test]
    fn journal_returns_nothing_in_read_only_or_off() {
        assert_eq!(
            journal_for(BrainMode::ReadOnly, &summary(), &tasks()).len(),
            0
        );
        assert_eq!(journal_for(BrainMode::Off, &summary(), &tasks()).len(), 0);
    }
}
