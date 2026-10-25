//! The seed: notes that describe the project on disk. It reads the repository root and runs no
//! agent. It writes a note only when no note with that id exists, so a note a person edited stays.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use sentinel_brain::mode::may_write;
use sentinel_brain::note::{Author, Note};
use sentinel_brain::{replace_generated, strength};
use sentinel_core::api::SeedReport;
use sentinel_core::{NoteId, NoteType, Tier, ToolInfo, ToolStatus};

use crate::brain::{BrainState, slug, write_note};
use crate::config::Config;
use crate::engine::{Engine, EngineError, Result, lock};

const MAX_FILES: usize = 5_000;
const MAX_AREAS: usize = 12;
const MAX_LAYOUT: usize = 40;
const MAX_NOTABLE: usize = 8;
const MAX_EXTENSIONS: usize = 6;
const MAX_SEED_BYTES: u64 = 1_000_000;
const README_LINES: usize = 40;
const CONVENTION_LINES: usize = 60;
const CONVENTION_FILES: [&str; 3] = ["AGENTS.md", "CLAUDE.md", "docs/code-standards.md"];
/// Hidden folders are skipped by their dot, so `.git` and `.sentinel` need no entry here.
const SKIPPED_DIRS: [&str; 4] = ["target", "node_modules", "dist", "build"];

/// The repository as the seed sees it. Paths are relative to the root.
pub(crate) struct Project {
    pub(crate) id: NoteId,
    pub(crate) name: String,
    root: PathBuf,
    pub(crate) files: Vec<PathBuf>,
    /// Top-level folders that get an area note, in name order.
    pub(crate) areas: Vec<String>,
    layout: Vec<String>,
}

impl Project {
    pub(crate) fn read(root: &Path) -> Project {
        let name = root.file_name().map_or_else(
            || "project".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        Project {
            id: project_id(&name),
            root: root.to_path_buf(),
            files: survey(root),
            areas: area_folders(root),
            layout: layout(root),
            name,
        }
    }
}

impl Engine {
    /// Write the notes that describe the project and do not exist yet. Notes that exist are
    /// counted and left as they are. Refused unless the Brain is in learn mode, since a seed writes.
    pub fn seed_brain(&self) -> Result<SeedReport> {
        if !may_write(self.config().brain_mode) {
            return Err(EngineError::Refused(
                "The Brain must be in learn mode to seed it.".to_owned(),
            ));
        }
        let (state, _) = self.open_brain()?;
        let notes = seed_notes(self)?;
        let _writing = lock(&state.seed_lock);
        write_missing(&state, &notes)
    }

