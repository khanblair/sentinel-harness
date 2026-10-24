//! The phone commands that read or write the repository: `/task` adds a section to `backlog.md`,
//! `/diff` reads the newest checkpoint, and `/daily` counts the day. Each method returns text or a
//! plain error, and the router sends it. None of them runs a shell or touches a file but the backlog.

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use sentinel_core::{BrainMode, Event, EventRecord, TaskId, TaskStatus};
use sentinel_plan::backlog::parse_backlog;
use sentinel_store::CheckpointRecord;
use sentinel_telegram::levels::Detail;
use sentinel_telegram::redact::redact;
use sentinel_tree::Git;
use time::{OffsetDateTime, Time, UtcOffset};

use crate::controls::Caller;
use crate::engine::{Engine, EngineError, Result};

/// The longest task text a chat may add.
pub(crate) const TASK_TEXT_LIMIT: usize = 500;
const BACKLOG_FILE: &str = "backlog.md";
const TITLE_CHARS: usize = 60;
/// Telegram takes 4096 characters. A diff is cut well before that.
const REPLY_CHARS: usize = 3500;
const PATH_LIMIT: usize = 8;
const DIFF_LINES: usize = 6;
const SHORT_SHA_CHARS: usize = 8;
const TITLE_LIMIT: usize = 8;
/// The newest events the daily count reads. The store lists them newest first, without agent output.
const EVENT_WINDOW: u32 = 5000;

/// A task that `/task` added. The id is the one the backlog now holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskAdded {
    pub id: TaskId,
}

impl Engine {
    /// Checks a new task's text and the run state. The router runs it before the confirm button,
    /// and `add_backlog_task` runs it again on confirm.
    pub(crate) fn check_new_task(&self, text: &str) -> Result<()> {
        if self.is_run_live() {
            return Err(EngineError::Refused(
                "A run is live. Add the task after it ends.".to_owned(),
            ));
        }
        let body = text.trim();
        if body.is_empty() {
            return Err(EngineError::Invalid("The task is empty.".to_owned()));
        }
        if body.chars().count() > TASK_TEXT_LIMIT {
            return Err(EngineError::Invalid(format!(
                "A task can be at most {TASK_TEXT_LIMIT} characters."
            )));
        }
        Ok(())
    }

    /// Appends one task section to `backlog.md`. Every existing byte stays as it was.
    pub(crate) fn add_backlog_task(&self, text: &str, caller: &Caller) -> Result<TaskAdded> {
        self.check_new_task(text)?;
        let body = text.trim();
        let path = self.repo_root().join(BACKLOG_FILE);
        let existing = read_backlog(&path)?;
        let id = next_task_id(&existing)?;
        append(&path, &section_text(&existing, &id, body))?;
        self.emit(&Event::DeviceAction {
            device: sentinel_core::DeviceId::new(&caller.name),
            action: format!("added {id} to the backlog"),
        })?;
        Ok(TaskAdded { id })
    }

    /// The short text for `/diff`: the named task's newest checkpoint, or the newest of all.
    pub(crate) fn diff_summary(&self, task: Option<&TaskId>, detail: Detail) -> Result<String> {
        if let Some(id) = task
            && self.store().get_task(id)?.is_none()
        {
            return Ok(format!("I do not know task {id}. Send /status."));
        }
        let Some(record) = self.checkpoint_for(task)? else {
            return Ok("Nothing is checkpointed yet.".to_owned());
        };
        let lines = self.diff_lines(&record, detail)?;
        Ok(redact(&lines.join("\n"))
            .chars()
            .take(REPLY_CHARS)
            .collect())
    }

