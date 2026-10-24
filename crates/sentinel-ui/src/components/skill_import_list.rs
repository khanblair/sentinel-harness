//! `SkillImportList`: the skills found in other tools' folders that Sentinel does not have yet
//! (`docs/ui-registry.md`, Skills). Import copies a skill and leaves the original as it was.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::skills::FoundSkill;

use crate::components::icon::Icon;
use crate::skills::{self, SkillLists, visible_found};
use crate::state::{ToastKind, Ui, use_ui};

const EMPTY_NOTE: &str = "Nothing new found. Sentinel looks in .claude/skills, .agents/skills, .agent/skills, and .codex/skills, in this repository and in your home folder.";

#[component]
pub fn SkillImportList(lists: SkillLists) -> impl IntoView {
    let ui = use_ui();
    let rows = Memo::new(move |_| {
        lists.found.with(|found| {
            visible_found(found)
                .into_iter()
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    move || {
        if ui.on_phone() {
            return view! { <Note text="Needs the computer" /> }.into_any();
        }
        if rows.with(Vec::is_empty) {
            return view! { <Note text=EMPTY_NOTE /> }.into_any();
        }
        rows.get()
            .into_iter()
            .map(|found| import_row(ui, lists, found))
            .collect_view()
            .into_any()
    }
}

/// A note line with the info icon, as the prototype draws it.
#[component]
fn Note(text: &'static str) -> impl IntoView {
    view! {
        <div class="note">
            <Icon name="info" size=14 />
            <span>{text}</span>
        </div>
    }
}

fn import_row(ui: Ui, lists: SkillLists, found: FoundSkill) -> impl IntoView {
    let FoundSkill {
        name,
        description,
        source,
        path,
        ..
    } = found;
    view! {
        <div class="fld">
            <div class="l">
                <b><span class="mono">{name.clone()}</span></b>
                <span>{description}" "<span class="chip">{source}</span></span>
            </div>
            <button class="btn sm" on:click=move |_| import(ui, lists, name.clone(), path.clone())>
                <Icon name="download" size=13 />
                " Import"
            </button>
        </div>
    }
}

fn import(ui: Ui, lists: SkillLists, name: String, path: String) {
    spawn_local(async move {
        let result = skills::import(&path).await;
        if skills::settle_change(ui, lists, result).is_some() {
            ui.show_toast(
                format!("Imported {name}. The original is untouched."),
                ToastKind::Ok,
            );
        }
    });
}
