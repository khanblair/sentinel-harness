//! `SkillsPage`: the Skills page of Settings (`docs/ui-rules.md`, Settings; `docs/ui-registry.md`,
//! Skills and Routines). It lists the skills Sentinel has, the ones found on this computer, and
//! how skills are used. On a phone the add, edit, delete, and import controls are off.

use leptos::prelude::*;

use crate::components::icon::Icon;
use crate::components::skill_editor::{EditorMode, SkillEditor};
use crate::components::skill_import_list::SkillImportList;
use crate::components::skill_row::SkillRow;
use crate::skills::{self, SkillLists};
use crate::state::{Ui, use_ui};

#[component]
pub fn SkillsPage() -> impl IntoView {
    let ui = use_ui();
    let lists = SkillLists {
        mine: RwSignal::new(Vec::new()),
        found: RwSignal::new(Vec::new()),
    };
    let editor = RwSignal::new(None::<EditorMode>);
    skills::load(ui, lists);
    view! {
        <h2>"Skills"</h2>
        <p class="lead">
            "A skill is a folder with a "<span class="k">"SKILL.md"</span>
            " that teaches agents one thing the way you want it done. It is the same format Claude, Codex, and Antigravity use. Sentinel puts the right skills into each task."
        </p>
        <div class="psec">
            <h5>"Your skills"</h5>
            {move || mine_view(lists, editor)}
        </div>
        <NewSkillButton ui=ui editor=editor />
        {move || editor.get().map(|mode| view! {
            <SkillEditor mode=mode lists=lists on_close=Callback::new(move |()| editor.set(None)) />
        })}
        <div class="psec">
            <h5>"Found on this computer"</h5>
            <SkillImportList lists=lists />
        </div>
        <UsageSection />
    }
}

/// The "How skills are used" section: what each switch position does, and where the files live.
#[component]
fn UsageSection() -> impl IntoView {
    view! {
        <div class="psec">
            <h5>"How skills are used"</h5>
            <UsageRow label="Always" hint="In every task prompt." />
            <UsageRow label="Auto" hint="When the words of its name and description match the task. At most 3 per task, within 1,500 tokens." />
            <UsageRow label="Off" hint="Never, unless you add it to a message by hand with /skill." />
            <div class="fld">
                <div class="l">
                    <b>"Files"</b>
                    <span>"Project skills are in .sentinel/skills/. Personal skills are in ~/.sentinel/skills/. Sentinel never runs a file in a skill folder."</span>
                </div>
                <span class="k">"SKILL.md"</span>
            </div>
        </div>
    }
}

/// The New skill button. On a phone it is off, with the note that says why.
#[component]
fn NewSkillButton(ui: Ui, editor: RwSignal<Option<EditorMode>>) -> impl IntoView {
    view! {
        <div class="row wrap" style="margin:.8em 0">
            <button class="btn pri" disabled=move || ui.on_phone()
                on:click=move |_| editor.set(Some(EditorMode::New))>
                <Icon name="plus" size=14 />" New skill"
            </button>
            {move || ui.on_phone().then(|| view! { <span class="sub">"Needs the computer"</span> })}
        </div>
    }
}

/// The list of the skills, or the note the prototype shows when there are none. A skill name
/// opens the editor for that skill.
fn mine_view(lists: SkillLists, editor: RwSignal<Option<EditorMode>>) -> AnyView {
    lists.mine.with(|list| {
        if list.is_empty() {
            return view! {
                <div class="note">
                    <Icon name="info" size=14 />
                    <span>"No skills yet. Add one, or import one from another tool."</span>
                </div>
            }
            .into_any();
        }
        list.iter()
            .map(|skill| {
                let on_edit = Callback::new(move |name: String| {
                    editor.set(Some(EditorMode::Edit(name)));
                });
                view! { <SkillRow skill=skill.clone() lists=lists on_edit=on_edit /> }
            })
            .collect_view()
            .into_any()
    })
}

/// One line of the "How skills are used" section: a label and its hint, with no control. It is
/// the prototype's `fld(label, hint, '')`.
#[component]
fn UsageRow(label: &'static str, hint: &'static str) -> impl IntoView {
    view! {
        <div class="fld">
            <div class="l">
                <b>{label}</b>
                <span>{hint}</span>
            </div>
        </div>
    }
}
