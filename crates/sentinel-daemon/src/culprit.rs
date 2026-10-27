//! A failed barrier blames only the tasks whose own change breaks the suite. Each checkpoint of the
//! wave is tested alone on the tree the wave started from. The tasks that fail alone go back, and
//! the rest are verified on a fresh suite run.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use sentinel_core::{Event, TaskId};
use sentinel_tree::{Checkpoint, Git};
use sentinel_verify::gate::{SuiteConfig, run_suite};

use crate::engine::{Result, lock};
use crate::proof::{ProofResult, rebuild_tree, write_bytes};
use crate::run_ctx::RunCtx;
use crate::verify_phase::{send_back_by_id, verify_wave};
use crate::wave::run_the_suite;

const CULPRIT_TIMEOUT: Duration = Duration::from_secs(120);

/// How to run the suite for one probe.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SuiteRun<'a> {
    pub command: &'a [String],
    pub timeout: Duration,
    pub log_dir: &'a Path,
}

/// The inputs that every task probe of one barrier shares.
struct Probe<'a> {
    git: &'a Git,
    base: &'a str,
    run: SuiteRun<'a>,
}

/// After a failed barrier with more than one checkpoint, send back only the tasks that break the
/// suite on their own, then verify the rest. Returns false when no task or every task is guilty:
/// the caller then keeps the old behavior.
pub(crate) async fn settle_failed_barrier(
    ctx: &Arc<RunCtx>,
    number: u32,
    checkpointed: &[TaskId],
) -> Result<bool> {
    let guilty = find_culprits(ctx, checkpointed).await;
    if guilty.is_empty() || guilty.len() == checkpointed.len() {
        return Ok(false);
    }
    let rest: Vec<TaskId> = checkpointed
        .iter()
        .filter(|id| !guilty.contains(*id))
        .cloned()
        .collect();
    let names = id_list(&guilty);
    say(
        ctx,
        format!(
            "The suite fails with only the change of {names} on top of the wave start, so only {names} goes back."
        ),
    )?;
    for id in &guilty {
        send_back_by_id(ctx, id, "its change fails the test suite on its own.")?;
    }
    let again = run_the_suite(ctx, number).await?;
    let verdict = if again.passed { "passed" } else { "failed" };
    say(
        ctx,
        format!("barrier {number} again without {names}: suite {verdict}"),
    )?;
    verify_wave(ctx, &again, &rest).await?;
    Ok(true)
}

/// The tasks of a wave that break the suite on the wave start, each tested alone.
pub(crate) async fn find_culprits(ctx: &RunCtx, tasks: &[TaskId]) -> Vec<TaskId> {
    let Some(base) = lock(&ctx.memory).wave_base.clone() else {
        return Vec::new();
    };
    let pairs = checkpoints_of(ctx, tasks);
    let run = SuiteRun {
        command: &ctx.config.test_command,
        timeout: CULPRIT_TIMEOUT,
        log_dir: &ctx.suite_dir,
    };
    guilty_tasks(&ctx.git, &base, &pairs, &run).await
}

/// A task is guilty when its own change makes the suite fail on the base tree. A probe that could
/// not build the tree, could not start the suite, or timed out gives no verdict.
pub(crate) async fn guilty_tasks(
    git: &Git,
    base: &str,
    tasks: &[(TaskId, Checkpoint)],
    run: &SuiteRun<'_>,
) -> Vec<TaskId> {
    let probe = Probe {
        git,
        base,
        run: *run,
    };
    let mut results = Vec::with_capacity(tasks.len());
    for (task, checkpoint) in tasks {
        results.push((task.clone(), probe_task(&probe, task, checkpoint).await));
    }
    guilty_ids(&results)
}

/// `None` means the probe gave no verdict. Only a suite that ran and failed makes a task guilty.
fn guilty_ids(results: &[(TaskId, Option<bool>)]) -> Vec<TaskId> {
    results
        .iter()
        .filter(|(_, passed)| *passed == Some(false))
        .map(|(id, _)| id.clone())
        .collect()
}

/// The temp folder goes away when `folder` drops, on every path.
async fn probe_task(probe: &Probe<'_>, task: &TaskId, checkpoint: &Checkpoint) -> Option<bool> {
    let folder = tempfile::tempdir().ok()?;
    rebuild_tree(probe.git, probe.base, folder.path()).ok()?;
    overlay_task_files(probe.git, checkpoint, folder.path()).ok()?;
    let config = SuiteConfig {
        command: probe.run.command.to_vec(),
        timeout: probe.run.timeout,
        log_dir: probe.run.log_dir.to_path_buf(),
    };
    let result = run_suite(folder.path(), &config, &format!("culprit-{task}"))
        .await
        .ok()?;
    (!result.timed_out).then_some(result.passed)
}

/// Lay the task's own files over the base tree. A file the task deleted is removed.
fn overlay_task_files(git: &Git, checkpoint: &Checkpoint, root: &Path) -> ProofResult {
    for path in &checkpoint.paths {
        let target = root.join(path);
        match git.file_at(&checkpoint.commit, path)? {
            Some(bytes) => write_bytes(&target, &bytes, false)?,
            None => remove_if_file(&target)?,
        }
    }
    Ok(())
}

