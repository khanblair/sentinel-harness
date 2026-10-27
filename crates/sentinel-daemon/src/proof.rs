//! Proof that a verified task's new tests catch the change. They must fail on the code from
//! before the task. The check rebuilds the parent tree in a temp folder, lays the new test
//! files on top, and runs the suite there.

use std::fs;
use std::path::Path;
use std::time::Duration;

use sentinel_core::{Event, TaskId, TaskStatus};
use sentinel_tree::{Checkpoint, Git, TreeError};
use sentinel_verify::gate::{SuiteConfig, run_suite};
use sentinel_verify::verdict::{Verdict, looks_like_test_path};
use thiserror::Error;

use crate::engine::Result;
use crate::run_ctx::RunCtx;

/// Bigger repos take too long to rebuild file by file.
const MAX_LISTED_FILES: usize = 5000;
const SUITE_TIMEOUT: Duration = Duration::from_secs(120);

/// What the check showed about the new tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proof {
    /// The new tests fail without the change.
    Proven,
    /// The new tests pass without the change, so they do not show the change matters.
    Unproven(String),
    /// The check could not finish, so the verdict stands as it was.
    Unknown(String),
}

#[derive(Debug, Error)]
pub(crate) enum ProofError {
    #[error("{0}")]
    Tree(#[from] TreeError),
    #[error("could not write the check folder: {0}")]
    Io(#[from] std::io::Error),
    #[error("the repository is too large for the check")]
    TooLarge,
}

pub(crate) type ProofResult<T = ()> = std::result::Result<T, ProofError>;

/// The task whose verdict is being proven, with the diff that the verdict was read from.
pub(crate) struct ProofTarget<'a> {
    pub id: &'a TaskId,
    pub checkpoint: &'a Checkpoint,
    pub diff: &'a str,
}

/// One line of `git ls-tree -r -z`: `MODE TYPE OID<TAB>PATH`.
struct Listed {
    path: String,
    blob: bool,
    executable: bool,
}

/// The paths of files the diff adds that look like test files.
pub fn added_test_files(diff: &str) -> Vec<String> {
    let mut added = Vec::new();
    let mut after_dev_null = false;
    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ b/")
            && after_dev_null
            && looks_like_test_path(path)
        {
            added.push(path.to_owned());
        }
        after_dev_null = line == "--- /dev/null";
    }
    added
}

pub fn proof_from_suite(passed: bool, timed_out: bool) -> Proof {
    if timed_out {
        Proof::Unknown("the check timed out".to_owned())
    } else if passed {
        Proof::Unproven(
            "The new tests also pass without the change, so they do not prove it.".to_owned(),
        )
    } else {
        Proof::Proven
    }
}

/// Run the new tests on the parent tree. The temp folder goes away on every path.
pub(crate) async fn prove_fails_before(ctx: &RunCtx, checkpoint: &Checkpoint, diff: &str) -> Proof {
    let test_files = added_test_files(diff);
    if test_files.is_empty() {
        return Proof::Unknown("no new test file was added".to_owned());
    }
    match run_proof_suite(ctx, checkpoint, &test_files).await {
        Ok(proof) => proof,
        Err(error) => Proof::Unknown(error.to_string()),
    }
}

async fn run_proof_suite(
    ctx: &RunCtx,
    checkpoint: &Checkpoint,
    test_files: &[String],
) -> ProofResult<Proof> {
    let folder = tempfile::tempdir()?;
    build_check_tree(&ctx.git, &checkpoint.commit, test_files, folder.path())?;
    let config = SuiteConfig {
        command: ctx.config.test_command.clone(),
        timeout: SUITE_TIMEOUT,
        log_dir: ctx.suite_dir.clone(),
    };
    let label = format!("proof-{}", checkpoint.commit);
    let proof = match run_suite(folder.path(), &config, &label).await {
        Ok(result) => proof_from_suite(result.passed, result.timed_out),
        Err(error) => Proof::Unknown(format!("the check could not start: {error}")),
    };
    Ok(proof)
}

