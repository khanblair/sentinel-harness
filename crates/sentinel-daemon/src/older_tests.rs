//! A later task can change behaviour that an earlier task's own tests check. Those tests are files
//! Sentinel made in `sentinel-tests/`, not the locked tests of the repository, so the later task
//! may update them. Without this, the earlier task's test blocks the later task for good.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use sentinel_core::{Event, Scope, TaskId};
use sentinel_plan::planner::OWN_TEST_FOLDER;

use crate::engine::lock;
use crate::run_ctx::RunCtx;

/// The longest part of a suite log that is searched for test files.
const LOG_LIMIT: usize = 4 * 1024 * 1024;

/// The files in the run's own test folder that the text names, in order, once each.
pub(crate) fn own_test_files(text: &str) -> Vec<String> {
    let mut found = BTreeSet::new();
    let mut rest = text;
    while let Some(start) = rest.find(OWN_TEST_FOLDER) {
        let tail = &rest[start..];
        let end = tail
            .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-')))
            .unwrap_or(tail.len());
        let path = &tail[..end];
        if path.len() > OWN_TEST_FOLDER.len() + 1 && path.as_bytes()[OWN_TEST_FOLDER.len()] == b'/'
        {
            found.insert(path.to_owned());
        }
        rest = &tail[end.max(1)..];
    }
    found.into_iter().collect()
}

/// A suite failure names tests of earlier tasks: the task may update them on its next try.
/// Returns the sentence that tells the agent so, or `None` when no such test failed.
pub(crate) fn allow_older_tests(ctx: &RunCtx, task: &TaskId, log: &Path) -> Option<String> {
    let text = read_limited(log)?;
    let own = own_test_path(ctx, task);
    let repo = ctx.engine.repo_path();
    let files: Vec<String> = own_test_files(&text)
        .into_iter()
        .filter(|file| Some(file) != own.as_ref() && repo.join(file).is_file())
        .collect();
    if files.is_empty() {
        return None;
    }
    widen(ctx, task, &files)?;
    let list = files.join(", ");
    ctx.engine.emit_quiet(&Event::Message {
        from: "sentinel".to_owned(),
        to: None,
        text: format!("{task} may also update {list} on its next try: those tests of earlier tasks failed with its change."),
    });
    Some(format!(
        "These tests belong to earlier tasks of this run, not to the locked tests: {list}. \
         If your ticket changes what they check, update them to match your ticket and keep \
         everything else they check. Do not delete them."
    ))
}

fn read_limited(log: &Path) -> Option<String> {
    let bytes = fs::read(log).ok()?;
    let slice = &bytes[..bytes.len().min(LOG_LIMIT)];
    Some(String::from_utf8_lossy(slice).into_owned())
}

/// The new test file of the task, which is already in its scope.
fn own_test_path(ctx: &RunCtx, task: &TaskId) -> Option<String> {
    let task = ctx.engine.store().get_task(task).ok()??;
    task.scope
        .entries()
        .find(|entry| entry.starts_with(OWN_TEST_FOLDER))
        .map(str::to_owned)
}

fn widen(ctx: &RunCtx, task: &TaskId, files: &[String]) -> Option<()> {
    let current = ctx.engine.store().get_task(task).ok()??;
    let mut paths: Vec<String> = current.scope.entries().map(str::to_owned).collect();
    for file in files {
        if !paths.contains(file) {
            paths.push(file.clone());
        }
    }
    let wider = Scope::from_paths(&paths).ok()?;
    ctx.engine.store().set_task_scope(task, &wider).ok()?;
    lock(&ctx.memory)
        .older_tests
        .entry(task.clone())
        .or_default()
        .extend(files.iter().cloned());
    Some(())
}

/// Edits to test files that the task may update are not tampering.
pub(crate) fn without_allowed(ctx: &RunCtx, task: &TaskId, edited: Vec<String>) -> Vec<String> {
    let memory = lock(&ctx.memory);
    let Some(allowed) = memory.older_tests.get(task) else {
        return edited;
    };
    edited
        .into_iter()
        .filter(|file| !allowed.contains(file))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_files_are_found_in_node_runner_output() {
        let text = "test at sentinel-tests/t02.test.js:31:1\n  at file:///x/sentinel-tests/t02.test.js:37:10\ntest at sentinel-tests/t05.test.js:9:1";

        assert_eq!(
            own_test_files(text),
            vec!["sentinel-tests/t02.test.js", "sentinel-tests/t05.test.js"]
        );
    }

    #[test]
    fn the_bare_folder_name_is_not_a_test_file() {
        assert_eq!(
            own_test_files("run node --test sentinel-tests/*.test.js and sentinel-tests"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_text_with_no_test_file_gives_nothing() {
        assert_eq!(own_test_files("all good"), Vec::<String>::new());
    }
}
