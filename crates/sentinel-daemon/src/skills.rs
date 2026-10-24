//! Skills on disk: a folder with a `SKILL.md` per skill, in the project or in the personal folder
//! (`docs/product-scope.md`, F24). The switch of each skill lives in the store, not in the file.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use sentinel_core::skills::{
    DESCRIPTION_MAX, EditSkill, NAME_MAX, NewSkill, SKILL_FILE_MAX, SKILL_FOLDER_MAX, SkillDetail,
    SkillInfo, SkillMode, SkillScope,
};

use crate::brain::require_pc;
use crate::controls::Caller;
use crate::engine::{Engine, EngineError, Result};

/// A skill as read from its folder: the list row and the markdown after the front matter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoadedSkill {
    pub info: SkillInfo,
    pub body: String,
}

/// What a `SKILL.md` says, before the checks.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct SkillText {
    pub name: Option<String>,
    pub description: String,
    pub body: String,
}

/// One file of a skill folder, with its path relative to the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FolderFile {
    pub path: String,
    pub bytes: u64,
}

impl Engine {
    /// Every skill of both scopes, sorted by name. A project skill wins over a personal one.
    pub fn list_skills(&self) -> Result<Vec<SkillInfo>> {
        Ok(self.load_skills()?.into_iter().map(|s| s.info).collect())
    }

    /// The skill with this name. The project copy wins over the personal one.
    pub fn skill_detail(&self, name: &str) -> Result<SkillDetail> {
        let scope = [SkillScope::Project, SkillScope::Personal]
            .into_iter()
            .find(|scope| self.skill_dir(*scope, name).is_dir())
            .ok_or_else(|| EngineError::NotFound(format!("skill {name}")))?;
        self.skill_detail_in(scope, name)
    }