    /// Seed on a thread when a freshly opened vault has no notes. The caller does not wait.
    pub(crate) fn seed_when_empty(&self, state: &BrainState) {
        let empty = state.vault.list().is_ok_and(|notes| notes.is_empty());
        if !empty || !may_write(self.config().brain_mode) {
            return;
        }
        let engine = self.clone();
        let started = std::thread::Builder::new()
            .name("brain-seed".to_owned())
            .spawn(move || {
                if let Err(error) = engine.seed_brain() {
                    eprintln!("sentinel: the Brain seed did not finish: {error}");
                }
            });
        if let Err(error) = started {
            eprintln!("sentinel: the Brain seed did not start: {error}");
        }
    }
}

/// Every note the seed would write for this repository.
fn seed_notes(engine: &Engine) -> Result<Vec<Note>> {
    let root = engine.repo_path();
    let config = engine.config();
    let project = Project::read(&root);
    // An empty folder has nothing to describe, so it gets no project notes yet.
    if project.files.is_empty() {
        return Ok(Vec::new());
    }
    let conventions = conventions_note(&root);
    let mut notes: Vec<Note> = project
        .areas
        .iter()
        .map(|folder| area_note(&project, folder))
        .collect();
    notes.push(project_note(&project, &config, conventions.is_some()));
    notes.extend(readme_note(&root));
    notes.extend(conventions);
    notes.push(agents_note(&engine.store().list_tools()?));
    Ok(notes)
}

/// Write each note whose id is not in the vault. The count of notes that exist is kept too.
fn write_missing(state: &BrainState, notes: &[Note]) -> Result<SeedReport> {
    let existing: HashSet<NoteId> = state
        .vault
        .list()?
        .into_iter()
        .map(|note| note.id)
        .collect();
    let mut report = SeedReport {
        created: 0,
        existing: 0,
    };
    for note in notes {
        if existing.contains(&note.id) {
            report.existing += 1;
        } else {
            write_note(state, note, None)?;
            report.created += 1;
        }
    }
    Ok(report)
}

pub(crate) fn area_id(folder: &str) -> NoteId {
    NoteId::new(format!("area-{}", slug(folder)))
}

fn project_id(name: &str) -> NoteId {
    let stem = slug(name);
    if stem.is_empty() {
        NoteId::new("project")
    } else {
        NoteId::new(format!("project-{stem}"))
    }
}

/// The note for one top-level folder: its file count, extension mix, and notable files.
pub(crate) fn area_note(project: &Project, folder: &str) -> Note {
    let prefix = Path::new(folder);
    let inside: Vec<&Path> = project
        .files
        .iter()
        .map(PathBuf::as_path)
        .filter(|file| file.starts_with(prefix))
        .collect();
    let mut lines = vec![
        format!(
            "{} files. Extensions: {}.",
            inside.len(),
            format_counts(&extension_counts(inside.iter().copied()))
        ),
        "Notable files:".to_owned(),
    ];
    lines.extend(
        notable(&inside, prefix)
            .into_iter()
            .map(|path| format!("- {path}")),
    );
    lines.push(format!("Part of [[{}]].", project.id));
    generated_note(area_id(folder), NoteType::File, folder.to_owned(), &lines)
}

fn project_note(project: &Project, config: &Config, convention_present: bool) -> Note {
    let mut lines = vec![
        format!("Project {}.", project.name),
        test_line(&config.test_command),
        format!(
            "Backlog: {} tasks.",
            backlog_tasks(&project.root.join(&config.backlog))
        ),
        "Layout:".to_owned(),
    ];
    lines.extend(project.layout.iter().map(|entry| format!("- {entry}")));
    lines.push(format!(
        "Languages: {}.",
        format_counts(&extension_counts(
            project.files.iter().map(PathBuf::as_path)
        ))
    ));
    lines.push("Areas:".to_owned());
    lines.extend(
        project
            .areas
            .iter()
            .map(|folder| format!("- [[{}]]", area_id(folder))),
    );
    lines.push("Agents: [[agents]].".to_owned());
    if convention_present {
        lines.push("Conventions: [[conventions]].".to_owned());
    }
    generated_note(
        project.id.clone(),
        NoteType::Glossary,
        project.name.clone(),
        &lines,
    )
}

fn readme_note(root: &Path) -> Option<Note> {
    let text = read_seed_text(&root.join("README.md"))?;
    let lines: Vec<String> = text.lines().take(README_LINES).map(str::to_owned).collect();
    Some(generated_note(
        NoteId::new("readme"),
        NoteType::File,
        "README".to_owned(),
        &lines,
    ))
}

fn conventions_note(root: &Path) -> Option<Note> {
    let (source, text) = CONVENTION_FILES
        .iter()
        .find_map(|name| read_seed_text(&root.join(name)).map(|text| (*name, text)))?;
    let mut lines = vec![format!("Source: `{source}`.")];
    lines.extend(text.lines().take(CONVENTION_LINES).map(str::to_owned));
    Some(generated_note(
        NoteId::new("conventions"),
        NoteType::Convention,
        "Conventions".to_owned(),
        &lines,
    ))
}

fn agents_note(tools: &[ToolInfo]) -> Note {
    let mut lines: Vec<String> = tools
        .iter()
        .filter(|tool| tool.enabled && tool.status != ToolStatus::Missing)
        .map(|tool| {
            format!(
                "- {} ({}), tier {}.",
                tool.name,
                tool.id.as_str(),
                tier_name(tool.tier)
            )
        })
        .collect();
    if lines.is_empty() {
        lines.push("No agent CLI is enabled on this computer.".to_owned());
    }
    generated_note(
        NoteId::new("agents"),
        NoteType::Glossary,
        "Agents".to_owned(),
        &lines,
    )
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

fn tier_name(tier: Tier) -> &'static str {
    match tier {
        Tier::Cheap => "cheap",
        Tier::Mid => "mid",
        Tier::Smart => "smart",
    }
}

fn test_line(command: &[String]) -> String {
    if command.is_empty() {
        "No test command is set.".to_owned()
    } else {
        format!("Test command: `{}`.", command.join(" "))
    }
}

/// Lines in the backlog that start a task heading. A missing backlog counts as none.
fn backlog_tasks(path: &Path) -> usize {
    fs::read_to_string(path).map_or(0, |text| {
        text.lines().filter(|line| line.starts_with("## ")).count()
    })
}

/// Text of a seed source file: at most `MAX_SEED_BYTES`, and UTF-8 only.
fn read_seed_text(path: &Path) -> Option<String> {
    let small = fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.len() <= MAX_SEED_BYTES);
    if small {
        fs::read_to_string(path).ok()
    } else {
        None
    }
}

