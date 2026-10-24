//! `CareSettings`: the care schedule, the budget per run, the switch that lets care apply changes
//! to notes Sentinel wrote, and the last run date. Saves with `PUT /api/brain/care`. Run care now
//! is the computer's job.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::brain_api::CareSettings as CareData;

use crate::api;
use crate::brain::{
    FREQUENCY_CHOICES, UNAVAILABLE, budget_error, frequency_from_value, frequency_value,
};
use crate::components::icon::Icon;
use crate::components::toggle::Toggle;
use crate::layout::Layout;
use crate::state::{ToastKind, Ui, use_ui};
use crate::views::brain::{report_failure, run_care};
use crate::views::settings::Field;

/// The form: the loaded settings, the draft fields, and the budget message.
#[derive(Clone, Copy)]
struct CareForm {
    ui: Ui,
    loaded: RwSignal<Option<CareData>>,
    missing: RwSignal<bool>,
    frequency: RwSignal<String>,
    budget: RwSignal<String>,
    auto_apply: RwSignal<bool>,
    error: RwSignal<Option<&'static str>>,
}

impl CareForm {
    fn new(ui: Ui) -> Self {
        Self {
            ui,
            loaded: RwSignal::new(None),
            missing: RwSignal::new(false),
            frequency: RwSignal::new(String::new()),
            budget: RwSignal::new(String::new()),
            auto_apply: RwSignal::new(false),
            error: RwSignal::new(None),
        }
    }

    /// Fill the draft fields from settings the daemon sent.
    fn show(self, care: CareData) {
        self.frequency
            .set(frequency_value(care.frequency).to_owned());
        self.budget.set(care.budget.clone());
        self.auto_apply.set(care.auto_apply);
        self.loaded.set(Some(care));
    }

    /// The loaded settings with the draft fields applied. The last run date stays as it was.
    fn draft_settings(self) -> Option<CareData> {
        let mut next = self.loaded.get_untracked()?;
        next.frequency = frequency_from_value(&self.frequency.get_untracked());
        self.budget
            .get_untracked()
            .trim()
            .clone_into(&mut next.budget);
        next.auto_apply = self.auto_apply.get_untracked();
        Some(next)
    }
}

fn load_care(form: CareForm) {
    spawn_local(async move {
        match api::get::<CareData>("/api/brain/care").await {
            Ok(care) => form.show(care),
            Err(failure) => report_failure(form.ui, form.missing, &failure),
        }
    });
}

fn save_care(form: CareForm) {
    if let Some(message) = budget_error(&form.budget.get_untracked()) {
        form.error.set(Some(message));
        return;
    }
    form.error.set(None);
    let Some(settings) = form.draft_settings() else {
        return;
    };
    spawn_local(async move {
        match api::send::<_, serde_json::Value>("PUT", "/api/brain/care", &settings).await {
            Ok(_) => {
                form.ui
                    .show_toast("Brain care settings saved.", ToastKind::Ok);
                load_care(form);
            }
            Err(failure) => report_failure(form.ui, form.missing, &failure),
        }
    });
}

#[component]
pub fn CareSettings() -> impl IntoView {
    let form = CareForm::new(use_ui());
    Effect::new(move |_| load_care(form));
    view! {
        {move || if form.missing.get() {
            view! {
                <div class="note">
                    <Icon name="info" size=14 />
                    <span>{UNAVAILABLE}</span>
                </div>
            }.into_any()
        } else {
            view! { <CareFields form=form /> }.into_any()
        }}
    }
}

#[component]
fn CareFields(form: CareForm) -> impl IntoView {
    view! {
        <ScheduleField form=form />
        <BudgetField form=form />
        {move || form.error.get().map(|message| view! {
            <div class="sub" style="color:var(--danger)">{message}</div>
        })}
        <AutoApplyField form=form />
        <LastRunField form=form />
        <CareActions form=form />
    }
}

#[component]
fn ScheduleField(form: CareForm) -> impl IntoView {
    view! {
        <Field label="Schedule" hint="How often care runs. Off means it never runs by itself.">
            <select class="fs" aria-label="Schedule"
                on:change=move |event| form.frequency.set(event_target_value(&event))>
                {FREQUENCY_CHOICES
                    .into_iter()
                    .map(|(value, label)| view! {
                        <option value=value selected=move || form.frequency.get() == value>
                            {label}
                        </option>
                    })
                    .collect_view()}
            </select>
        </Field>
    }
}

#[component]
fn BudgetField(form: CareForm) -> impl IntoView {
    view! {
        <Field label="Budget per care run" hint="Separate from run budgets. A care run stops at this amount.">
            <span class="row">
                <span class="mono">"$"</span>
                <input type="number" step="0.01" min="0.01" aria-label="Budget per care run"
                    prop:value=move || form.budget.get()
                    on:input=move |event| form.budget.set(event_target_value(&event)) />
            </span>
        </Field>
    }
}

#[component]
fn AutoApplyField(form: CareForm) -> impl IntoView {
    view! {
        <Field label="Apply changes to notes Sentinel wrote without asking"
            hint="A note you wrote never changes without your accept.">
            <Toggle checked=Signal::derive(move || form.auto_apply.get())
                on_change=Callback::new(move |next: bool| form.auto_apply.set(next)) />
        </Field>
    }
}

#[component]
fn LastRunField(form: CareForm) -> impl IntoView {
    let last_run = move || {
        form.loaded.with(|care| {
            care.as_ref()
                .and_then(|care| care.last_run.clone())
                .unwrap_or_else(|| "Never".to_owned())
        })
    };
    view! {
        <Field label="Last run">
            <span class="k">{last_run}</span>
        </Field>
    }
}

#[component]
fn CareActions(form: CareForm) -> impl IntoView {
    view! {
        <div class="row wrap" style="margin-top:1em">
            <button class="btn pri" on:click=move |_| save_care(form)>"Save changes"</button>
            {move || (form.ui.layout.get() != Layout::Phone).then(|| view! {
                <button class="btn" on:click=move |_| run_care(form.ui, form.missing)>
                    <Icon name="play" size=14 />" Run care now"
                </button>
            })}
        </div>
    }
}
