//! Commit a task's paths with the run's private index.

use crate::git::Git;
use crate::{Result, repo_path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub commit: String,
    pub paths: Vec<String>,
}

/// Commit only `paths`. Returns `None` when their staged form matches the current HEAD tree.
pub fn checkpoint(git: &Git, paths: &[String], message: &str) -> Result<Option<Checkpoint>> {
    let paths = normalize(paths)?;
    if paths.is_empty() {
        return Ok(None);
    }
    let previous = git.head()?;
    git.run(&stage_args(&paths))?;
    let tree = git.run(&["write-tree"])?;
    if is_unchanged(git, previous.as_deref(), &tree)? {
        return Ok(None);
    }
    let commit = create_commit(git, &tree, previous.as_deref(), message)?;
    update_head(git, previous.as_deref(), &commit)?;
    Ok(Some(Checkpoint { commit, paths }))
}

fn normalize(paths: &[String]) -> Result<Vec<String>> {
    paths.iter().map(|path| repo_path(path)).collect()
}

fn stage_args(paths: &[String]) -> Vec<&str> {
    let mut args = vec!["add", "-A", "--"];
    args.extend(paths.iter().map(String::as_str));
    args
}

fn is_unchanged(git: &Git, previous: Option<&str>, tree: &str) -> Result<bool> {
    let known = match previous {
        Some(head) => git.run(&["rev-parse", &format!("{head}^{{tree}}")])?,
        None => git.run(&["mktree"])?,
    };
    Ok(tree == known)
}

fn create_commit(git: &Git, tree: &str, previous: Option<&str>, message: &str) -> Result<String> {
    let mut args = vec!["commit-tree", tree];
    if let Some(head) = previous {
        args.push("-p");
        args.push(head);
    }
    args.push("-m");
    args.push(message);
    git.run_with_config(
        &[
            ("user.name", "Sentinel"),
            ("user.email", "sentinel@localhost"),
        ],
        &args,
    )
}

/// Move HEAD only if it still points where the checkpoint started. A moved HEAD fails here.
fn update_head(git: &Git, previous: Option<&str>, commit: &str) -> Result<()> {
    match previous {
        Some(head) => {
            git.run(&["update-ref", "HEAD", commit, head])?;
        }
        None => {
            git.run(&["update-ref", "HEAD", commit])?;
        }
    }
    Ok(())
}