/// Every file under the root, as paths relative to it, sorted. Stops at `MAX_FILES`.
fn survey(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![PathBuf::new()];
    while files.len() < MAX_FILES {
        let Some(dir) = pending.pop() else { break };
        for entry in sorted_entries(&root.join(&dir)) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let relative = dir.join(&name);
            match entry.file_type() {
                Ok(kind) if kind.is_dir() && !SKIPPED_DIRS.contains(&name.as_str()) => {
                    pending.push(relative);
                }
                Ok(kind) if kind.is_file() => files.push(relative),
                _ => {}
            }
        }
    }
    files.sort();
    files.truncate(MAX_FILES);
    files
}

fn sorted_entries(dir: &Path) -> Vec<fs::DirEntry> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<fs::DirEntry> = read.flatten().collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    entries
}

fn is_dir_entry(entry: &fs::DirEntry) -> bool {
    entry.file_type().is_ok_and(|kind| kind.is_dir())
}

/// Top-level folders that get an area note: no hidden or skipped ones, one per slug, at most
/// `MAX_AREAS`.
fn area_folders(root: &Path) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut folders = Vec::new();
    for entry in sorted_entries(root)
        .iter()
        .filter(|entry| is_dir_entry(entry))
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let stem = slug(&name);
        if name.starts_with('.') || SKIPPED_DIRS.contains(&name.as_str()) || stem.is_empty() {
            continue;
        }
        if seen.insert(stem) {
            folders.push(name);
        }
        if folders.len() == MAX_AREAS {
            break;
        }
    }
    folders
}

/// Top-level entries shown in the project note. Folders end with a slash.
fn layout(root: &Path) -> Vec<String> {
    sorted_entries(root)
        .iter()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = is_dir_entry(entry);
            let skipped =
                name.starts_with('.') || (is_dir && SKIPPED_DIRS.contains(&name.as_str()));
            (!skipped).then(|| if is_dir { format!("{name}/") } else { name })
        })
        .take(MAX_LAYOUT)
        .collect()
}

/// Extension counts, most common first. Ties keep alphabetical order. Files without an
/// extension are left out.
fn extension_counts<'a>(files: impl IntoIterator<Item = &'a Path>) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for file in files {
        if let Some(ext) = file.extension().and_then(|ext| ext.to_str()) {
            *counts.entry(ext.to_ascii_lowercase()).or_default() += 1;
        }
    }
    let mut ranked: Vec<(String, usize)> = counts.into_iter().collect();
    ranked.sort_by_key(|(_, count)| Reverse(*count));
    ranked
}

