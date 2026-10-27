//! Thin wrapper over the git binary.
//!
//! The wrapper owns one private index file and points `GIT_INDEX_FILE` at it on every call.
//! That is what keeps the user's real index untouched while a run makes checkpoints.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::{Result, TreeError, repo_path};

#[derive(Debug, Clone)]
pub struct Git {
    root: PathBuf,
    private_index: PathBuf,
}

pub(crate) struct TreeEntry {
    pub(crate) mode: String,
    pub(crate) oid: String,
}

impl Git {
    pub fn new(root: impl Into<PathBuf>, private_index: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        if !root.join(".git").exists() {
            return Err(TreeError::NotARepo(root));
        }
        let private_index = private_index.into();
        let private_index = if private_index.is_absolute() {
            private_index
        } else {
            root.join(private_index)
        };
        create_parent(&private_index)?;
        Ok(Self {
            root,
            private_index,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn run(&self, args: &[&str]) -> Result<String> {
        Ok(trim_one_newline(&self.run_bytes(args)?))
    }

    pub fn run_bytes(&self, args: &[&str]) -> Result<Vec<u8>> {
        self.execute(&[], args)
    }

    pub(crate) fn run_with_config(&self, config: &[(&str, &str)], args: &[&str]) -> Result<String> {
        Ok(trim_one_newline(&self.execute(config, args)?))
    }

    /// Run git with the private index and the environment a run needs.
    fn execute(&self, config: &[(&str, &str)], args: &[&str]) -> Result<Vec<u8>> {
        let mut command = Command::new("git");
        command
            .current_dir(&self.root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C")
            .env("GIT_INDEX_FILE", &self.private_index)
            .args(["-c", "core.quotepath=off"])
            .stdin(Stdio::null());
        for (key, value) in config {
            command.arg("-c").arg(format!("{key}={value}"));
        }
        let output = command.args(args).output().map_err(TreeError::Io)?;
        if !output.status.success() {
            return Err(TreeError::Git {
                args: args.join(" "),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        Ok(output.stdout)
    }

    /// The current commit, or `None` in a repo with no commit yet.
    pub fn head(&self) -> Result<Option<String>> {
        match self.run(&["rev-parse", "--verify", "--quiet", "HEAD"]) {
            Ok(head) if !head.is_empty() => Ok(Some(head)),
            Ok(_) | Err(TreeError::Git { .. }) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Load the private index from HEAD, or empty it when there is no HEAD.
    pub fn reset_private_index(&self) -> Result<()> {
        if self.head()?.is_some() {
            self.run(&["read-tree", "HEAD"])?;
        } else {
            self.run(&["read-tree", "--empty"])?;
        }
        Ok(())
    }

    /// Put the user's real index back in step with HEAD. Checkpoints move HEAD through the private
    /// index, so without this `git status` shows every checkpointed file as staged the other way
    /// and a plain `git commit` would commit the old state. The work tree is not touched. Call it
    /// only while no agent runs, because it takes the real index lock.
    pub fn sync_real_index(&self) -> Result<()> {
        if self.head()?.is_none() {
            return Ok(());
        }
        let output = Command::new("git")
            .current_dir(&self.root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C")
            .env_remove("GIT_INDEX_FILE")
            .args(["reset", "--quiet"])
            .stdin(Stdio::null())
            .output()
            .map_err(TreeError::Io)?;
        if output.status.success() {
            return Ok(());
        }
        Err(TreeError::Git {
            args: "reset --quiet".to_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }

    /// Paths whose work tree form differs from the private index, plus untracked files.
    pub fn changed_paths(&self) -> Result<Vec<String>> {
        let raw = self.run_bytes(&["status", "--porcelain=v1", "-z", "--untracked-files=all"])?;
        let mut paths = BTreeSet::new();
        collect_status_paths(&raw, &mut paths);
        Ok(paths
            .into_iter()
            .filter(|path| !is_sentinel_path(path))
            .collect())
    }

    pub fn is_clean(&self) -> Result<bool> {
        if !self.changed_paths()?.is_empty() {
            return Ok(false);
        }
        let index_tree = self.run(&["write-tree"])?;
        let head_tree = match self.head()? {
            Some(_) => self.run(&["rev-parse", "HEAD^{tree}"])?,
            None => self.run(&["mktree"])?,
        };
        Ok(index_tree == head_tree)
    }

    pub fn file_at(&self, commit: &str, path: &str) -> Result<Option<Vec<u8>>> {
        match self.entry_at(commit, path)? {
            Some(entry) => Ok(Some(self.blob(&entry.oid)?)),
            None => Ok(None),
        }
    }

    pub(crate) fn entry_at(&self, commit: &str, path: &str) -> Result<Option<TreeEntry>> {
        let path = repo_path(path)?;
        let spec = format!(":(literal){path}");
        let raw = self.run_bytes(&["ls-tree", "-z", commit, "--", &spec])?;
        Ok(parse_tree_entry(&raw, &path))
    }

    pub(crate) fn blob(&self, oid: &str) -> Result<Vec<u8>> {
        self.run_bytes(&["cat-file", "blob", oid])
    }
}

fn create_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(TreeError::Io)?;
    }
    Ok(())
}

fn trim_one_newline(raw: &[u8]) -> String {
    let mut text = String::from_utf8_lossy(raw).into_owned();
    if text.ends_with('\n') {
        text.pop();
        if text.ends_with('\r') {
            text.pop();
        }
    }
    text
}

/// Keep the work tree side of each status record. Staged-only changes are left out.
fn collect_status_paths(raw: &[u8], paths: &mut BTreeSet<String>) {
    let mut records = raw.split(|byte| *byte == 0);
    while let Some(record) = records.next() {
        if record.len() < 4 {
            continue;
        }
        let index_status = record[0];
        let worktree_status = record[1];
        if worktree_status != b' ' && index_status != b'!' {
            paths.insert(String::from_utf8_lossy(&record[3..]).into_owned());
        }
        let renamed = [index_status, worktree_status]
            .iter()
            .any(|status| matches!(status, b'R' | b'C'));
        if renamed {
            records.next();
        }
    }
}

fn is_sentinel_path(path: &str) -> bool {
    path == ".sentinel" || path.starts_with(".sentinel/")
}

fn parse_tree_entry(raw: &[u8], want: &str) -> Option<TreeEntry> {
    for record in raw.split(|byte| *byte == 0) {
        let Some(tab) = record.iter().position(|byte| *byte == b'\t') else {
            continue;
        };
        if &record[tab + 1..] != want.as_bytes() {
            continue;
        }
        let meta = String::from_utf8_lossy(&record[..tab]);
        let mut parts = meta.split(' ');
        let mode = parts.next()?;
        let kind = parts.next()?;
        let oid = parts.next()?;
        if kind != "blob" {
            return None;
        }
        return Some(TreeEntry {
            mode: mode.to_owned(),
            oid: oid.to_owned(),
        });
    }
    None
}