    /// Write a new skill folder in one scope. Computer only.
    pub fn create_skill(&self, new: NewSkill, caller: &Caller) -> Result<SkillDetail> {
        require_pc(caller)?;
        check_fields(&new.name, &new.description)?;
        let text = render_skill_md(&new.name, &new.description, &new.body)?;
        let dir = self.skill_dir(new.scope, &new.name);
        if dir.exists() {
            return Err(EngineError::Conflict(format!(
                "A skill named {} already exists here.",
                new.name
            )));
        }
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("SKILL.md"), text)?;
        // A switch left behind by a skill removed outside Sentinel must not apply to the new one.
        self.store().delete_skill_mode(&new.name)?;
        self.skill_detail_in(new.scope, &new.name)
    }

    /// Change a skill's switch, or its text. A phone may only move a switch toward Off.
    pub fn edit_skill(&self, name: &str, edit: EditSkill, caller: &Caller) -> Result<SkillDetail> {
        let current = self.skill_detail(name)?;
        let text_edit = edit.description.is_some() || edit.body.is_some();
        if text_edit {
            require_pc(caller)?;
        }
        let phone_turns_up = edit
            .mode
            .is_some_and(|target| !caller.from_pc && !current.info.mode.phone_may_set(target));
        if phone_turns_up {
            return Err(EngineError::Refused(
                "A phone may only turn a skill off. Turn it up at the computer.".to_owned(),
            ));
        }
        if text_edit {
            self.write_skill_text(&current, edit.description, edit.body)?;
        }
        if let Some(target) = edit.mode {
            self.store().set_skill_mode(name, target)?;
        }
        self.skill_detail_in(current.info.scope, name)
    }

    /// Remove a skill folder and, when no copy is left, its switch. Computer only.
    pub fn delete_skill(&self, name: &str, caller: &Caller) -> Result<()> {
        require_pc(caller)?;
        let current = self.skill_detail(name)?;
        fs::remove_dir_all(self.skill_dir(current.info.scope, name))?;
        let copy_left = [SkillScope::Project, SkillScope::Personal]
            .into_iter()
            .any(|scope| self.skill_dir(scope, name).is_dir());
        if !copy_left {
            self.store().delete_skill_mode(name)?;
        }
        Ok(())
    }

    /// Both scopes, project first. A bad folder is logged and skipped, never fatal.
    pub(crate) fn load_skills(&self) -> Result<Vec<LoadedSkill>> {
        let modes = self.store().get_skill_modes()?;
        let mut by_name: BTreeMap<String, LoadedSkill> = BTreeMap::new();
        for scope in [SkillScope::Project, SkillScope::Personal] {
            for name in folder_names(&self.scope_dir(scope)) {
                let mode = modes.get(&name).copied().unwrap_or_default();
                match load_skill(&self.skill_dir(scope, &name), &name, scope, mode) {
                    Ok(skill) => {
                        by_name.entry(name).or_insert(skill);
                    }
                    Err(error) => eprintln!("sentinel: the skill {name} was skipped: {error}"),
                }
            }
        }
        Ok(by_name.into_values().collect())
    }

    /// The folder of a skill in one scope, which may not exist.
    pub(crate) fn skill_dir(&self, scope: SkillScope, name: &str) -> PathBuf {
        self.scope_dir(scope).join(name)
    }

    /// `.sentinel/skills` of the project, or of the home folder for the personal scope.
    pub(crate) fn scope_dir(&self, scope: SkillScope) -> PathBuf {
        match scope {
            SkillScope::Project => self.repo_path().join(".sentinel/skills"),
            SkillScope::Personal => self.home().join(".sentinel/skills"),
        }
    }

    fn skill_detail_in(&self, scope: SkillScope, name: &str) -> Result<SkillDetail> {
        let dir = self.skill_dir(scope, name);
        if !dir.is_dir() {
            return Err(EngineError::NotFound(format!("skill {name}")));
        }
        let mode = self
            .store()
            .get_skill_modes()?
            .get(name)
            .copied()
            .unwrap_or_default();
        let loaded = load_skill(&dir, name, scope, mode)?;
        let files = folder_files(&dir)?
            .into_iter()
            .map(|file| file.path)
            .filter(|path| path != "SKILL.md")
            .collect();
        Ok(SkillDetail {
            info: loaded.info,
            body: loaded.body,
            files,
        })
    }

    /// Rewrite `SKILL.md` with the new description or body. Missing fields keep their text.
    fn write_skill_text(
        &self,
        current: &SkillDetail,
        description: Option<String>,
        body: Option<String>,
    ) -> Result<()> {
        let description = description.unwrap_or_else(|| current.info.description.clone());
        let body = body.unwrap_or_else(|| current.body.clone());
        let text = render_skill_md(&current.info.name, &description, &body)?;
        let path = self
            .skill_dir(current.info.scope, &current.info.name)
            .join("SKILL.md");
        fs::write(path, text)?;
        Ok(())
    }
}

/// The name and description rules. Both are checked before anything is written.
pub(crate) fn check_fields(name: &str, description: &str) -> Result<()> {
    if !valid_name(name) {
        return Err(EngineError::Invalid(format!(
            "A skill name uses lowercase letters, digits, and dashes, up to {NAME_MAX} characters."
        )));
    }
    let trimmed = description.trim();
    if trimmed.is_empty() || trimmed.contains('\n') || trimmed.chars().count() > DESCRIPTION_MAX {
        return Err(EngineError::Invalid(format!(
            "A description is one line, from 1 to {DESCRIPTION_MAX} characters."
        )));
    }
    Ok(())
}

