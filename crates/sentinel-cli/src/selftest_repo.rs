//! The throwaway repository of `sentinel selftest`: its files, the scripted plans, and the folder
//! that holds them for one scenario.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};

use crate::selftest_cases::{Scenario, TEST_SCRIPT};

pub(crate) const BACKLOG_FILE: &str = "backlog.md";

/// The files of the throwaway repository. A scenario's backlog is added on top of them.
/// `.gitignore` keeps the engine's `.sentinel` folder out of the clean-tree check.
pub(crate) const REPO_FILES: [(&str, &str); 6] = [
    ("src/alpha.txt", "alpha original\n"),
    ("src/beta.txt", "beta original\n"),
    ("src/gamma.txt", "gamma original\n"),
    ("tests/run.sh", TEST_SCRIPT),
    ("README.md", "# selftest repo\n"),
    (".gitignore", ".sentinel/\n"),
];

pub(crate) const FAKE_AGENT: &str = include_str!("../../../fixtures/selftest/fake-agent");

/// The throwaway repository: the base files and the backlog, committed on `main`.
pub(crate) fn write_repo(repo: &Path, backlog: &str) -> anyhow::Result<()> {
    for (path, text) in REPO_FILES {
        write_file(&repo.join(path), text)?;
    }
    write_file(&repo.join(BACKLOG_FILE), backlog)?;
    git(repo, &["init", "-b", "main"])?;
    git(repo, &["config", "user.name", "Sentinel selftest"])?;
    git(repo, &["config", "user.email", "selftest@example.com"])?;
    git(repo, &["add", "-A"])?;
    git(repo, &["commit", "-m", "start"])
}

/// The scripted plans sit outside the repository, so they never show up as changes.
pub(crate) fn write_plans(plans: &Path, scenario: &Scenario) -> anyhow::Result<()> {
    write_file(&plans.join("plan.json"), scenario.plan_json)?;
    for (name, text) in scenario.plans {
        write_file(&plans.join(name), text)?;
    }
    Ok(())
}

pub(crate) fn write_script(path: &Path) -> anyhow::Result<()> {
    write_file(path, FAKE_AGENT)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .context("could not make the fake agent runnable")
}

pub(crate) fn write_file(path: &Path, text: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text).with_context(|| format!("could not write {}", path.display()))
}

pub(crate) fn git(repo: &Path, args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .context("could not start git")?;
    if output.status.success() {
        return Ok(());
    }
    bail!(
        "git {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

/// A folder under the system temp folder, removed when the scenario ends.
pub(crate) struct Scratch(pub(crate) PathBuf);

impl Scratch {
    pub(crate) fn new(name: &str) -> anyhow::Result<Self> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "sentinel-selftest-{name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).with_context(|| format!("could not make {}", path.display()))?;
        Ok(Self(fs::canonicalize(&path)?))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
