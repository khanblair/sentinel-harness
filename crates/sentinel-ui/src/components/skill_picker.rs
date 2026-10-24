//! `SkillPicker`: the Use a skill dialog from the plus menu. It lists the skills Sentinel has, takes
//! one choice, and adds it to the next message as a skill chip (`docs/ui-rules.md`, Composer).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::skills::SkillInfo;

use crate::api;
use crate::components::icon::Icon;
use crate::state::{ToastKind, Ui, use_ui};
use crate::views::composer_send::add_skill;

/// The skills as the list request returned them. `None` while the request runs.
type SkillChoices = RwSignal<Option<Vec<SkillInfo>>>;

#[component]
pub fn SkillPicker() -> impl IntoView {
    let ui = use_ui();
    let choices: SkillChoices = RwSignal::new(None);
    let picked = RwSignal::new(None::<String>);
    load(ui, choices);
    view! {
        <div class="modal-hd">
            <Icon name="book" size=18 />
            <h3>"Use a skill"</h3>
            <button class="ibtn" aria-label="Close" on:click=move |_| ui.modal.set(None)>
                <Icon name="x" size=17 />
            </button>
        </div>
        <div class="modal-body" style="padding:1em 1.1em">
            <div>{move || choices.with(|list| choice_rows(list.as_deref(), picked))}</div>
        </div>
        <div class="modal-ft">
            <button class="btn" on:click=move |_| ui.modal.set(None)>"Cancel"</button>
            <button class="btn pri" disabled=move || picked.with(Option::is_none)
                on:click=move |_| confirm(ui, picked)>
                "Use skill"
            </button>
        </div>
    }
}

/// Loads the skill list into the dialog. A failed load shows its sentence and an empty list.
fn load(ui: Ui, choices: SkillChoices) {
    spawn_local(async move {
        let list = match api::get::<Vec<SkillInfo>>("/api/skills").await {
            Ok(list) => list,
            Err(failure) => {
                ui.show_toast(failure.to_string(), ToastKind::Danger);
                Vec::new()
            }
        };
        let _ = choices.try_set(Some(list));
    });
}

/// The rows of the list, or the note that says why there are none.
fn choice_rows(list: Option<&[SkillInfo]>, picked: RwSignal<Option<String>>) -> AnyView {
    match list {
        None => view! { <div class="note">"Loading the skills."</div> }.into_any(),
        Some([]) => view! {
            <div class="note">"No skills yet. Add one in Settings, Skills."</div>
        }
        .into_any(),
        Some(skills) => skills
            .iter()
            .map(|skill| choice_row(picked, skill))
            .collect_view()
            .into_any(),
    }
}

/// One radio row: the name in mono, the description under it.
fn choice_row(picked: RwSignal<Option<String>>, skill: &SkillInfo) -> impl IntoView {
    let name = skill.name.clone();
    let checked_name = name.clone();
    view! {
        <label class="mi">
            <input type="radio" name="skillPick" value=name.clone()
                prop:checked=move || picked.with(|choice| choice.as_deref() == Some(checked_name.as_str()))
                on:change=move |_| picked.set(Some(name.clone())) />
            <span>
                <span class="mono">{skill.name.clone()}</span>
                <div class="d">{skill.description.clone()}</div>
            </span>
        </label>
    }
}

/// Closes the dialog and adds the chosen skill to the open chat.
fn confirm(ui: Ui, picked: RwSignal<Option<String>>) {
    let Some(name) = picked.get_untracked() else {
        return;
    };
    ui.modal.set(None);
    add_skill(ui, &name);
}
