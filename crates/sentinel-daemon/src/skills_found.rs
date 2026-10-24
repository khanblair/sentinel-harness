//! Skills that other tools keep in their own folders, offered for import (`docs/product-scope.md`,
//! F24). An import copies the folder into Sentinel's scope and never writes to the original.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use sentinel_core::skills::{FoundSkill, ImportSkill, SKILL_FOLDER_MAX, SkillInfo, SkillMode};

use crate::brain::require_pc;
use crate::controls::Caller;
use crate::engine::{Engine, EngineError, Result};
use crate::skills::{
    FolderFile, folder_files, folder_names, load_skill, parse_skill_md, valid_name,
};

/// Folder names that are never scanned, the plugin cache of another tool.
const SKIPPED_FOLDER: &str = "plugins";

/// One folder that holds skills, and the label the list shows for it.
struct Source {
    label: String,
    root: PathBuf,
}

impl Engine {
    /// Every skill in a scanned folder. `already_imported` is true when Sentinel has the name.
    pub fn found_skills(&self) -> Result<Vec<FoundSkill>> {
        let have: BTreeSet<String> = self.list_skills()?.into_iter().map(|s| s.name).collect();
        let mut found = Vec::new();
        for source in self.sources() {
            for name in skill_names(&source.root) {
                let folder = source.root.join(&name);
                found.push(FoundSkill {
                    already_imported: have.contains(&name),
                    description: lenient_description(&folder),
                    source: source.label.clone(),
                    path: folder.to_string_lossy().into_owned(),
                    name,
                });
            }
        }
        Ok(found)
    }

    /// Copy one found skill into a scope and turn its switch to Auto. Computer only.
    pub fn import_skill(&self, request: ImportSkill, caller: &Caller) -> Result<SkillInfo> {
        require_pc(caller)?;
        let ImportSkill { path, scope } = request;
        let folder = self.scanned_folder(&path)?;
        let name = folder
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !valid_name(&name) {
            return Err(EngineError::Invalid(format!(
                "The folder {name} is not a skill name."
            )));
        }
        let target = self.skill_dir(scope, &name);
        if target.exists() {
            return Err(EngineError::Conflict(format!(
                "A skill named {name} already exists in this scope."
            )));
        }
        let files = folder_files(&folder)?;
        let bytes: u64 = files.iter().map(|file| file.bytes).sum();
        if bytes > SKILL_FOLDER_MAX {
            return Err(EngineError::Invalid(
                "The skill folder is over 200 KB.".to_owned(),
            ));
        }
        copy_or_clean(&folder, &target, &files)?;
        match load_skill(&target, &name, scope, SkillMode::Auto) {
            Ok(loaded) => {
                self.store().set_skill_mode(&name, SkillMode::Auto)?;
                Ok(loaded.info)
            }
            Err(error) => {
                // The copy is not a skill, so it must not stay behind. The original is untouched.
                let _ = fs::remove_dir_all(&target);
                Err(error)
            }
        }
    }

    /// The folder of a request, when it is one of the scanned skill folders. Anything else is
    /// refused, so the import cannot copy an arbitrary folder.
    fn scanned_folder(&self, path: &str) -> Result<PathBuf> {
        let refused =
            || EngineError::Refused("That skill is not in a folder Sentinel scans.".to_owned());
        let requested = fs::canonicalize(path).map_err(|_| refused())?;
        for source in self.sources() {
            for name in skill_names(&source.root) {
                let folder = source.root.join(&name);
                if fs::canonicalize(&folder).is_ok_and(|canonical| canonical == requested) {
                    return Ok(folder);
                }
            }
        }
        Err(refused())
    }

    fn sources(&self) -> Vec<Source> {
        let repo = [
            ".claude/skills",
            ".agents/skills",
            ".agent/skills",
            ".codex/skills",
        ]
        .into_iter()
        .map(|rel| Source {
            label: rel.to_owned(),
            root: self.repo_path().join(rel),
        });
        let home = [".claude/skills", ".agents/skills", ".codex/skills"]
            .into_iter()
            .map(|rel| Source {
                label: format!("~/{rel}"),
                root: self.home().join(rel),
            });
        repo.chain(home).collect()
    }
}

/// The folders of a root that hold a readable `SKILL.md` and a valid name.
fn skill_names(root: &Path) -> Vec<String> {
    folder_names(root)
        .into_iter()
        .filter(|name| name != SKIPPED_FOLDER && valid_name(name))
        .filter(|name| root.join(name).join("SKILL.md").is_file())
        .collect()
}

/// The description from the front matter, or nothing. A found skill is listed even when its
/// front matter is bad, so the user can see it before importing.
fn lenient_description(folder: &Path) -> String {
    fs::read_to_string(folder.join("SKILL.md"))
        .ok()
        .and_then(|text| parse_skill_md(&text))
        .map(|parsed| parsed.description)
        .unwrap_or_default()
}

/// Copy the files of a folder. A copy that fails leaves nothing behind.
fn copy_or_clean(src: &Path, dst: &Path, files: &[FolderFile]) -> Result<()> {
    match copy_files(src, dst, files) {
        Ok(()) => Ok(()),
        Err(error) => {
            // The copy is partial, so remove it. The original is never touched.
            let _ = fs::remove_dir_all(dst);
            Err(error)
        }
    }
}

fn copy_files(src: &Path, dst: &Path, files: &[FolderFile]) -> Result<()> {
    fs::create_dir_all(dst)?;
    for file in files {
        let target = dst.join(&file.path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src.join(&file.path), target)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(root: &Path, name: &str) {
        let folder = root.join(name);
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Found one\n---\nBody\n"),
        )
        .unwrap();
    }

    #[test]
    fn a_plugin_folder_and_a_folder_without_skill_md_are_not_found() {
        let dir = tempfile::tempdir().unwrap();
        write_skill(dir.path(), "review");
        write_skill(dir.path(), SKIPPED_FOLDER);
        fs::create_dir_all(dir.path().join("empty")).unwrap();
        assert_eq!(skill_names(dir.path()), ["review"]);
    }
}