    /// The `/daily` text: tasks verified today and in total, today's spend, the run state, and the
    /// open Brain proposals. Titles stay out when hackathon mode is on.
    pub(crate) fn daily_summary(&self) -> Result<String> {
        let midnight = self
            .now()
            .to_offset(UtcOffset::UTC)
            .replace_time(Time::MIDNIGHT);
        let tasks = self.store().list_tasks()?;
        let verified_today = self.verified_since(midnight)?;
        let verified_total = tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Verified)
            .count();
        let spend = self.store().cost_since(midnight)?;
        let state = format!("{:?}", self.harness_state()).to_lowercase();
        let mut lines = vec![
            format!(
                "Verified today: {}. In total: {verified_total}.",
                verified_today.len()
            ),
            format!("Spend today: {spend}."),
            format!("Sentinel is {state}."),
        ];
        if let Some(count) = self.open_proposal_count() {
            lines.push(format!("Brain proposals waiting: {count}."));
        }
        if !self.config().hackathon_mode {
            lines.extend(
                tasks
                    .iter()
                    .filter(|task| verified_today.contains(&task.id))
                    .take(TITLE_LIMIT)
                    .map(|task| format!("{} {}", task.id, task.title)),
            );
        }
        Ok(lines.join("\n"))
    }

    fn checkpoint_for(&self, task: Option<&TaskId>) -> Result<Option<CheckpointRecord>> {
        let record = match task {
            Some(id) => self.store().latest_checkpoint(id)?,
            None => self.store().list_checkpoints()?.pop(),
        };
        Ok(record)
    }

    fn diff_lines(&self, record: &CheckpointRecord, detail: Detail) -> Result<Vec<String>> {
        let mut lines = vec![record.task.to_string()];
        if detail == Detail::Status {
            lines.push(files_changed(record.paths.len()));
        } else {
            lines.push(format!("Commit {}", short_sha(&record.commit)));
            lines.push(self.stat_summary(&record.commit)?);
        }
        lines.extend(path_lines(&record.paths));
        if detail == Detail::Detail {
            lines.extend(self.diff_head(&record.commit)?);
        }
        Ok(lines)
    }

    /// The last line of `git show --stat`, which holds the file and line counts. The commit
    /// message is left out: a task title never reaches a chat.
    fn stat_summary(&self, commit: &str) -> Result<String> {
        let stat = self.git_read(&["show", "--stat", "--format=", commit])?;
        Ok(stat.lines().last().unwrap_or_default().trim().to_owned())
    }

    fn diff_head(&self, commit: &str) -> Result<Vec<String>> {
        let diff = self.git_read(&["show", "--unified=0", "--format=", commit])?;
        Ok(diff
            .lines()
            .filter(|line| is_changed_line(line))
            .take(DIFF_LINES)
            .map(str::to_owned)
            .collect())
    }

    /// Runs one read-only git command through the wrapper, with the run's private index.
    fn git_read(&self, args: &[&str]) -> Result<String> {
        let repo = self.repo_root().to_path_buf();
        let git = Git::new(&repo, repo.join(".sentinel/index"))
            .map_err(|error| EngineError::Invalid(error.to_string()))?;
        git.run(args)
            .map_err(|error| EngineError::Invalid(error.to_string()))
    }

    /// Verdict events that say `Verified` at or after `since`, by task.
    fn verified_since(&self, since: OffsetDateTime) -> Result<BTreeSet<TaskId>> {
        let records = self.store().recent_stream_events(EVENT_WINDOW)?;
        Ok(records
            .iter()
            .filter(|record| record.at >= since)
            .filter_map(verified_task)
            .collect())
    }

    /// The open proposals, or `None` when the Brain is off. A Brain that cannot list them says
    /// nothing either.
    fn open_proposal_count(&self) -> Option<usize> {
        if self.config().brain_mode == BrainMode::Off {
            return None;
        }
        self.brain_proposals().ok().map(|proposals| proposals.len())
    }
}

fn verified_task(record: &EventRecord) -> Option<TaskId> {
    let Event::Verdict { task, status, .. } = &record.event else {
        return None;
    };
    (*status == TaskStatus::Verified).then(|| task.clone())
}

fn read_backlog(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error.into()),
    }
}

fn append(path: &Path, section: &str) -> Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(section.as_bytes())?;
    Ok(())
}

/// One past the highest `T-NN` the backlog holds. Unnamed headings count too, because the run
/// gives them ids in the same way.
fn next_task_id(backlog: &str) -> Result<TaskId> {
    let highest = highest_task_number(backlog)?;
    Ok(TaskId::new(format!("T-{:02}", highest + 1)))
}

fn highest_task_number(backlog: &str) -> Result<u32> {
    if !backlog
        .lines()
        .any(|line| line.trim_start().starts_with("##"))
    {
        return Ok(0);
    }
    let items = parse_backlog(backlog)
        .map_err(|error| EngineError::Invalid(format!("backlog.md cannot be read: {error}")))?;
    Ok(items
        .iter()
        .filter_map(|item| task_number(&item.id))
        .max()
        .unwrap_or(0))
}

fn task_number(id: &TaskId) -> Option<u32> {
    id.as_str().strip_prefix("T-")?.parse().ok()
}

