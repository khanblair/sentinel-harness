//! Which skills a task gets, and the text they add to its prompt. Always skills come first, then
//! the skills the user picked in the chat, then Auto skills whose words match the task.

use std::collections::BTreeSet;

use sentinel_core::chat::{Attachment, AttachmentKind};
use sentinel_core::skills::SkillMode;
use sentinel_core::{Event, Task};

use crate::engine::Engine;
use crate::skills::LoadedSkill;

/// The most Auto skills one task gets, however many words match.
const AUTO_LIMIT: usize = 3;
/// The prompt budget for skills: 1,500 tokens at 4 characters a token.
const SKILL_CHAR_CAP: usize = 1_500 * 4;
const HEADER: &str = "## Skills";
/// Words shorter than this never count as a match.
const MIN_WORD_CHARS: usize = 3;
const STOP_WORDS: [&str; 16] = [
    "the", "and", "for", "with", "that", "this", "from", "into", "when", "should", "must", "will",
    "not", "are", "all", "use",
];

impl Engine {
    /// The Brain memory and the skill text for one task, joined. Skills come first.
    pub(crate) fn task_notes(&self, task: &Task, attachments: &[Attachment]) -> String {
        let skills = self.skill_text(task, &hand_picked(attachments));
        let memory = self.brain_memory(task);
        match (skills.is_empty(), memory.is_empty()) {
            (_, true) => skills,
            (true, false) => memory,
            (false, false) => format!("{skills}\n\n{memory}"),
        }
    }

    /// The `## Skills` text for one task, or an empty string when none apply. The stream names
    /// the skills that were loaded.
    pub(crate) fn skill_text(&self, task: &Task, hand_picked: &[String]) -> String {
        let skills = match self.load_skills() {
            Ok(skills) => skills,
            Err(error) => {
                eprintln!("sentinel: the skills were skipped: {error}");
                return String::new();
            }
        };
        let chosen = fitting_blocks(&choose(&skills, task, hand_picked));
        if chosen.blocks.is_empty() {
            return String::new();
        }
        self.emit_quiet(&Event::Message {
            from: "sentinel".to_owned(),
            to: None,
            text: format!("Skills loaded: {}.", chosen.names.join(", ")),
        });
        format!("{HEADER}\n\n{}", chosen.blocks.join("\n\n"))
    }
}

/// The skill names the user picked in the chat for this run.
pub(crate) fn hand_picked(attachments: &[Attachment]) -> Vec<String> {
    attachments
        .iter()
        .filter(|attachment| attachment.kind == AttachmentKind::Skill)
        .map(|attachment| attachment.name.clone())
        .collect()
}

/// The skills for one task, in prompt order: Always, then picked, then the best Auto matches.
fn choose<'a>(
    skills: &'a [LoadedSkill],
    task: &Task,
    hand_picked: &[String],
) -> Vec<&'a LoadedSkill> {
    let task_words = task_words(task);
    let mut chosen: Vec<&LoadedSkill> = skills
        .iter()
        .filter(|skill| skill.info.mode == SkillMode::Always)
        .collect();
    chosen.extend(
        skills
            .iter()
            .filter(|skill| hand_picked.contains(&skill.info.name))
            .filter(|skill| skill.info.mode != SkillMode::Always),
    );
    let mut auto: Vec<(usize, &LoadedSkill)> = skills
        .iter()
        .filter(|skill| skill.info.mode == SkillMode::Auto)
        .filter(|skill| !hand_picked.contains(&skill.info.name))
        .map(|skill| (shared_words(skill, &task_words), skill))
        .filter(|(score, _)| *score > 0)
        .collect();
    auto.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.info.name.cmp(&b.1.info.name))
    });
    chosen.extend(auto.into_iter().take(AUTO_LIMIT).map(|(_, skill)| skill));
    chosen
}

fn task_words(task: &Task) -> BTreeSet<String> {
    let scope: Vec<&str> = task.scope.entries().collect();
    words(&format!("{} {} {}", task.title, task.body, scope.join(" ")))
}

fn shared_words(skill: &LoadedSkill, task_words: &BTreeSet<String>) -> usize {
    words(&format!("{} {}", skill.info.name, skill.info.description))
        .intersection(task_words)
        .count()
}

/// Lowercase words of letters and digits, without the short and stop words. Dots, dashes,
/// slashes, and underscores split words, so a file path gives its parts.
fn words(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|word| word.chars().count() >= MIN_WORD_CHARS)
        .filter(|word| !STOP_WORDS.contains(&word.as_str()))
        .collect()
}

/// The rendered skills that fit the budget, with their names. A skill that does not fit is
/// left out whole, and the next one is tried.
struct Blocks {
    names: Vec<String>,
    blocks: Vec<String>,
}

fn fitting_blocks(candidates: &[&LoadedSkill]) -> Blocks {
    let mut used = HEADER.len();
    let mut out = Blocks {
        names: Vec::new(),
        blocks: Vec::new(),
    };
    for skill in candidates {
        let block = skill_block(skill);
        let cost = block.len() + 2;
        if used + cost > SKILL_CHAR_CAP {
            continue;
        }
        used += cost;
        out.names.push(skill.info.name.clone());
        out.blocks.push(block);
    }
    out
}