/// The parent tree, then the new test files from the checkpoint over it.
fn build_check_tree(git: &Git, commit: &str, test_files: &[String], root: &Path) -> ProofResult {
    rebuild_parent_tree(git, commit, root)?;
    for path in test_files {
        if let Some(bytes) = git.file_at(commit, path)? {
            write_bytes(&root.join(path), &bytes, false)?;
        }
    }
    Ok(())
}

/// Write every file of `commit^` into `root`.
fn rebuild_parent_tree(git: &Git, commit: &str, root: &Path) -> ProofResult {
    rebuild_tree(git, &format!("{commit}^"), root)
}

/// Write every file of `rev` into `root`. Submodules and other non-blob entries are skipped.
pub(crate) fn rebuild_tree(git: &Git, rev: &str, root: &Path) -> ProofResult {
    let raw = git.run(&["ls-tree", "-r", "-z", rev])?;
    let listed = parse_listing(&raw);
    if listed.len() > MAX_LISTED_FILES {
        return Err(ProofError::TooLarge);
    }
    for file in listed.iter().filter(|file| file.blob) {
        if let Some(bytes) = git.file_at(rev, &file.path)? {
            write_bytes(&root.join(&file.path), &bytes, file.executable)?;
        }
    }
    Ok(())
}

fn parse_listing(raw: &str) -> Vec<Listed> {
    raw.split('\0').filter_map(parse_record).collect()
}

fn parse_record(record: &str) -> Option<Listed> {
    let (meta, path) = record.split_once('\t')?;
    let mut parts = meta.split(' ');
    let mode = parts.next()?;
    let kind = parts.next()?;
    Some(Listed {
        path: path.to_owned(),
        blob: kind == "blob",
        executable: mode == "100755",
    })
}

pub(crate) fn write_bytes(path: &Path, bytes: &[u8], executable: bool) -> ProofResult {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    set_executable(path, executable)
}