fn remove_if_file(path: &Path) -> ProofResult {
    if path.is_file() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn checkpoints_of(ctx: &RunCtx, tasks: &[TaskId]) -> Vec<(TaskId, Checkpoint)> {
    let memory = lock(&ctx.memory);
    tasks
        .iter()
        .filter_map(|id| {
            memory
                .checkpoints
                .get(id)
                .map(|cp| (id.clone(), cp.clone()))
        })
        .collect()
}

fn id_list(ids: &[TaskId]) -> String {
    ids.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn say(ctx: &RunCtx, text: String) -> Result<()> {
    ctx.engine.emit(&Event::Message {
        from: "sentinel".to_owned(),
        to: None,
        text,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    const CHECK: &str = "grep -q ok a.txt && grep -q ok b.txt\n";

    #[test]
    fn only_tasks_whose_probe_failed_are_guilty() {
        let results = vec![
            (TaskId::new("T-1"), Some(false)),
            (TaskId::new("T-2"), Some(true)),
            (TaskId::new("T-3"), None),
        ];
        assert_eq!(guilty_ids(&results), vec![TaskId::new("T-1")]);
    }

    #[test]
    fn no_probe_results_give_no_guilty_task() {
        assert_eq!(guilty_ids(&[]), Vec::<TaskId>::new());
    }

    fn git_in(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(root)
            .args(["-c", "user.name=sentinel-test"])
            .args(["-c", "user.email=sentinel-test@example.com"])
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?} failed");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// Write each file, stage every change, and commit. Returns the new HEAD.
    fn commit_files(root: &Path, files: &[(&str, &str)]) -> String {
        for (path, text) in files {
            fs::write(root.join(path), text).unwrap();
        }
        git_in(root, &["add", "-A"]);
        git_in(root, &["commit", "-q", "-m", "change"]);
        git_in(root, &["rev-parse", "HEAD"])
    }

    fn new_repo() -> (tempfile::TempDir, Git) {
        let dir = tempfile::tempdir().unwrap();
        git_in(dir.path(), &["init", "-q"]);
        let git = Git::new(dir.path(), dir.path().join(".git/private.index")).unwrap();
        (dir, git)
    }

    fn checkpoint(commit: String, path: &str) -> Checkpoint {
        Checkpoint {
            commit,
            paths: vec![path.to_owned()],
        }
    }

    async fn guilty_in(git: &Git, base: &str, tasks: &[(TaskId, Checkpoint)]) -> Vec<TaskId> {
        let logs = tempfile::tempdir().unwrap();
        let command = vec!["sh".to_owned(), "check.sh".to_owned()];
        let run = SuiteRun {
            command: &command,
            timeout: Duration::from_secs(60),
            log_dir: logs.path(),
        };
        guilty_tasks(git, base, tasks, &run).await
    }

    #[tokio::test]
    async fn only_the_task_that_breaks_the_suite_alone_is_guilty() {
        let (dir, git) = new_repo();
        let root = dir.path();
        let base = commit_files(
            root,
            &[("a.txt", "ok"), ("b.txt", "ok"), ("check.sh", CHECK)],
        );
        let first = checkpoint(commit_files(root, &[("a.txt", "bad")]), "a.txt");
        let second = checkpoint(commit_files(root, &[("b.txt", "fine ok")]), "b.txt");
        let tasks = vec![(TaskId::new("T-1"), first), (TaskId::new("T-2"), second)];
        assert_eq!(
            guilty_in(&git, &base, &tasks).await,
            vec![TaskId::new("T-1")]
        );
    }

    #[tokio::test]
    async fn every_task_that_breaks_the_suite_alone_is_guilty() {
        let (dir, git) = new_repo();
        let root = dir.path();
        let base = commit_files(
            root,
            &[("a.txt", "ok"), ("b.txt", "ok"), ("check.sh", CHECK)],
        );
        let first = checkpoint(commit_files(root, &[("a.txt", "bad")]), "a.txt");
        let second = checkpoint(commit_files(root, &[("b.txt", "bad")]), "b.txt");
        let tasks = vec![(TaskId::new("T-1"), first), (TaskId::new("T-2"), second)];
        assert_eq!(
            guilty_in(&git, &base, &tasks).await,
            vec![TaskId::new("T-1"), TaskId::new("T-2")]
        );
    }

    #[tokio::test]
    async fn every_task_is_guilty_when_the_base_tree_already_fails() {
        let (dir, git) = new_repo();
        let root = dir.path();
        let base = commit_files(
            root,
            &[("a.txt", "bad"), ("b.txt", "ok"), ("check.sh", CHECK)],
        );
        let first = checkpoint(commit_files(root, &[("b.txt", "fine ok")]), "b.txt");
        let second = checkpoint(commit_files(root, &[("b.txt", "fine ok 2")]), "b.txt");
        let tasks = vec![(TaskId::new("T-1"), first), (TaskId::new("T-2"), second)];
        assert_eq!(
            guilty_in(&git, &base, &tasks).await,
            vec![TaskId::new("T-1"), TaskId::new("T-2")]
        );
    }
}
