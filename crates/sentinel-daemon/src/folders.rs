//! Folder browsing for a phone, repository checks, and the starter backlog.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use sentinel_core::api::{
    CheckRow, CheckStatus, FolderEntry, RepoCheckResponse, RepoTree, TreeFolder,
};

const STARTER_BACKLOG: &str = "# Backlog

## T-01 Add a greeting command
Done when the command prints a greeting to the terminal.

## T-02 Cover the greeting with a test
Done when a test runs the command and checks the printed text.

## T-03 Document how to run the project
Done when the README explains the setup and the run command.
";

#[derive(Debug, thiserror::Error)]
pub enum FolderError {
    #[error("path is outside the home folder")]
    OutsideHome,
    #[error("path may not contain ..")]
    ParentTraversal,
    #[error("path is outside the repository")]
    OutsideRepo,
    #[error("not a folder in this repository: {0}")]
    NotFolder(String),
    #[error("file already exists: {0}")]
    Exists(PathBuf),
    #[error("cannot use folder: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, FolderError>;

pub fn list_folders(home: &Path, path: &Path) -> Result<Vec<FolderEntry>> {
    let home = fs::canonicalize(home)?;
    let folder = resolve_inside(&home, path)?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(&folder)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(resolved) = fs::canonicalize(entry.path()) else {
            // A broken link is not a folder the user can open.
            continue;
        };
        if resolved.is_dir() && resolved.starts_with(&home) {
            entries.push(FolderEntry {
                name,
                path: resolved.to_string_lossy().into_owned(),
            });
        }
    }
    entries.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(entries)
}

pub fn within_home(home: &Path, path: &Path) -> bool {
    if has_parent_dir(path) {
        return false;
    }
    let Ok(home) = fs::canonicalize(home) else {
        return false;
    };
    let Ok(path) = fs::canonicalize(path) else {
        return false;
    };
    path.starts_with(home)
}

fn resolve_inside(home: &Path, path: &Path) -> Result<PathBuf> {
    if has_parent_dir(path) {
        return Err(FolderError::ParentTraversal);
    }
    let folder = fs::canonicalize(path)?;
    if !folder.starts_with(home) {
        return Err(FolderError::OutsideHome);
    }
    Ok(folder)
}

fn has_parent_dir(path: &Path) -> bool {
    path.components()
        .any(|component| component == Component::ParentDir)
}

/// Folders the browser never lists: build output and dependencies.
const SKIPPED_FOLDERS: [&str; 4] = ["target", "node_modules", "dist", "build"];
/// The most subfolders one listing sends.
pub const TREE_FOLDER_LIMIT: usize = 500;

/// One folder of the repository and the subfolders a browser can open. `relative` is empty for
/// the root. Nothing outside `repo` is read.
pub fn repo_tree(repo: &Path, relative: &str) -> Result<RepoTree> {
    let root = fs::canonicalize(repo)?;
    let path = tree_path(relative)?;
    let target =
        fs::canonicalize(root.join(&path)).map_err(|_| FolderError::NotFolder(path.clone()))?;
    if !target.starts_with(&root) {
        return Err(FolderError::OutsideRepo);
    }
    if !target.is_dir() {
        return Err(FolderError::NotFolder(path));
    }
    let children = visible_children(&target, &root)?;
    let files_here = count_u32(children.iter().filter(|child| !child.is_dir).count());
    let folders = listed_folders(&path, &children, &root);
    Ok(RepoTree {
        parent: parent_of(&path),
        path,
        folders,
        files_here,
    })
}

/// A plain relative path with no `..`, no absolute parts, and no hidden names.
fn tree_path(relative: &str) -> Result<String> {
    let mut parts = Vec::new();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(name) => {
                let name = name
                    .to_str()
                    .filter(|name| !name.starts_with('.'))
                    .ok_or_else(|| FolderError::NotFolder(relative.to_owned()))?;
                parts.push(name);
            }
            Component::CurDir => {}
            Component::ParentDir => return Err(FolderError::ParentTraversal),
            Component::RootDir | Component::Prefix(_) => return Err(FolderError::OutsideRepo),
        }
    }
    Ok(parts.join("/"))
}

/// One entry of a folder that a listing may show. `resolved` is the real path after links.
struct Child {
    name: String,
    resolved: PathBuf,
    is_dir: bool,
}

/// The files and folders of `dir` that the browser may show. Hidden names and links that leave
/// `root` are left out.
fn visible_children(dir: &Path, root: &Path) -> Result<Vec<Child>> {
    let mut children = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let Ok(resolved) = fs::canonicalize(entry.path()) else {
            // A broken link is not something the user can open.
            continue;
        };
        if !resolved.starts_with(root) || !(resolved.is_dir() || resolved.is_file()) {
            continue;
        }
        let is_dir = resolved.is_dir();
        children.push(Child {
            name,
            resolved,
            is_dir,
        });
    }
    Ok(children)
}

