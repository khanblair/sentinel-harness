//! The skill tools. Agents list the skills of the project and of the home folder, and read one.
//! Both tools are read-only. The project copy of a name wins over the personal one.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::protocol::argument;

pub const TOOL_NAMES: [&str; 2] = ["skill_list", "skill_read"];
/// The most skill text one read returns, in characters.
const BODY_CHARS: usize = 8_000;
/// The longest skill name the tools accept.
const NAME_CHARS: usize = 48;

pub struct SkillTools {
    /// The project folder first, then the personal folder.
    roots: Vec<PathBuf>,
}

impl SkillTools {
    pub fn new(workdir: &Path, home: Option<&Path>) -> Self {
        let mut roots = vec![workdir.join(".sentinel/skills")];
        roots.extend(home.map(|home| home.join(".sentinel/skills")));
        Self { roots }
    }

    /// Run one skill tool. Errors come back as the text the model reads.
    pub fn call(&self, name: &str, args: &Value) -> Result<String, String> {
        match name {
            "skill_list" => Ok(self.list()),
            "skill_read" => self.read(argument(args, "name")?),
            other => Err(format!("Unknown tool: {other}.")),
        }
    }

    /// One line per skill: its name and its description.
    fn list(&self) -> String {
        let mut by_name: BTreeMap<String, String> = BTreeMap::new();
        for root in &self.roots {
            for name in folder_names(root) {
                if let Some(description) = description_of(&root.join(&name)) {
                    by_name.entry(name).or_insert(description);
                }
            }
        }
        if by_name.is_empty() {
            return "No skills.".to_owned();
        }
        by_name
            .iter()
            .map(|(name, description)| format!("- {name}: {description}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The body of one skill, without its front matter, cut to the limit.
    fn read(&self, name: &str) -> Result<String, String> {
        if !is_skill_name(name) {
            return Err("A skill name uses lowercase letters, digits, and dashes.".to_owned());
        }
        for root in &self.roots {
            let Ok(text) = fs::read_to_string(root.join(name).join("SKILL.md")) else {
                continue;
            };
            let (_, body) = split_skill(&text);
            return Ok(body.chars().take(BODY_CHARS).collect());
        }
        Err(format!("No skill named {name}."))
    }
}

/// The name only letters, digits, and dashes, so a name never leaves its skill folder.
fn is_skill_name(name: &str) -> bool {
    (1..=NAME_CHARS).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// The names of the subfolders of a root that hold a `SKILL.md`, sorted.
fn folder_names(root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| is_skill_name(name) && root.join(name).join("SKILL.md").is_file())
        .collect();
    names.sort();
    names
}

fn description_of(folder: &Path) -> Option<String> {
    let text = fs::read_to_string(folder.join("SKILL.md")).ok()?;
    let (description, _) = split_skill(&text);
    (!description.is_empty()).then_some(description)
}

/// The description and the body of a `SKILL.md`. A file without a front matter block has no
/// description, and its whole text is the body.
fn split_skill(text: &str) -> (String, String) {
    let text = text.replace("\r\n", "\n");
    let Some((front, body)) = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
    else {
        return (String::new(), text.trim().to_owned());
    };
    let description = front
        .lines()
        .find_map(|line| line.strip_prefix("description:"))
        .map(|value| unquote(value.trim()))
        .unwrap_or_default();
    (description, body.trim().to_owned())
}

/// Remove the quotes of a value and the escapes inside them, the way the daemon writes them.
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn write(root: &Path, name: &str, text: &str) {
        let folder = root.join(name);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("SKILL.md"), text).unwrap();
    }

    #[test]
    fn a_project_skill_wins_over_a_personal_one_with_the_same_name() {
        let project = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let text = "---\nname: review\ndescription: Project copy.\n---\nProject body\n";
        write(&project.path().join(".sentinel/skills"), "review", text);
        let personal = "---\ndescription: Personal copy.\n---\nPersonal body\n";
        write(&home.path().join(".sentinel/skills"), "review", personal);
        write(
            &home.path().join(".sentinel/skills"),
            "lint",
            "---\ndescription: Lints.\n---\n",
        );
        let tools = SkillTools::new(project.path(), Some(home.path()));
        let listed = tools.call("skill_list", &json!({})).unwrap();
        assert_eq!(listed, "- lint: Lints.\n- review: Project copy.");
        let read = tools
            .call("skill_read", &json!({"name": "review"}))
            .unwrap();
        assert_eq!(read, "Project body");
    }

    #[test]
    fn a_name_with_a_slash_is_refused_before_any_file_is_read() {
        let project = tempfile::tempdir().unwrap();
        let tools = SkillTools::new(project.path(), None);
        let result = tools.call("skill_read", &json!({"name": "../secret"}));
        assert!(result.is_err());
    }

    #[test]
    fn a_long_body_is_cut_to_the_limit() {
        let project = tempfile::tempdir().unwrap();
        let text = format!("---\ndescription: Long.\n---\n{}", "y".repeat(9_000));
        write(&project.path().join(".sentinel/skills"), "long", &text);
        let tools = SkillTools::new(project.path(), None);
        let read = tools.call("skill_read", &json!({"name": "long"})).unwrap();
        assert_eq!(read.chars().count(), BODY_CHARS);
    }
}
