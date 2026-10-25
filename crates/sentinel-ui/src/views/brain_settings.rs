//! The Brain settings page: the mode, the vault folder, the run journal, and the care schedule.
//! A phone may only tighten the mode, the same rule as the Brain menu.

use leptos::prelude::*;
use sentinel_core::BrainMode;

use crate::brain::phone_may_pick;
use crate::components::care_settings::CareSettings;
use crate::components::icon::Icon;
use crate::state::{Ui, use_ui};
use crate::views::brain::set_brain_mode;
use crate::views::settings::Field;
use crate::views::settings_pages::setting_or;

/// The three modes, with the prototype's words.
const MODES: [(BrainMode, &str, &str); 3] = [
    (BrainMode::Off, "Off", "No notes read or written."),
    (
        BrainMode::ReadOnly,
        "Read only",
        "Agents read notes. Nothing is written. Best for live runs.",
    ),
    (
        BrainMode::Learn,
        "Learn",
        "Agents read notes. Each run writes a journal and lesson candidates.",
    ),
];

#[component]
pub fn BrainSettings() -> impl IntoView {
    let ui = use_ui();
    view! {
        <h2>"Brain"</h2>
        <p class="lead">"A folder of plain markdown notes with links between them. Agents read it as memory, every run can write to it, and you can open the same folder in Obsidian."</p>
        <div class="psec">
            <h5>"Mode"</h5>
            <div class="radios">
                {MODES
                    .into_iter()
                    .map(|(mode, label, hint)| mode_button(ui, mode, label, hint))
                    .collect_view()}
            </div>
        </div>
        <Field label="Vault folder" hint="Plain markdown files. Open it in Obsidian to browse or edit. Edits show up here.">
            <span class="k">{move || setting_or(&ui.snapshot.get().settings, "brain_vault", ".sentinel/brain/")}</span>
        </Field>
        <Field label="Run journal" hint="In learn mode, write a note for every run, task, and touched file.">
            <span class="k">{move || setting_or(&ui.snapshot.get().settings, "brain_journal", "Not set")}</span>
        </Field>
        <h2 style="margin-top:1.3em;font-size:1.1em">"Scheduled care"</h2>
        <p class="lead">"Runs on a schedule while Sentinel is on. It refreshes notes from the repository, merges duplicates, summarizes old runs, weakens notes nobody uses, and proposes improvements."</p>
        <CareSettings />
    }
}

fn mode_button(ui: Ui, mode: BrainMode, label: &'static str, hint: &'static str) -> impl IntoView {
    let pressed = move || {
        ui.snapshot
            .with(|snapshot| snapshot.brain_mode == mode)
            .to_string()
    };
    let disabled = move || {
        let current = ui.snapshot.with(|snapshot| snapshot.brain_mode);
        !phone_may_pick(current, mode, ui.on_phone())
    };
    view! {
        <button aria-pressed=pressed disabled=disabled on:click=move |_| set_brain_mode(ui, mode)>
            <span><Icon name="brain" size=17 /></span>
            <div>
                <b>{label}</b>
                <div class="d">{hint}</div>
            </div>
        </button>
    }
}
