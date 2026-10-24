//! Undo a checkpoint whose task failed verification, so its code does not break the next wave.

use crate::Result;
use crate::checkpoint::{Checkpoint, checkpoint};
use crate::git::Git;
use crate::restore::restore_path;

/// Put every path of `target` back to its state before that checkpoint, then checkpoint the result
/// as a new commit. History stays append only. Returns `None` when nothing differs.
pub fn revert_checkpoint(
    git: &Git,
    target: &Checkpoint,
    message: &str,
) -> Result<Option<Checkpoint>> {
    let parent = format!("{}^", target.commit);
    for path in &target.paths {
        restore_path(git, path, &parent)?;
    }
    checkpoint(git, &target.paths, message)
}