/// The subfolders of a listing: sorted by lower-case name, capped, each with its direct counts.
fn listed_folders(path: &str, children: &[Child], root: &Path) -> Vec<TreeFolder> {
    let mut folders: Vec<&Child> = children.iter().filter(|child| is_listed(child)).collect();
    folders.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.name.cmp(&right.name))
    });
    folders.truncate(TREE_FOLDER_LIMIT);
    folders
        .into_iter()
        .map(|child| {
            let (files, subfolders) = direct_counts(&child.resolved, root);
            TreeFolder {
                name: child.name.clone(),
                path: path_of(path, &child.name),
                files,
                folders: subfolders,
            }
        })
        .collect()
}

fn is_listed(child: &Child) -> bool {
    child.is_dir && !SKIPPED_FOLDERS.contains(&child.name.as_str())
}

/// Direct files and direct listed subfolders of one folder. A folder that cannot be read counts
/// as empty, so the listing still shows it.
fn direct_counts(dir: &Path, root: &Path) -> (u32, u32) {
    let Ok(children) = visible_children(dir, root) else {
        return (0, 0);
    };
    let files = children.iter().filter(|child| !child.is_dir).count();
    let folders = children.iter().filter(|child| is_listed(child)).count();
    (count_u32(files), count_u32(folders))
}

