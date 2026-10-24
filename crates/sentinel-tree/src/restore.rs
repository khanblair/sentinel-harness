//! Put one work tree file back to its state at a commit. No index is touched.

use std::fs;
use std::path::Path;

use crate::git::Git;
use crate::{Result, TreeError, repo_path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreOutcome {
    Rewritten,
    Removed,
    Unchanged,
}

pub fn restore_path(git: &Git, path: &str, commit: &str) -> Result<RestoreOutcome> {
    let path = repo_path(path)?;
    let target = git.root().join(Path::new(&path));
    match git.entry_at(commit, &path)? {
        Some(entry) => {
            let bytes = git.blob(&entry.oid)?;
            if fs::read(&target).is_ok_and(|current| current == bytes) {
                return Ok(RestoreOutcome::Unchanged);
            }
            write_file(&target, &bytes, entry.mode == "100755")?;
            Ok(RestoreOutcome::Rewritten)
        }
        None => remove_file(&target),
    }
}

fn write_file(target: &Path, bytes: &[u8], executable: bool) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(TreeError::Io)?;
    }
    fs::write(target, bytes).map_err(TreeError::Io)?;
    set_executable(target, executable)
}

fn remove_file(target: &Path) -> Result<RestoreOutcome> {
    match fs::remove_file(target) {
        Ok(()) => Ok(RestoreOutcome::Removed),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(RestoreOutcome::Unchanged),
        Err(error) => Err(TreeError::Io(error)),
    }
}

#[cfg(unix)]
fn set_executable(target: &Path, executable: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(target).map_err(TreeError::Io)?;
    let mut permissions = metadata.permissions();
    let mode = permissions.mode();
    let wanted = if executable {
        mode | 0o111
    } else {
        mode & !0o111
    };
    if wanted != mode {
        permissions.set_mode(wanted);
        fs::set_permissions(target, permissions).map_err(TreeError::Io)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_target: &Path, _executable: bool) -> Result<()> {
    Ok(())
}