fn valid_name(name: &str) -> bool {
    (1..=NAME_MAX).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// The `SKILL.md` text of a skill. The description is quoted, so it reads back as one value.
pub(crate) fn render_skill_md(name: &str, description: &str, body: &str) -> Result<String> {
    let text = format!(
        "---\nname: {name}\ndescription: \"{}\"\n---\n\n{}\n",
        escape(description.trim()),
        body.trim()
    );
    if text.len() as u64 > SKILL_FILE_MAX {
        return Err(EngineError::Invalid(
            "SKILL.md is over 20 KB. Make the skill shorter.".to_owned(),
        ));
    }
    Ok(text)
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Read a folder into a skill. Every check here is a reason to skip the folder in a list.
fn load_skill(dir: &Path, name: &str, scope: SkillScope, mode: SkillMode) -> Result<LoadedSkill> {
    if !valid_name(name) {
        return Err(EngineError::Invalid(format!(
            "The folder {name} is not a skill name."
        )));
    }
    let files = folder_files(dir)?;
    let bytes: u64 = files.iter().map(|file| file.bytes).sum();
    if bytes > SKILL_FOLDER_MAX {
        return Err(EngineError::Invalid(
            "The skill folder is over 200 KB.".to_owned(),
        ));
    }
    let text = read_skill_md(dir, &files)?;
    let parsed = parse_skill_md(&text).ok_or_else(|| {
        EngineError::Invalid(
            "SKILL.md needs a front matter block with name and description.".to_owned(),
        )
    })?;
    if parsed.name.as_deref().is_some_and(|found| found != name) {
        return Err(EngineError::Invalid(
            "The name in SKILL.md must match the folder.".to_owned(),
        ));
    }
    check_fields(name, &parsed.description)?;
    let info = SkillInfo {
        name: name.to_owned(),
        description: parsed.description.trim().to_owned(),
        scope,
        mode,
        bytes,
    };
    Ok(LoadedSkill {
        info,
        body: parsed.body,
    })
}

fn read_skill_md(dir: &Path, files: &[FolderFile]) -> Result<String> {
    let too_big = files
        .iter()
        .any(|file| file.path == "SKILL.md" && file.bytes > SKILL_FILE_MAX);
    if too_big {
        return Err(EngineError::Invalid("SKILL.md is over 20 KB.".to_owned()));
    }
    fs::read_to_string(dir.join("SKILL.md"))
        .map_err(|_| EngineError::Invalid("SKILL.md is missing or is not text.".to_owned()))
}

/// Split a `SKILL.md` into its front matter values and its body. Other keys are ignored.
/// A missing `---` block gives `None`.
pub(crate) fn parse_skill_md(text: &str) -> Option<SkillText> {
    let text = text.replace("\r\n", "\n");
    let rest = text.strip_prefix("---\n")?;
    let (front, body) = rest.split_once("\n---\n")?;
    let mut parsed = SkillText {
        body: body.trim().to_owned(),
        ..SkillText::default()
    };
    for line in front.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        match key.trim() {
            "name" => parsed.name = Some(unquote(value.trim())),
            "description" => parsed.description = unquote(value.trim()),
            _ => {}
        }
    }
    Some(parsed)
}

/// Remove the quotes of a value and the escapes inside them.
fn unquote(raw: &str) -> String {
    let Some(inner) = raw
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    else {
        return raw.to_owned();
    };
    let mut out = String::with_capacity(inner.len());
    let mut escaped = false;
    for c in inner.chars() {
        if escaped {
            out.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else {
            out.push(c);
        }
    }
    out
}

/// The names of the immediate subfolders of a skill root, sorted. A missing root has none.
pub(crate) fn folder_names(root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Every file under a folder, skipping symlinks and `.git`. Paths are relative to the folder.
pub(crate) fn folder_files(dir: &Path) -> Result<Vec<FolderFile>> {
    let mut found = Vec::new();
    walk(dir, dir, &mut found)?;
    found.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(found)
}

fn walk(root: &Path, dir: &Path, found: &mut Vec<FolderFile>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() || entry.file_name() == ".git" {
            continue;
        }
        let path = entry.path();
        if kind.is_dir() {
            walk(root, &path, found)?;
            continue;
        }
        let relative = path.strip_prefix(root).unwrap_or(&path);
        found.push(FolderFile {
            path: relative.to_string_lossy().replace('\\', "/"),
            bytes: entry.metadata()?.len(),
        });
    }
    Ok(())
}
