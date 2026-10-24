//! The Appearance page of Settings. Every control writes to `ui.prefs`. The app saves the prefs
//! for this device and applies them as CSS variables (`docs/ui-tokens.md`, Settings-Driven Tokens).

use leptos::prelude::*;
use sentinel_core::TaskStatus;

use crate::components::appearance_form::AppearanceControls;
use crate::components::contrast_readout::{ContrastReadout, contrast_rows};
use crate::components::css_export::CssExport;
use crate::components::icon::Icon;
use crate::components::status_badge::StatusBadge;
use crate::components::token_editor::{TokenEditor, resolved_now};
use crate::prefs::Prefs;
use crate::state::use_ui;
use crate::views::settings::Field;

#[component]
pub fn AppearancePage() -> impl IntoView {
    view! {
        <h2>"Appearance"</h2>
        <p class="lead">"Changes apply right away and are saved on this device."</p>
        <AppearanceControls />
        <ContrastField />
        <PreviewField />
        <TokenField />
        <CssExport />
    }
}

#[component]
fn ContrastField() -> impl IntoView {
    let ui = use_ui();
    let rows = Signal::derive(move || {
        let prefs = ui.prefs.get();
        contrast_rows(&prefs, resolved_now(&prefs))
    });
    view! {
        <Field label="Contrast check" hint="Text 4.5:1, large text and borders 3:1." modifier="col">
            <ContrastReadout rows=rows />
        </Field>
    }
}

#[component]
fn PreviewField() -> impl IntoView {
    view! {
        <Field label="Preview" modifier="col">
            <div class="prev" id="prevBox">
                <div class="msg-user" style="align-self:flex-end">"Run the practice backlog."</div>
                <div class="msg-sys">
                    <p>
                        "Plan ready: " <span class="k">"12 tasks"</span> " in 4 waves. "
                        <b>"T-04"</b> " waits for " <b>"T-03"</b> "."
                    </p>
                </div>
                <div class="filecard">
                    <Icon name="file" size=15 />
                    <span class="p mono">"src/fees/rules.ts"</span>
                    <span class="rv">"Revert"</span>
                    <span class="add mono">"+24"</span>
                    <span class="del mono">"−3"</span>
                </div>
                <div class="row wrap">
                    <button class="btn pri">"Approve"</button>
                    <button class="btn">"Deny"</button>
                    <StatusBadge status=TaskStatus::Verified />
                    <StatusBadge status=TaskStatus::Running />
                    <StatusBadge status=TaskStatus::Blocked />
                    <StatusBadge status=TaskStatus::Verifying />
                </div>
            </div>
        </Field>
    }
}

#[component]
fn TokenField() -> impl IntoView {
    let ui = use_ui();
    let theme_name = move || resolved_now(&ui.prefs.get()).name();
    view! {
        <div class="fld col">
            <div class="row sp">
                <div class="l">
                    <b>"Color tokens"</b>
                    <span>{move || format!("Edit the {} theme. Each theme keeps its own values.", theme_name())}</span>
                </div>
                <button class="btn sm" on:click=move |_| ui.prefs.set(Prefs::default())>
                    "Reset all appearance"
                </button>
            </div>
            <TokenEditor />
        </div>
    }
}
