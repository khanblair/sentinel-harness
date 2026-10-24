//! Lists the files a check should look at.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

/// Folders that never hold source we own.
const SKIP_DIRS: [&str; 7] = [
    ".git",
    "target",
    ".sentinel",
    ".build",
    "node_modules",
    "dist",
    "book",
];

/// Files under `root` as paths relative to `root`, sorted.
///
/// Inside a git repo this is `git ls-files` (tracked plus untracked and not ignored), so ignored
/// folders never count. Elsewhere, such as a temp fixture, it walks the folder.
pub fn repo_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = match git_files(root) {
        Some(files) => files,
        None => walk(root, root)?,
    };
    files.retain(|rel| root.join(rel).is_file());
    files.sort();
    Ok(files)
}

fn git_files(root: &Path) -> Option<Vec<PathBuf>> {
    if !root.join(".git").exists() {
        return None;
    }
    let out = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    Some(
        text.split('\0')
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect(),
    )
}

fn walk(root: &Path, dir: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            let skip = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| SKIP_DIRS.contains(&n));
            if !skip {
                found.extend(walk(root, &path)?);
            }
        } else if let Ok(rel) = path.strip_prefix(root) {
            found.push(rel.to_path_buf());
        }
    }
    Ok(found)
}

/// Read a file as text. Returns `None` for binary or unreadable files so checks skip them.
pub fn read_text(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_skips_target_and_build_folders() {
        let dir = tempfile::tempdir().unwrap();
        for rel in ["src/a.rs", "target/b.rs", ".build/c.md", "docs/d.md"] {
            let path = dir.path().join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "x").unwrap();
        }
        let files = repo_files(dir.path()).unwrap();
        assert_eq!(
            files,
            vec![PathBuf::from("docs/d.md"), PathBuf::from("src/a.rs")]
        );
    }

    #[test]
    fn read_text_returns_none_for_binary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.bin");
        fs::write(&path, [0xff, 0xfe, 0x00]).unwrap();
        assert!(read_text(&path).is_none());
    }
}