/// The new section. It starts with the blank line the file needs, and it names the task by its
/// first line, cut to 60 characters. The body is the whole text.
fn section_text(existing: &str, id: &TaskId, body: &str) -> String {
    let first_line = body.lines().next().unwrap_or_default().trim();
    let title: String = first_line.chars().take(TITLE_CHARS).collect();
    let separator = match existing {
        "" => "",
        text if text.ends_with("\n\n") => "",
        text if text.ends_with('\n') => "\n",
        _ => "\n\n",
    };
    format!("{separator}## {id} {title}\n\n{body}\n")
}

fn files_changed(count: usize) -> String {
    let noun = if count == 1 { "file" } else { "files" };
    format!("{count} {noun} changed.")
}

fn path_lines(paths: &[String]) -> Vec<String> {
    let mut lines: Vec<String> = paths.iter().take(PATH_LIMIT).cloned().collect();
    if paths.len() > PATH_LIMIT {
        lines.push(format!("and {} more", paths.len() - PATH_LIMIT));
    }
    lines
}

fn short_sha(commit: &str) -> &str {
    commit.get(..SHORT_SHA_CHARS).unwrap_or(commit)
}

/// A added or removed line of a diff. The `---` and `+++` file headers are not changes.
fn is_changed_line(line: &str) -> bool {
    (line.starts_with('+') && !line.starts_with("+++"))
        || (line.starts_with('-') && !line.starts_with("---"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::SystemRunner;
    use crate::engine::EngineOptions;
    use crate::secrets::MemoryStore;
    use std::sync::Arc;

    fn engine_in(dir: &Path) -> Engine {
        Engine::open(EngineOptions {
            repo_root: dir.to_path_buf(),
            home: dir.to_path_buf(),
            runner: Arc::new(SystemRunner),
            secrets: Arc::new(MemoryStore::new()),
            fake_agents: None,
            in_memory: true,
        })
        .unwrap()
    }

    #[test]
    fn the_first_task_of_a_new_backlog_is_t_01() {
        assert_eq!(next_task_id("").unwrap().as_str(), "T-01");
    }

    #[test]
    fn the_next_task_follows_the_highest_named_id() {
        let backlog = "## T-04 Rate limit\nBody.\n\n## T-02 Logging\nBody.\n";
        assert_eq!(next_task_id(backlog).unwrap().as_str(), "T-05");
    }

    #[test]
    fn an_unnamed_heading_takes_a_number_the_run_would_give_it() {
        let backlog = "## Rate limit\nBody.\n";
        assert_eq!(next_task_id(backlog).unwrap().as_str(), "T-02");
    }

    #[test]
    fn a_section_after_text_without_a_final_newline_gets_a_blank_line_first() {
        let id = TaskId::new("T-02");
        let section = section_text("## T-01 Old\nBody.", &id, "Add logging");
        assert_eq!(section, "\n\n## T-02 Add logging\n\nAdd logging\n");
    }

    #[test]
    fn the_title_is_the_first_line_cut_to_sixty_characters() {
        let id = TaskId::new("T-01");
        let long = "a".repeat(80);
        let section = section_text("", &id, &format!("{long}\nSecond line"));
        let heading = section.lines().next().unwrap_or_default();
        assert_eq!(heading, format!("## T-01 {}", "a".repeat(60)));
        assert!(section.ends_with("Second line\n"), "{section}");
    }

    #[test]
    fn a_task_of_five_hundred_characters_passes_and_five_hundred_and_one_fails() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine_in(dir.path());
        assert!(engine.check_new_task(&"x".repeat(500)).is_ok());
        assert!(engine.check_new_task(&"x".repeat(501)).is_err());
    }

    #[test]
    fn a_live_run_refuses_a_new_task() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine_in(dir.path());
        engine.set_state(sentinel_core::HarnessState::Running);
        let refusal = engine.check_new_task("Add logging").unwrap_err();
        assert_eq!(
            refusal.to_string(),
            "A run is live. Add the task after it ends."
        );
    }

    #[test]
    fn only_added_and_removed_lines_count_as_changes() {
        assert!(is_changed_line("+let x = 1;"));
        assert!(is_changed_line("-let y = 2;"));
        assert!(!is_changed_line("+++ b/src/lib.rs"));
        assert!(!is_changed_line("--- a/src/lib.rs"));
        assert!(!is_changed_line("@@ -1 +1 @@"));
    }
}
