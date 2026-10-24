//! Watch the tree and restore writes to paths the caller's policy does not allow.
//!
//! notify can report the same file many times, so the worker collects paths for a short
//! debounce window and acts on each path once. It asks the policy per path, never about
//! event order, and it never touches a path the policy allows.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher};
use sentinel_core::{Event, TaskId};
use tokio::sync::mpsc::UnboundedSender;

use crate::git::Git;
use crate::restore::{RestoreOutcome, restore_path};
use crate::{Result, TreeError};

/// What the caller's reservation table says about one repo-relative path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteVerdict {
    Allowed,
    Blocked { holder: Option<TaskId> },
}

/// Decides whether a task may write a path. `path` is relative to the repo root and uses `/`.
pub trait WritePolicy: Send + Sync {
    fn check(&self, path: &str) -> WriteVerdict;
}

#[derive(Debug, Clone)]
pub struct GuardOptions {
    pub debounce: Duration,
    pub ignore_prefixes: Vec<String>,
}

impl Default for GuardOptions {
    fn default() -> Self {
        Self {
            debounce: Duration::from_millis(100),
            ignore_prefixes: vec![".git/".to_owned(), ".sentinel/".to_owned()],
        }
    }
}

/// Stops the guard thread when dropped.
pub struct GuardHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for GuardHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Start the filesystem watcher. The guard runs on its own thread, so no tokio runtime is
/// needed here; the event sender is the synchronous handle of an unbounded channel.
pub fn start_guard(
    git: Git,
    policy: Arc<dyn WritePolicy>,
    events: UnboundedSender<Event>,
    options: GuardOptions,
) -> Result<GuardHandle> {
    let root = git
        .root()
        .canonicalize()
        .unwrap_or_else(|_| git.root().to_path_buf());
    let (path_sender, incoming) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |change: notify::Result<notify::Event>| {
        forward_paths(change, &path_sender);
    })
    .map_err(notify_error)?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(notify_error)?;
    let stop = Arc::new(AtomicBool::new(false));
    let guard = Guard {
        root,
        git,
        policy,
        events,
        options,
        incoming,
        stop: Arc::clone(&stop),
        ignored: HashMap::new(),
    };
    let thread = thread::Builder::new()
        .name("sentinel-write-guard".to_owned())
        .spawn(move || {
            let _watcher = watcher;
            guard.run();
        })
        .map_err(TreeError::Io)?;
    Ok(GuardHandle {
        stop,
        thread: Some(thread),
    })
}

fn forward_paths(change: notify::Result<notify::Event>, sender: &Sender<PathBuf>) {
    let Ok(change) = change else {
        return;
    };
    for path in change.paths {
        if sender.send(path).is_err() {
            return;
        }
    }
}

fn notify_error(error: notify::Error) -> TreeError {
    TreeError::Io(std::io::Error::other(error))
}

struct Guard {
    root: PathBuf,
    git: Git,
    policy: Arc<dyn WritePolicy>,
    events: UnboundedSender<Event>,
    options: GuardOptions,
    incoming: Receiver<PathBuf>,
    stop: Arc<AtomicBool>,
    ignored: HashMap<String, bool>,
}

impl Guard {
    fn run(mut self) {
        while !self.stop.load(Ordering::SeqCst) {
            match self.incoming.recv_timeout(self.options.debounce) {
                Ok(path) => self.handle_batch(path),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    /// Collect one debounce window of paths, then act on each distinct path once.
    fn handle_batch(&mut self, first: PathBuf) {
        let mut batch = vec![first];
        let deadline = Instant::now() + self.options.debounce;
        while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
            match self.incoming.recv_timeout(remaining) {
                Ok(path) => batch.push(path),
                Err(_) => break,
            }
        }
        let mut seen = HashSet::new();
        for path in batch {
            let Some(relative) = self.accept(&path) else {
                continue;
            };
            if seen.insert(relative.clone()) {
                self.handle(&relative);
            }
        }
    }

    /// Turn an absolute event path into a repo-relative path worth checking.
    fn accept(&mut self, path: &Path) -> Option<String> {
        if path.is_dir() {
            return None;
        }
        let relative = relative_path(&self.root, path)?;
        if self.ignored_prefix(&relative) || self.git_ignores(&relative) {
            return None;
        }
        Some(relative)
    }

    fn ignored_prefix(&self, path: &str) -> bool {
        self.options
            .ignore_prefixes
            .iter()
            .any(|prefix| path.starts_with(prefix))
    }

    fn git_ignores(&mut self, path: &str) -> bool {
        if let Some(known) = self.ignored.get(path) {
            return *known;
        }
        let ignored = self.git.run(&["check-ignore", "-q", "--", path]).is_ok();
        self.ignored.insert(path.to_owned(), ignored);
        ignored
    }

    fn handle(&mut self, path: &str) {
        let WriteVerdict::Blocked { holder } = self.policy.check(path) else {
            return;
        };
        let outcome = restore_path(&self.git, path, "HEAD");
        let restored = match outcome {
            Ok(RestoreOutcome::Rewritten | RestoreOutcome::Removed) => true,
            Ok(RestoreOutcome::Unchanged) => return,
            Err(_) => false,
        };
        let _ = self.events.send(Event::WriteBlocked {
            task: holder,
            path: path.to_owned(),
            restored,
        });
    }
}

fn relative_path(root: &Path, path: &Path) -> Option<String> {
    let rest = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in rest.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir => {
                return None;
            }
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::relative_path;

    #[test]
    fn relative_path_uses_forward_slashes_and_rejects_outsiders() {
        let root = Path::new("/repo");
        assert_eq!(
            relative_path(root, Path::new("/repo/src/a.rs")).as_deref(),
            Some("src/a.rs")
        );
        assert_eq!(relative_path(root, Path::new("/elsewhere/a.rs")), None);
        assert_eq!(relative_path(root, root), None);
    }
}
