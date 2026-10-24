//! Skills: folders with a `SKILL.md` that teach agents one thing (`docs/product-scope.md`, F24).
//! These are the shapes the daemon sends and the UI reads.

use serde::{Deserialize, Serialize};

/// Longest skill name, in characters.
pub const NAME_MAX: usize = 48;
/// Longest description, in characters.
pub const DESCRIPTION_MAX: usize = 300;
/// Largest `SKILL.md`, in bytes.
pub const SKILL_FILE_MAX: u64 = 20 * 1024;
/// Largest skill folder, in bytes.
pub const SKILL_FOLDER_MAX: u64 = 200 * 1024;

/// Whether a skill goes into task prompts. Off never, Auto when the words match, Always every time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillMode {
    Off,
    #[default]
    Auto,
    Always,
}

impl SkillMode {
    /// A phone may only move a switch toward Off: Always to Auto or Off, Auto to Off.
    pub fn phone_may_set(self, target: SkillMode) -> bool {
        rank(target) <= rank(self)
    }
}

fn rank(mode: SkillMode) -> u8 {
    match mode {
        SkillMode::Off => 0,
        SkillMode::Auto => 1,
        SkillMode::Always => 2,
    }
}

/// Where a skill lives. A project skill wins over a personal one with the same name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillScope {
    Project,
    Personal,
}

/// A skill in the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub scope: SkillScope,
    pub mode: SkillMode,
    /// Size of the whole folder.
    pub bytes: u64,
}

/// One skill with its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDetail {
    pub info: SkillInfo,
    /// The markdown after the front matter.
    pub body: String,
    /// The other files in the folder, relative to it.
    pub files: Vec<String>,
}

/// `POST /api/skills`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewSkill {
    pub name: String,
    pub description: String,
    pub body: String,
    pub scope: SkillScope,
}

/// `PUT /api/skills/:name`. A missing field stays as it is.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EditSkill {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub mode: Option<SkillMode>,
}

/// A skill in another tool's folder, offered for import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundSkill {
    pub name: String,
    pub description: String,
    /// The folder it sits in, as a label such as `.claude/skills` or `~/.codex/skills`.
    pub source: String,
    /// The full path of the skill folder.
    pub path: String,
    /// True when Sentinel already has a skill of this name.
    pub already_imported: bool,
}

/// `POST /api/skills/import`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportSkill {
    pub path: String,
    pub scope: SkillScope,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_phone_moves_a_switch_only_toward_off() {
        assert!(SkillMode::Always.phone_may_set(SkillMode::Auto));
        assert!(SkillMode::Auto.phone_may_set(SkillMode::Off));
        assert!(!SkillMode::Off.phone_may_set(SkillMode::Auto));
        assert!(!SkillMode::Auto.phone_may_set(SkillMode::Always));
    }

    #[test]
    fn a_new_skill_is_auto() {
        assert_eq!(SkillMode::default(), SkillMode::Auto);
    }
}
