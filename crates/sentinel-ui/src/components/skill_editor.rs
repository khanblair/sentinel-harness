//! `SkillEditor`: the New skill and Edit form (`docs/ui-registry.md`, `SkillEditor`). It sits on the
//! Skills page under the list. A new skill takes a name and a scope. An edit keeps the name and
//! sends only the fields it changed.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::skills::{EditSkill, NewSkill, SkillDetail, SkillScope};

use crate::skills::{self, SkillLists, description_left, edit_changes, name_valid, scope_label};
use crate::state::{Ui, use_ui};

/// Which form is open: a new skill, or the edit of the skill with this name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorMode {
    New,
    Edit(String),
}

/// Whether Save can send. A new skill needs a valid name. Both need a description within the limit.
pub fn can_save(mode: &EditorMode, name: &str, description: &str) -> bool {
    let named = match mode {
        EditorMode::New => name_valid(name),
        EditorMode::Edit(_) => true,
    };
    named && !description.trim().is_empty() && description_left(description) >= 0
}

/// The form's text, kept together so the save path takes one value.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Draft {
    name: String,
    description: String,
    body: String,
    scope: SkillScope,
}

/// The handles the form and its save path share. All of them are `Copy`.
#[derive(Clone, Copy)]
struct Form {
    ui: Ui,
    lists: SkillLists,
    draft: RwSignal<Draft>,
    original: RwSignal<Option<SkillDetail>>,
    on_close: Callback<()>,
}

/// The form's signals. An edit starts loading the skill's copy right away.
fn open_form(mode: &EditorMode, lists: SkillLists, on_close: Callback<()>) -> Form {
    let name = match mode {
        EditorMode::New => String::new(),
        EditorMode::Edit(name) => name.clone(),
    };
    let draft = RwSignal::new(Draft {
        name,
        description: String::new(),
        body: String::new(),
        scope: SkillScope::Project,
    });
    let original = RwSignal::new(None::<SkillDetail>);
    if let EditorMode::Edit(name) = mode {
        load_detail(name.clone(), draft, original);
    }
    Form {
        ui: use_ui(),
        lists,
        draft,
        original,
        on_close,
    }
}

#[component]
pub fn SkillEditor(mode: EditorMode, lists: SkillLists, on_close: Callback<()>) -> impl IntoView {
    let form = open_form(&mode, lists, on_close);
    let draft = form.draft;
    let title = match &mode {
        EditorMode::New => "New skill",
        EditorMode::Edit(_) => "Edit skill",
    };
    let is_new = mode == EditorMode::New;
    let save_mode = mode.clone();
    view! {
        <div class="psec">
            <h5>{title}</h5>
            <DraftFields draft=draft is_new=is_new />
            <Show when=move || is_new fallback=|| ()>
                <ScopeChoice draft=draft />
            </Show>
            <div class="row wrap" style="margin-top:.6em">
                <button class="btn" on:click=move |_| on_close.run(())>"Cancel"</button>
                <button class="btn pri"
                    disabled=move || !ready(form, &mode)
                    on:click=move |_| save(form, save_mode.clone())>
                    "Save"
                </button>
            </div>
        </div>
    }
}

/// The name, description, and text fields. The name is fixed once a skill exists.
#[component]
fn DraftFields(draft: RwSignal<Draft>, is_new: bool) -> impl IntoView {
    view! {
        <input class="field" placeholder="Name" aria-label="Name" disabled=!is_new
            prop:value=move || draft.with(|d| d.name.clone())
            on:input=move |event| draft.update(|d| d.name = event_target_value(&event)) />
        <p class="sub">"Lowercase letters, digits, and dashes."</p>
        <input class="field" placeholder="One sentence: what it does and when to use it"
            aria-label="Description" prop:value=move || draft.with(|d| d.description.clone())
            on:input=move |event| draft.update(|d| d.description = event_target_value(&event)) />
        <p class="sub">{move || draft.with(|d| description_left(&d.description))}" characters left"</p>
        <textarea class="field" rows="7" aria-label="Skill text"
            prop:value=move || draft.with(|d| d.body.clone())
            on:input=move |event| draft.update(|d| d.body = event_target_value(&event))></textarea>
    }
}

/// Project or Personal. The pressed one is the scope of the new skill.
#[component]
fn ScopeChoice(draft: RwSignal<Draft>) -> impl IntoView {
    let choice = move |scope: SkillScope| {
        view! {
            <button aria-pressed=move || (draft.with(|d| d.scope) == scope).to_string()
                on:click=move |_| draft.update(|d| d.scope = scope)>
                {scope_label(scope)}
            </button>
        }
    };
    view! {
        <div class="row wrap" style="margin-top:.6em">
            <span class="seg">
                {choice(SkillScope::Project)}
                {choice(SkillScope::Personal)}
            </span>
            <span class="sub">"Project skills are in .sentinel/skills/. Personal skills are in ~/.sentinel/skills/."</span>
        </div>
    }
}

/// Fills the form from the daemon's copy of the skill. The copy is kept for the changed fields.
fn load_detail(name: String, draft: RwSignal<Draft>, original: RwSignal<Option<SkillDetail>>) {
    spawn_local(async move {
        let Ok(detail) = skills::read(&name).await else {
            return;
        };
        let _ = draft.try_update(|d| {
            d.description.clone_from(&detail.info.description);
            d.body.clone_from(&detail.body);
        });
        let _ = original.try_set(Some(detail));
    });
}

/// Save is ready once the form passes `can_save`, and an edit has its copy loaded.
fn ready(form: Form, mode: &EditorMode) -> bool {
    let loaded = match mode {
        EditorMode::New => true,
        EditorMode::Edit(_) => form.original.with(Option::is_some),
    };
    loaded && form.draft.with(|d| can_save(mode, &d.name, &d.description))
}

fn save(form: Form, mode: EditorMode) {
    let current = form.draft.get_untracked();
    spawn_local(async move {
        let saved = match mode {
            EditorMode::New => create(form, current).await,
            EditorMode::Edit(name) => match form.original.get_untracked() {
                Some(detail) => update(form, &name, &detail, &current).await,
                None => false,
            },
        };
        if saved {
            form.on_close.run(());
        }
    });
}

async fn create(form: Form, draft: Draft) -> bool {
    let new = NewSkill {
        name: draft.name,
        description: draft.description,
        body: draft.body,
        scope: draft.scope,
    };
    let result = skills::create(&new).await;
    skills::settle_change(form.ui, form.lists, result).is_some()
}

async fn update(form: Form, name: &str, detail: &SkillDetail, draft: &Draft) -> bool {
    let change = edit_changes(detail, &draft.description, &draft.body);
    if change == EditSkill::default() {
        return true;
    }
    let result = skills::edit(name, &change).await;
    skills::settle_change(form.ui, form.lists, result).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_skill_needs_a_valid_name_and_a_description() {
        assert!(can_save(
            &EditorMode::New,
            "house-style",
            "Use the house style."
        ));
        assert!(!can_save(
            &EditorMode::New,
            "House Style",
            "Use the house style."
        ));
        assert!(!can_save(&EditorMode::New, "house-style", "   "));
    }

    #[test]
    fn an_edit_keeps_its_name_and_only_needs_a_description() {
        let mode = EditorMode::Edit("house-style".to_owned());
        assert!(can_save(&mode, "house-style", "Use the house style."));
        assert!(!can_save(&mode, "house-style", ""));
    }

    #[test]
    fn a_description_over_the_limit_cannot_be_saved() {
        let long = "x".repeat(301);
        assert!(!can_save(&EditorMode::New, "house-style", &long));
    }
}
