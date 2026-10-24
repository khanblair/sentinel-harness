//! `RoutineRow`: one routine on the Routines page (`docs/product-scope.md`, F25). On a phone a
//! routine can only be turned off. Its time, days, destination, and list changes need the computer.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::RoutineId;
use sentinel_core::routines::{EditRoutine, Routine, RoutineKind, clock_text, parse_clock};

use crate::components::icon::Icon;
use crate::components::routine_editor::EditorMode;
use crate::layout::Layout;
use crate::routines::{
    self, about_text, day_choices, days_from_label, days_label, last_text, time_choices,
};
use crate::state::{Ui, use_ui};

#[component]
pub fn RoutineRow(routine: Routine, editor: RwSignal<Option<EditorMode>>) -> impl IntoView {
    view! {
        <div class="card" style="margin-bottom:.6em">
            <RoutineHead routine=routine.clone() editor />
            <div class="sub" style="margin:.35em 0">{about_text(&routine)}</div>
            <RoutineSchedule routine />
        </div>
    }
}

/// The name, the edit and delete buttons of a reminder or backlog run, and the on switch.
#[component]
fn RoutineHead(routine: Routine, editor: RwSignal<Option<EditorMode>>) -> impl IntoView {
    let ui = use_ui();
    let phone = move || ui.layout.get() == Layout::Phone;
    let on = routine.enabled;
    let id = routine.id.clone();
    let name = routine.name.clone();
    let removable = matches!(
        routine.kind,
        RoutineKind::Reminder | RoutineKind::BacklogRun
    );
    view! {
        <div class="row sp wrap">
            <span class="row"><Icon name="clock" size=17 /><b>{name.clone()}</b></span>
            <span class="row">
                {removable.then(|| view! { <EditButtons routine=routine.clone() editor /> })}
                <RoutineSwitch
                    checked=on
                    label=name
                    disabled=move || phone() && !on
                    on_change=move |value| edit_routine(ui, id.clone(), EditRoutine { enabled: Some(value), ..EditRoutine::default() })
                />
            </span>
        </div>
    }
}

/// Edit and delete for reminders and backlog runs. The two built-in routines have neither.
#[component]
fn EditButtons(routine: Routine, editor: RwSignal<Option<EditorMode>>) -> impl IntoView {
    let ui = use_ui();
    let phone = move || ui.layout.get() == Layout::Phone;
    let name = routine.name.clone();
    let edit_name = format!("Edit {name}");
    let delete_name = format!("Delete {name}");
    let mode = EditorMode::Edit(routine.clone());
    let id = routine.id.clone();
    view! {
        <button class="ibtn" aria-label=edit_name disabled=phone
            title=move || needs_computer(phone())
            on:click=move |_| editor.set(Some(mode.clone()))>
            <Icon name="edit" size=15 />
        </button>
        <button class="ibtn" aria-label=delete_name disabled=phone
            title=move || needs_computer(phone())
            on:click=move |_| delete_routine(ui, id.clone())>
            <Icon name="trash" size=15 />
        </button>
    }
}

/// The time, days, destination, last run, and Send now. Send now works on a phone too.
#[component]
fn RoutineSchedule(routine: Routine) -> impl IntoView {
    let ui = use_ui();
    let phone = move || ui.layout.get() == Layout::Phone;
    let send_id = routine.id.clone();
    let time_id = routine.id.clone();
    let days_id = routine.id.clone();
    let tg_id = routine.id.clone();
    view! {
        <div class="row wrap">
            <span class="row">
                <label class="sub">"At"</label>
                <TimeSelect
                    minute=routine.minute_of_day
                    disabled=phone
                    on_pick=move |minute| edit_routine(ui, time_id.clone(), EditRoutine { minute_of_day: Some(minute), ..EditRoutine::default() })
                />
            </span>
            <DaySelect
                days=routine.days
                disabled=phone
                on_pick=move |days| edit_routine(ui, days_id.clone(), EditRoutine { days: Some(days), ..EditRoutine::default() })
            />
            <span class="row">
                <RoutineSwitch
                    checked=routine.to_telegram
                    label="Send to Telegram"
                    disabled=phone
                    on_change=move |value| edit_routine(ui, tg_id.clone(), EditRoutine { to_telegram: Some(value), ..EditRoutine::default() })
                />
                <span class="sub">"Send to Telegram"</span>
            </span>
            <span style="flex:1"></span>
            <span class="sub">{last_text(routine.last_fired_on.as_deref())}</span>
            <button class="btn sm" on:click=move |_| routines::send_now_in_background(ui, send_id.clone())>
                "Send now"
            </button>
        </div>
    }
}

/// An on or off switch that can be disabled. The shared `Toggle` has no disabled state.
#[component]
pub fn RoutineSwitch(
    #[prop(into)] checked: Signal<bool>,
    #[prop(into)] disabled: Signal<bool>,
    #[prop(into)] label: String,
    #[prop(into)] on_change: Callback<bool>,
) -> impl IntoView {
    view! {
        <button class="tog" role="switch" aria-label=label
            aria-checked=move || checked.get().to_string()
            disabled=move || disabled.get()
            on:click=move |_| on_change.run(!checked.get_untracked())>
        </button>
    }
}

/// The `At` select: each time choice as `HH:MM`, with the routine's own time among them.
#[component]
pub fn TimeSelect(
    #[prop(into)] minute: Signal<u16>,
    #[prop(into)] disabled: Signal<bool>,
    #[prop(into)] on_pick: Callback<u16>,
) -> impl IntoView {
    view! {
        <select aria-label="Time" disabled=move || disabled.get()
            on:change=move |ev| pick_time(&event_target_value(&ev), on_pick)>
            {move || time_options(minute.get())}
        </select>
    }
}

fn time_options(current: u16) -> impl IntoView {
    time_choices(current)
        .into_iter()
        .map(|minute| {
            view! {
                <option value=clock_text(minute) selected={minute == current}>{clock_text(minute)}</option>
            }
        })
        .collect_view()
}

fn pick_time(value: &str, on_pick: Callback<u16>) {
    if let Some(minute) = parse_clock(value) {
        on_pick.run(minute);
    }
}

/// The days select: Every day, Weekdays, Weekends, and Custom only when the mask is custom.
#[component]
pub fn DaySelect(
    #[prop(into)] days: Signal<u8>,
    #[prop(into)] disabled: Signal<bool>,
    #[prop(into)] on_pick: Callback<u8>,
) -> impl IntoView {
    view! {
        <select aria-label="Days" disabled=move || disabled.get()
            on:change=move |ev| on_pick.run(days_from_label(&event_target_value(&ev)))>
            {move || day_options(days.get())}
        </select>
    }
}

fn day_options(current: u8) -> impl IntoView {
    let current_label = days_label(current);
    day_choices(current)
        .into_iter()
        .map(|label| {
            view! { <option value=label selected={label == current_label}>{label}</option> }
        })
        .collect_view()
}

fn needs_computer(phone: bool) -> &'static str {
    if phone { "Needs the computer" } else { "" }
}

/// Changes one or more fields of a routine. The list reloads inside `routines::edit`.
fn edit_routine(ui: Ui, id: RoutineId, change: EditRoutine) {
    spawn_local(async move {
        routines::edit(ui, &id, &change).await;
    });
}

fn delete_routine(ui: Ui, id: RoutineId) {
    spawn_local(async move {
        routines::remove(ui, &id).await;
    });
}