#[cfg(unix)]
fn set_executable(path: &Path, executable: bool) -> ProofResult {
    use std::os::unix::fs::PermissionsExt;
    if executable {
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_path: &Path, _executable: bool) -> ProofResult {
    Ok(())
}

/// Run the proof for a verdict that would be Verified. Returns the verdict to record.
pub(crate) async fn prove_verdict(
    ctx: &RunCtx,
    target: &ProofTarget<'_>,
    verdict: Verdict,
) -> Result<Verdict> {
    if !ctx.config.prove_new_tests || verdict.status != TaskStatus::Verified {
        return Ok(verdict);
    }
    let proof = prove_fails_before(ctx, target.checkpoint, target.diff).await;
    if let Some(text) = proof_message(target.id, &proof) {
        ctx.engine.emit(&Event::Message {
            from: "sentinel".to_owned(),
            to: None,
            text,
        })?;
    }
    Ok(apply_proof(verdict, proof))
}

fn proof_message(id: &TaskId, proof: &Proof) -> Option<String> {
    match proof {
        Proof::Proven => Some(format!(
            "{id}: the new tests fail without the change, so they prove it."
        )),
        Proof::Unknown(reason) => Some(format!("proof skipped for {id}: {reason}")),
        Proof::Unproven(_) => None,
    }
}

/// Proven keeps Verified and adds a note. Unproven makes it Partial. Unknown changes nothing.
fn apply_proof(verdict: Verdict, proof: Proof) -> Verdict {
    match proof {
        Proof::Proven => Verdict {
            reason: format!(
                "{}. The new tests fail without the change.",
                verdict.reason.trim_end_matches('.')
            ),
            ..verdict
        },
        Proof::Unproven(reason) => Verdict {
            status: TaskStatus::Partial,
            reason,
        },
        Proof::Unknown(_) => verdict,
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    const DIFF: &str = r"diff --git a/tests/new.rs b/tests/new.rs
new file mode 100644
--- /dev/null
+++ b/tests/new.rs
@@ -0,0 +1 @@
+fn check() {}
diff --git a/src/lib.rs b/src/lib.rs
new file mode 100644
--- /dev/null
+++ b/src/lib.rs
@@ -0,0 +1 @@
+pub fn value() {}
diff --git a/tests/old.rs b/tests/old.rs
--- a/tests/old.rs
+++ b/tests/old.rs
@@ -1 +1 @@
-fn old() {}
+fn old() { }
";

    #[test]
    fn added_test_files_lists_only_added_test_paths() {
        assert_eq!(added_test_files(DIFF), vec!["tests/new.rs".to_owned()]);
    }

    #[test]
    fn added_test_files_ignores_an_added_source_file() {
        let diff = "--- /dev/null\n+++ b/src/lib.rs\n";
        assert_eq!(added_test_files(diff), Vec::<String>::new());
    }

    #[test]
    fn added_test_files_ignores_an_edited_test_file() {
        let diff = "--- a/tests/old.rs\n+++ b/tests/old.rs\n";
        assert_eq!(added_test_files(diff), Vec::<String>::new());
    }

    #[test]
    fn timed_out_check_is_unknown() {
        assert_eq!(
            proof_from_suite(false, true),
            Proof::Unknown("the check timed out".to_owned())
        );
    }

    #[test]
    fn timeout_wins_over_a_passing_suite() {
        assert!(matches!(proof_from_suite(true, true), Proof::Unknown(_)));
    }

    #[test]
    fn passing_suite_without_the_change_is_unproven() {
        assert!(matches!(proof_from_suite(true, false), Proof::Unproven(_)));
    }

    #[test]
    fn failing_suite_without_the_change_is_proven() {
        assert_eq!(proof_from_suite(false, false), Proof::Proven);
    }

    fn verified() -> Verdict {
        Verdict {
            status: TaskStatus::Verified,
            reason: "all four checks passed".to_owned(),
        }
    }

    #[test]
    fn proven_keeps_verified_and_adds_the_note() {
        let verdict = apply_proof(verified(), Proof::Proven);
        assert_eq!(verdict.status, TaskStatus::Verified);
        assert_eq!(
            verdict.reason,
            "all four checks passed. The new tests fail without the change."
        );
    }

    #[test]
    fn unproven_turns_verified_into_partial() {
        let proof = Proof::Unproven("they pass anyway".to_owned());
        let verdict = apply_proof(verified(), proof);
        assert_eq!(verdict.status, TaskStatus::Partial);
        assert_eq!(verdict.reason, "they pass anyway");
    }

    #[test]
    fn unknown_proof_keeps_the_verdict() {
        let proof = Proof::Unknown("the check timed out".to_owned());
        assert_eq!(apply_proof(verified(), proof), verified());
    }

    fn git_in(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(root)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    fn commit(root: &Path, message: &str) {
        git_in(
            root,
            &[
                "-c",
                "user.name=sentinel-test",
                "-c",
                "user.email=sentinel-test@example.com",
                "commit",
                "-q",
                "-m",
                message,
            ],
        );
    }

    #[test]
    fn rebuilt_parent_tree_runs_the_new_test_and_it_fails() {
        let repo = tempfile::tempdir().unwrap();
        let root = repo.path();
        git_in(root, &["init", "-q"]);
        fs::write(root.join("a.txt"), "no").unwrap();
        git_in(root, &["add", "a.txt"]);
        commit(root, "first");
        fs::create_dir(root.join("tests")).unwrap();
        fs::write(root.join("tests/new.sh"), "#!/bin/sh\ngrep -q ok a.txt\n").unwrap();
        fs::write(root.join("a.txt"), "ok").unwrap();
        git_in(root, &["add", "a.txt", "tests/new.sh"]);
        commit(root, "second");

        let git = Git::new(root, root.join("private.index")).unwrap();
        let second = git.run(&["rev-parse", "HEAD"]).unwrap();
        let check = tempfile::tempdir().unwrap();
        build_check_tree(&git, &second, &["tests/new.sh".to_owned()], check.path()).unwrap();

        assert_eq!(
            fs::read_to_string(check.path().join("a.txt")).unwrap(),
            "no"
        );
        let status = Command::new("sh")
            .arg("tests/new.sh")
            .current_dir(check.path())
            .status()
            .unwrap();
        assert!(!status.success());
    }
}