fn format_counts(counts: &[(String, usize)]) -> String {
    if counts.is_empty() {
        return "none".to_owned();
    }
    counts
        .iter()
        .take(MAX_EXTENSIONS)
        .map(|(ext, count)| format!("{ext} ({count})"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Up to `MAX_NOTABLE` files, shallowest first, shown relative to the folder.
fn notable(inside: &[&Path], prefix: &Path) -> Vec<String> {
    let mut sorted = inside.to_vec();
    sorted.sort_by(|a, b| {
        a.components()
            .count()
            .cmp(&b.components().count())
            .then_with(|| a.cmp(b))
    });
    sorted
        .into_iter()
        .take(MAX_NOTABLE)
        .map(|file| {
            file.strip_prefix(prefix)
                .unwrap_or(file)
                .display()
                .to_string()
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod fixtures {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use crate::cmd::SystemRunner;
    use crate::engine::{Engine, EngineOptions};
    use crate::secrets::MemoryStore;

    /// An engine on a temporary repository, with the Brain in its default mode.
    pub(crate) fn engine_for(repo: &Path) -> Engine {
        Engine::open(EngineOptions {
            repo_root: repo.to_path_buf(),
            home: repo.to_path_buf(),
            runner: Arc::new(SystemRunner),
            secrets: Arc::new(MemoryStore::new()),
            fake_agents: None,
            in_memory: true,
        })
        .unwrap()
    }

    /// A small repository named `sample`, with `src/`, `docs/`, and a README.
    pub(crate) fn sample_repo() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("sample");
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::create_dir_all(repo.join("docs")).unwrap();
        fs::write(repo.join("README.md"), "# Sample\n\nA sample project.\n").unwrap();
        fs::write(repo.join("src/lib.rs"), "pub fn run() {}\n").unwrap();
        fs::write(repo.join("docs/guide.md"), "Guide.\n").unwrap();
        (dir, repo)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use sentinel_core::{BrainMode, NoteId};

    use super::fixtures::{engine_for, sample_repo};

    #[test]
    fn seeding_a_project_writes_project_area_and_link_notes() {
        let (_dir, repo) = sample_repo();
        let engine = engine_for(&repo);
        let report = engine.seed_brain().unwrap();
        assert!(report.created >= 4, "{report:?}");

        let state = engine.brain().unwrap();
        for id in [
            "project-sample",
            "area-src",
            "area-docs",
            "readme",
            "agents",
        ] {
            assert!(
                state.vault.read(&NoteId::new(id)).unwrap().is_some(),
                "{id}"
            );
        }
        let graph = engine.brain_graph().unwrap();
        assert!(!graph.links.is_empty(), "the seed links its notes");
    }

    #[test]
    fn seeding_twice_creates_nothing_the_second_time() {
        let (_dir, repo) = sample_repo();
        let engine = engine_for(&repo);
        let first = engine.seed_brain().unwrap();
        let second = engine.seed_brain().unwrap();
        assert_eq!(second.created, 0);
        assert_eq!(second.existing, first.created);
    }

    #[test]
    fn hidden_and_target_folders_get_no_area_note() {
        let (_dir, repo) = sample_repo();
        fs::create_dir_all(repo.join(".hidden")).unwrap();
        fs::write(repo.join(".hidden/a.md"), "hidden").unwrap();
        fs::create_dir_all(repo.join("target/debug")).unwrap();
        fs::write(repo.join("target/debug/out.txt"), "built").unwrap();
        let engine = engine_for(&repo);
        engine.seed_brain().unwrap();

        let state = engine.brain().unwrap();
        for id in ["area-hidden", "area-target"] {
            assert!(
                state.vault.read(&NoteId::new(id)).unwrap().is_none(),
                "{id}"
            );
        }
        assert!(
            state
                .vault
                .read(&NoteId::new("area-src"))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn a_seed_never_overwrites_a_note_that_exists() {
        let (_dir, repo) = sample_repo();
        let engine = engine_for(&repo);
        engine.seed_brain().unwrap();
        let state = engine.brain().unwrap();
        let mut readme = state.vault.read(&NoteId::new("readme")).unwrap().unwrap();
        readme.body = "My own words.".to_owned();
        state.vault.write(&readme, None).unwrap();

        engine.seed_brain().unwrap();
        let kept = state.vault.read(&NoteId::new("readme")).unwrap().unwrap();
        assert_eq!(kept.body, "My own words.");
    }

    #[test]
    fn an_empty_repository_gets_no_seed_notes() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine_for(&dir.path().join("empty"));
        let report = engine.seed_brain().unwrap();
        assert_eq!(report.created, 0);
        assert_eq!(report.existing, 0);
    }

    #[test]
    fn a_read_only_brain_is_not_seeded() {
        let (_dir, repo) = sample_repo();
        let engine = engine_for(&repo);
        engine
            .update_config(|config| config.brain_mode = BrainMode::ReadOnly)
            .unwrap();
        assert!(engine.seed_brain().is_err());
        let notes = engine.brain().unwrap().vault.list().unwrap();
        assert_eq!(notes.len(), 0, "a read-only Brain writes no note");
    }
}