fn skill_block(skill: &LoadedSkill) -> String {
    format!(
        "### {}\n\n*{}*\n\n{}",
        skill.info.name, skill.info.description, skill.body
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sentinel_core::skills::{EditSkill, NewSkill, SkillInfo, SkillScope};
    use sentinel_core::{Scope, TaskId};

    use super::*;
    use crate::cmd::SystemRunner;
    use crate::controls::Caller;
    use crate::engine::EngineOptions;
    use crate::secrets::MemoryStore;

    fn skill(name: &str, description: &str, mode: SkillMode, body: &str) -> LoadedSkill {
        LoadedSkill {
            info: SkillInfo {
                name: name.to_owned(),
                description: description.to_owned(),
                scope: SkillScope::Project,
                mode,
                bytes: body.len() as u64,
            },
            body: body.to_owned(),
        }
    }

    fn task(title: &str, files: &[&str]) -> Task {
        let mut task = Task::new(TaskId::new("T-01"), title, "");
        task.scope = Scope::from_paths(files.iter().copied()).unwrap();
        task
    }

    fn names(skills: &[&LoadedSkill]) -> Vec<String> {
        skills.iter().map(|s| s.info.name.clone()).collect()
    }

    #[test]
    fn always_skills_come_first_and_off_skills_stay_out() {
        let skills = [
            skill("zeta-rules", "Ledger rules", SkillMode::Always, "a"),
            skill("off-ledger", "Ledger notes", SkillMode::Off, "b"),
            skill("auto-ledger", "Ledger amounts", SkillMode::Auto, "c"),
        ];
        let picked = choose(&skills, &task("Reject negative ledger", &[]), &[]);
        assert_eq!(names(&picked), ["zeta-rules", "auto-ledger"]);
    }

    #[test]
    fn a_picked_skill_is_used_even_when_it_is_off() {
        let skills = [skill("docs-style", "Writing rules", SkillMode::Off, "a")];
        let picked = choose(
            &skills,
            &task("Unrelated title", &[]),
            &["docs-style".to_owned()],
        );
        assert_eq!(names(&picked), ["docs-style"]);
    }

    #[test]
    fn an_auto_skill_needs_a_shared_word() {
        let skills = [skill(
            "release-notes",
            "Writes changelog text",
            SkillMode::Auto,
            "a",
        )];
        let picked = choose(&skills, &task("Reject negative amounts", &[]), &[]);
        assert_eq!(names(&picked), Vec::<String>::new());
    }

    #[test]
    fn a_file_path_gives_its_words_for_matching() {
        let skills = [skill("ledger-review", "Review rules", SkillMode::Auto, "a")];
        let picked = choose(&skills, &task("Fix it", &["src/ledger.rs"]), &[]);
        assert_eq!(names(&picked), ["ledger-review"]);
    }

    #[test]
    fn at_most_three_auto_skills_are_chosen() {
        let skills: Vec<LoadedSkill> = ["aa-one", "bb-two", "cc-three", "dd-four"]
            .iter()
            .map(|name| skill(name, "ledger amounts", SkillMode::Auto, "x"))
            .collect();
        let picked = choose(&skills, &task("Ledger amounts", &[]), &[]);
        assert_eq!(picked.len(), AUTO_LIMIT);
    }

    #[test]
    fn an_always_skill_is_in_the_text_and_an_off_skill_is_not() {
        let repo = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let engine = Engine::open(EngineOptions {
            repo_root: repo.path().to_path_buf(),
            home: home.path().to_path_buf(),
            runner: Arc::new(SystemRunner),
            secrets: Arc::new(MemoryStore::new()),
            fake_agents: None,
            in_memory: true,
        })
        .unwrap();
        let pc = Caller::pc("You");
        for (name, mode) in [
            ("house-style", SkillMode::Always),
            ("quiet-notes", SkillMode::Off),
        ] {
            let new = NewSkill {
                name: name.to_owned(),
                description: "Team rules.".to_owned(),
                body: format!("Rules for {name}."),
                scope: SkillScope::Project,
            };
            engine.create_skill(new, &pc).unwrap();
            let switch = EditSkill {
                mode: Some(mode),
                ..EditSkill::default()
            };
            engine.edit_skill(name, switch, &pc).unwrap();
        }
        let text = engine.skill_text(&task("Anything", &[]), &[]);
        assert!(text.contains("### house-style") && text.contains("Rules for house-style."));
        assert!(!text.contains("quiet-notes"));
    }

    #[test]
    fn a_skill_too_big_for_the_budget_is_left_out_whole() {
        let huge = skill("huge", "Big one", SkillMode::Always, &"x".repeat(6_500));
        let small = skill("small", "Short one", SkillMode::Always, "short");
        let blocks = fitting_blocks(&[&huge, &small]);
        assert_eq!(blocks.names, ["small"]);
        assert!(blocks.blocks.iter().all(|block| !block.contains("xxxx")));
    }
}