fn path_of(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

fn parent_of(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    Some(
        path.rsplit_once('/')
            .map_or_else(String::new, |(parent, _)| parent.to_owned()),
    )
}

fn count_u32(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

pub fn check_repo(path: &Path) -> RepoCheckResponse {
    let shown = path.to_string_lossy().into_owned();
    if !path.is_dir() {
        return RepoCheckResponse {
            path: shown,
            rows: vec![CheckRow {
                name: "Folder".to_owned(),
                status: CheckStatus::Invalid,
                detail: "Not a folder".to_owned(),
                fix: None,
            }],
            usable: false,
        };
    }
    let git = git_row(path);
    let backlog = backlog_row(path);
    let clean = clean_row(path, git.status == CheckStatus::Ok);
    let usable = [git.status, backlog.status, clean.status]
        .iter()
        .all(|status| *status == CheckStatus::Ok);
    RepoCheckResponse {
        path: shown,
        rows: vec![git, backlog, clean],
        usable,
    }
}

fn git_row(path: &Path) -> CheckRow {
    if path.join(".git").exists() {
        CheckRow {
            name: "Git repository".to_owned(),
            status: CheckStatus::Ok,
            detail: "This folder is a git repository".to_owned(),
            fix: None,
        }
    } else {
        CheckRow {
            name: "Git repository".to_owned(),
            status: CheckStatus::NeedsAction,
            detail: "No .git entry in this folder".to_owned(),
            fix: None,
        }
    }
}

fn backlog_row(path: &Path) -> CheckRow {
    if path.join("backlog.md").is_file() {
        CheckRow {
            name: "backlog.md".to_owned(),
            status: CheckStatus::Ok,
            detail: "backlog.md is present".to_owned(),
            fix: None,
        }
    } else {
        CheckRow {
            name: "backlog.md".to_owned(),
            status: CheckStatus::NeedsAction,
            detail: "No backlog.md in this folder".to_owned(),
            fix: Some("Create a starter backlog".to_owned()),
        }
    }
}

fn clean_row(path: &Path, has_git: bool) -> CheckRow {
    if !has_git {
        return CheckRow {
            name: "Clean working tree".to_owned(),
            status: CheckStatus::NeedsAction,
            detail: "Skipped because this folder is not a git repository".to_owned(),
            fix: None,
        };
    }
    match git_status(path) {
        Ok(true) => CheckRow {
            name: "Clean working tree".to_owned(),
            status: CheckStatus::Ok,
            detail: "No uncommitted changes".to_owned(),
            fix: None,
        },
        Ok(false) => CheckRow {
            name: "Clean working tree".to_owned(),
            status: CheckStatus::NeedsAction,
            detail: "Uncommitted changes in this folder".to_owned(),
            fix: None,
        },
        Err(detail) => CheckRow {
            name: "Clean working tree".to_owned(),
            status: CheckStatus::NeedsAction,
            detail,
            fix: None,
        },
    }
}

fn git_status(path: &Path) -> std::result::Result<bool, String> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output()
        .map_err(|error| format!("Cannot run git status: {error}"))?;
    if !output.status.success() {
        return Err("git status failed".to_owned());
    }
    Ok(output.stdout.iter().all(u8::is_ascii_whitespace))
}

pub fn create_starter_backlog(path: &Path) -> Result<()> {
    let target = path.join("backlog.md");
    if target.exists() {
        return Err(FolderError::Exists(target));
    }
    fs::write(target, STARTER_BACKLOG)?;
    Ok(())
}

/// Blocking native dialog. The caller runs this inside `spawn_blocking`.
/// Keep Sentinel's own folder out of the repository's `git status` without touching any tracked
/// file: add it to `.git/info/exclude`, which is local to this clone.
pub fn exclude_sentinel_folder(repo: &Path) {
    let exclude = repo.join(".git/info/exclude");
    let known = fs::read_to_string(&exclude).unwrap_or_default();
    if known.lines().any(|line| line.trim() == "/.sentinel/") {
        return;
    }
    if let Some(parent) = exclude.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut text = known;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("/.sentinel/\n");
    // A repository we cannot write to is reported later, when git itself fails.
    let _ = fs::write(&exclude, text);
}

/// A plain message box, for a program that has no terminal to print to.
pub fn alert(text: &str) {
    rfd::MessageDialog::new()
        .set_title("Sentinel")
        .set_description(text)
        .set_level(rfd::MessageLevel::Warning)
        .show();
}

pub fn pick_native(start: Option<&Path>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(start) = start {
        dialog = dialog.set_directory(start);
    }
    dialog.pick_folder()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use sentinel_core::api::RepoTree;

    use super::{FolderError, TREE_FOLDER_LIMIT, parent_of, repo_tree};

    fn repo_with(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for file in files {
            let path = dir.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "x").unwrap();
        }
        dir
    }

    fn names(tree: &RepoTree) -> Vec<&str> {
        tree.folders
            .iter()
            .map(|folder| folder.name.as_str())
            .collect()
    }

    #[test]
    fn repo_tree_lists_direct_folders_with_their_direct_counts() {
        let dir = repo_with(&[
            "src/a.rs",
            "src/lib.rs",
            "src/fees/x.rs",
            "src/fees/y.rs",
            "src/fees/deep/z.rs",
            "tests/t.rs",
            "README.md",
        ]);
        let root = repo_tree(dir.path(), "").unwrap();
        assert_eq!(names(&root), vec!["src", "tests"]);
        assert_eq!(root.files_here, 1);
        assert_eq!(root.parent, None);
        let src = &root.folders[0];
        assert_eq!((src.path.as_str(), src.files, src.folders), ("src", 2, 1));
        let inside = repo_tree(dir.path(), "src").unwrap();
        assert_eq!(inside.parent.as_deref(), Some(""));
        assert_eq!(inside.files_here, 2);
        let fees = &inside.folders[0];
        assert_eq!(
            (fees.path.as_str(), fees.files, fees.folders),
            ("src/fees", 2, 1)
        );
        let nested = repo_tree(dir.path(), "src/fees").unwrap();
        assert_eq!(nested.parent.as_deref(), Some("src"));
    }

    #[test]
    fn repo_tree_rejects_parent_steps_and_absolute_paths() {
        let dir = repo_with(&["src/a.rs"]);
        assert!(matches!(
            repo_tree(dir.path(), "../outside"),
            Err(FolderError::ParentTraversal)
        ));
        assert!(matches!(
            repo_tree(dir.path(), "src/../../x"),
            Err(FolderError::ParentTraversal)
        ));
        assert!(matches!(
            repo_tree(dir.path(), "/etc"),
            Err(FolderError::OutsideRepo)
        ));
    }

    #[test]
    fn repo_tree_refuses_a_file_and_a_missing_folder() {
        let dir = repo_with(&["src/a.rs"]);
        assert!(matches!(
            repo_tree(dir.path(), "src/a.rs"),
            Err(FolderError::NotFolder(_))
        ));
        assert!(matches!(
            repo_tree(dir.path(), "nope"),
            Err(FolderError::NotFolder(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn repo_tree_skips_a_link_that_leaves_the_repository() {
        let dir = repo_with(&["src/a.rs"]);
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir(outside.path().join("secret")).unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("escape")).unwrap();
        std::os::unix::fs::symlink(dir.path().join("src"), dir.path().join("alias")).unwrap();
        let root = repo_tree(dir.path(), "").unwrap();
        assert_eq!(names(&root), vec!["alias", "src"]);
    }

    #[test]
    fn repo_tree_skips_hidden_and_build_folders() {
        let dir = repo_with(&[
            "src/a.rs",
            ".git/HEAD",
            ".sentinel/x",
            ".hidden/y",
            "target/t",
            "node_modules/n",
            "dist/d",
            "build/b",
        ]);
        let root = repo_tree(dir.path(), "").unwrap();
        assert_eq!(names(&root), vec!["src"]);
        assert!(matches!(
            repo_tree(dir.path(), ".git"),
            Err(FolderError::NotFolder(_))
        ));
    }

    #[test]
    fn repo_tree_sorts_folders_by_lower_case_name() {
        let dir = repo_with(&["beta/x", "Alpha/x", "gamma/x"]);
        let root = repo_tree(dir.path(), "").unwrap();
        assert_eq!(names(&root), vec!["Alpha", "beta", "gamma"]);
    }

    #[test]
    fn repo_tree_caps_a_listing_at_the_folder_limit() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..=TREE_FOLDER_LIMIT {
            fs::create_dir(dir.path().join(format!("pkg-{index:04}"))).unwrap();
        }
        let root = repo_tree(dir.path(), "").unwrap();
        assert_eq!(root.folders.len(), TREE_FOLDER_LIMIT);
    }

    #[test]
    fn parent_of_a_nested_folder_is_its_folder_and_the_root_has_none() {
        assert_eq!(parent_of("src/fees"), Some("src".to_owned()));
        assert_eq!(parent_of("src"), Some(String::new()));
        assert_eq!(parent_of(""), None);
    }
}
