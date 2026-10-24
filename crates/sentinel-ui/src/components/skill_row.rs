//! `SkillRow`: one skill on the Skills page (`docs/ui-registry.md`, `SkillRow`). Its name opens the
//! editor. Its switch sets Off, Auto, or Always. Its trash button asks once more before it removes
//! the skill. On a phone the name and the trash button are off, and a switch can only move toward
//! Off.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::skills::{SkillInfo, SkillMode};

use crate::components::icon::Icon;
use crate::skills::{self, SkillLists, mode_choices, mode_label, scope_label};
use crate::state::{ToastKind, Ui, use_ui};

#[component]
pub fn SkillRow(
    skill: SkillInfo,
    lists: SkillLists,
    #[prop(into)] on_edit: Callback<String>,
) -> impl IntoView {
    let ui = use_ui();
    let SkillInfo {
        name,
        description,
        scope,
        mode,
        ..
    } = skill;
    let edit_name = name.clone();
    view! {
        <div class="fld">
            <div class="l">
                <b>
                    <button class="mono" disabled=move || ui.on_phone()
                        on:click=move |_| on_edit.run(edit_name.clone())>
                        {name.clone()}
                    </button>
                </b>
                <span>{description}" "<span class="chip">{scope_label(scope)}</span></span>
            </div>
            <span class="row">
                <ModeSwitch name=name.clone() current=mode lists=lists />
                <TrashButton name=name lists=lists />
            </span>
        </div>
    }
}

/// The Off, Auto, Always switch. The pressed one is the current mode.
#[component]
fn ModeSwitch(name: String, current: SkillMode, lists: SkillLists) -> impl IntoView {
    let ui = use_ui();
    view! {
        <span class="seg">
            {move || mode_choices(current, ui.on_phone())
                .into_iter()
                .map(|(mode, enabled)| {
                    let name = name.clone();
                    view! {
                        <button aria-pressed=(current == mode).to_string() disabled=!enabled
                            on:click=move |_| change_mode(ui, lists, name.clone(), mode)>
                            {mode_label(mode)}
                        </button>
                    }
                })
                .collect_view()}
        </span>
    }
}

fn change_mode(ui: Ui, lists: SkillLists, name: String, mode: SkillMode) {
    spawn_local(async move {
        let result = skills::set_mode(&name, mode).await;
        if skills::settle_change(ui, lists, result).is_some() {
            ui.show_toast(format!("{name}: {}", mode_label(mode)), ToastKind::Ok);
        }
    });
}

/// The trash button. The first tap asks, the second removes the skill.
#[component]
fn TrashButton(name: String, lists: SkillLists) -> impl IntoView {
    let ui = use_ui();
    let asking = RwSignal::new(false);
    let label = format!("Delete {name}");
    view! {
        {move || if asking.get() {
            confirm_buttons(ui, lists, name.clone(), asking).into_any()
        } else {
            view! {
                <button class="ibtn" aria-label=label.clone() disabled=move || ui.on_phone()
                    on:click=move |_| asking.set(true)>
                    <Icon name="trash" size=15 />
                </button>
            }.into_any()
        }}
    }
}

fn confirm_buttons(
    ui: Ui,
    lists: SkillLists,
    name: String,
    asking: RwSignal<bool>,
) -> impl IntoView {
    view! {
        <button class="btn sm" on:click=move |_| remove(ui, lists, name.clone())>"Delete"</button>
        <button class="btn sm" on:click=move |_| asking.set(false)>"Cancel"</button>
    }
}

fn remove(ui: Ui, lists: SkillLists, name: String) {
    spawn_local(async move {
        let result = skills::remove(&name).await;
        if skills::settle_change(ui, lists, result).is_some() {
            ui.show_toast("Skill deleted", ToastKind::Ok);
        }
    });
}
