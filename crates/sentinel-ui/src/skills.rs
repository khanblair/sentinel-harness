//! Skills in the UI: the rules the Skills page and the skill picker share, and the calls to
//! `/api/skills` (`docs/product-scope.md`, F24). The rules are plain functions with unit tests.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::skills::{
    DESCRIPTION_MAX, EditSkill, FoundSkill, ImportSkill, NAME_MAX, NewSkill, SkillDetail,
    SkillInfo, SkillMode, SkillScope,
};

use crate::api::{self, ApiFailure};
use crate::state::{ToastKind, Ui};

/// The two lists the Skills page shows, as the daemon last returned them.
#[derive(Clone, Copy)]
pub struct SkillLists {
    pub mine: RwSignal<Vec<SkillInfo>>,
    pub found: RwSignal<Vec<FoundSkill>>,
}

/// Whether a skill name is usable: lowercase letters, digits, and dashes, one to `NAME_MAX`.
pub fn name_valid(name: &str) -> bool {
    let length = name.chars().count();
    (1..=NAME_MAX).contains(&length)
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Characters left under the description limit. Negative when the text is too long.
pub fn description_left(text: &str) -> i32 {
    let limit = i32::try_from(DESCRIPTION_MAX).unwrap_or(i32::MAX);
    let used = i32::try_from(text.chars().count()).unwrap_or(i32::MAX);
    limit - used
}

pub fn mode_label(mode: SkillMode) -> &'static str {
    match mode {
        SkillMode::Off => "Off",
        SkillMode::Auto => "Auto",
        SkillMode::Always => "Always",
    }
}

pub fn scope_label(scope: SkillScope) -> &'static str {
    match scope {
        SkillScope::Project => "Project",
        SkillScope::Personal => "Personal",
    }
}

/// The three switch positions, each with whether it can be picked. On a phone a switch only
/// moves toward Off, so a raise is disabled there.
pub fn mode_choices(current: SkillMode, phone: bool) -> Vec<(SkillMode, bool)> {
    [SkillMode::Off, SkillMode::Auto, SkillMode::Always]
        .into_iter()
        .map(|mode| (mode, !phone || current.phone_may_set(mode)))
        .collect()
}

/// The found skills that Sentinel does not have yet.
pub fn visible_found(found: &[FoundSkill]) -> Vec<&FoundSkill> {
    found
        .iter()
        .filter(|skill| !skill.already_imported)
        .collect()
}

/// The fields the editor changed. A field that did not change is left out, so the daemon keeps it.
pub fn edit_changes(detail: &SkillDetail, description: &str, body: &str) -> EditSkill {
    EditSkill {
        description: (description != detail.info.description).then(|| description.to_owned()),
        body: (body != detail.body).then(|| body.to_owned()),
        mode: None,
    }
}

/// The toast for a name that is not a skill. It names the skills Sentinel has.
pub fn unknown_skill_text(name: &str, list: &[SkillInfo]) -> String {
    if list.is_empty() {
        return format!("No skill named {name}. Add one in Settings, Skills.");
    }
    let names: Vec<&str> = list.iter().map(|skill| skill.name.as_str()).collect();
    format!("No skill named {name}. Your skills: {}.", names.join(", "))
}

fn skill_path(name: &str) -> String {
    format!("/api/skills/{name}")
}

pub async fn read(name: &str) -> Result<SkillDetail, ApiFailure> {
    api::get(&skill_path(name)).await
}

pub async fn create(new: &NewSkill) -> Result<SkillDetail, ApiFailure> {
    api::post("/api/skills", new).await
}

pub async fn edit(name: &str, change: &EditSkill) -> Result<SkillDetail, ApiFailure> {
    api::send("PUT", &skill_path(name), change).await
}

pub async fn set_mode(name: &str, mode: SkillMode) -> Result<SkillDetail, ApiFailure> {
    let change = EditSkill {
        mode: Some(mode),
        ..EditSkill::default()
    };
    edit(name, &change).await
}

pub async fn remove(name: &str) -> Result<serde_json::Value, ApiFailure> {
    api::send("DELETE", &skill_path(name), &serde_json::json!({})).await
}

/// Copies a found skill into Sentinel. The original folder is not touched.
pub async fn import(path: &str) -> Result<SkillInfo, ApiFailure> {
    let body = ImportSkill {
        path: path.to_owned(),
        scope: SkillScope::Project,
    };
    api::post("/api/skills/import", &body).await
}

