//! The repository tree that the folder browser lists. `folders.rs` re-exports `repo_tree` and
//! `TREE_FOLDER_LIMIT`, so `folders::repo_tree` keeps working.

use std::fs;
use std::path::{Component, Path, PathBuf};

use sentinel_core::api::{RepoTree, TreeFolder};

use crate::folders::{FolderError, Result};

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

#[cfg(test)]
mod tests {
    use std::fs;

    use sentinel_core::api::RepoTree;

    use super::{TREE_FOLDER_LIMIT, parent_of, repo_tree};
    use crate::folders::FolderError;

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