/// Loads the list, and on a computer the found skills too. A failed load shows its sentence.
pub fn load(ui: Ui, lists: SkillLists) {
    spawn_local(async move {
        match api::get::<Vec<SkillInfo>>("/api/skills").await {
            Ok(list) => {
                let _ = lists.mine.try_set(list);
            }
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
    if !ui.on_phone() {
        load_found(lists);
    }
}

/// The found list is computer only. A phone that asks gets an error, which stays quiet.
fn load_found(lists: SkillLists) {
    spawn_local(async move {
        if let Ok(list) = api::get::<Vec<FoundSkill>>("/api/skills/found").await {
            let _ = lists.found.try_set(list);
        }
    });
}

/// Settles one change. A failure shows the daemon's sentence and gives `None`. A success reloads
/// both lists and gives the answer.
pub fn settle_change<T>(ui: Ui, lists: SkillLists, result: Result<T, ApiFailure>) -> Option<T> {
    match result {
        Ok(answer) => {
            load(ui, lists);
            Some(answer)
        }
        Err(failure) => {
            ui.show_toast(failure.to_string(), ToastKind::Danger);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(name: &str, imported: bool) -> FoundSkill {
        FoundSkill {
            name: name.to_owned(),
            description: String::new(),
            source: ".claude/skills".to_owned(),
            path: format!("/repo/.claude/skills/{name}"),
            already_imported: imported,
        }
    }

    fn skill_info(name: &str) -> SkillInfo {
        SkillInfo {
            name: name.to_owned(),
            description: String::new(),
            scope: SkillScope::Project,
            mode: SkillMode::Auto,
            bytes: 0,
        }
    }

    fn detail(description: &str, body: &str) -> SkillDetail {
        SkillDetail {
            info: SkillInfo {
                name: "house-style".to_owned(),
                description: description.to_owned(),
                scope: SkillScope::Project,
                mode: SkillMode::Auto,
                bytes: 120,
            },
            body: body.to_owned(),
            files: Vec::new(),
        }
    }

    #[test]
    fn a_lowercase_name_with_digits_and_dashes_is_valid() {
        assert!(name_valid("house-style"));
        assert!(name_valid("release2"));
    }

    #[test]
    fn a_name_with_capitals_spaces_or_no_letters_is_refused() {
        assert!(!name_valid("House"));
        assert!(!name_valid("house style"));
        assert!(!name_valid("house_style"));
        assert!(!name_valid(""));
    }

    #[test]
    fn a_name_longer_than_the_limit_is_refused() {
        assert!(name_valid(&"a".repeat(NAME_MAX)));
        assert!(!name_valid(&"a".repeat(NAME_MAX + 1)));
    }

    #[test]
    fn the_description_count_goes_down_as_the_text_grows() {
        assert_eq!(description_left(""), 300);
        assert_eq!(description_left("abc"), 297);
        assert!(description_left(&"x".repeat(301)) < 0);
    }

    #[test]
    fn the_mode_labels_match_the_switch() {
        assert_eq!(mode_label(SkillMode::Off), "Off");
        assert_eq!(mode_label(SkillMode::Auto), "Auto");
        assert_eq!(mode_label(SkillMode::Always), "Always");
    }

    #[test]
    fn on_a_computer_every_mode_can_be_picked() {
        let choices = mode_choices(SkillMode::Off, false);
        assert_eq!(
            choices,
            vec![
                (SkillMode::Off, true),
                (SkillMode::Auto, true),
                (SkillMode::Always, true),
            ]
        );
    }

    #[test]
    fn on_a_phone_only_the_modes_at_or_below_the_current_one_can_be_picked() {
        let choices = mode_choices(SkillMode::Auto, true);
        assert_eq!(
            choices,
            vec![
                (SkillMode::Off, true),
                (SkillMode::Auto, true),
                (SkillMode::Always, false),
            ]
        );
    }

    #[test]
    fn found_skills_already_imported_are_hidden() {
        let list = vec![found("house-style", true), found("release", false)];
        let names: Vec<&str> = visible_found(&list)
            .iter()
            .map(|skill| skill.name.as_str())
            .collect();
        assert_eq!(names, vec!["release"]);
    }

    #[test]
    fn an_unknown_skill_name_lists_the_skills_there_are() {
        let list = vec![skill_info("house-style"), skill_info("release")];
        assert_eq!(
            unknown_skill_text("nope", &list),
            "No skill named nope. Your skills: house-style, release."
        );
        assert_eq!(
            unknown_skill_text("nope", &[]),
            "No skill named nope. Add one in Settings, Skills."
        );
    }

    #[test]
    fn an_unchanged_editor_sends_no_fields() {
        let before = detail("Use the house style.", "Write short lines.");
        let change = edit_changes(&before, "Use the house style.", "Write short lines.");
        assert_eq!(change, EditSkill::default());
    }

    #[test]
    fn only_the_changed_description_is_sent() {
        let before = detail("Use the house style.", "Write short lines.");
        let change = edit_changes(
            &before,
            "Use the house style, always.",
            "Write short lines.",
        );
        assert_eq!(
            change.description.as_deref(),
            Some("Use the house style, always.")
        );
        assert_eq!(change.body, None);
        assert_eq!(change.mode, None);
    }
}
