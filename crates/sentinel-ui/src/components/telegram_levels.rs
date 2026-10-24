//! The Telegram rows that choose what the chat gets: levels, detail, events, and quiet hours.
//! Each change sends the whole settings update through `TelegramCtx::edit`. Every control locks
//! on a phone.

use leptos::prelude::*;
use sentinel_core::telegram_api::{SettingsUpdate, TelegramState};

use crate::components::telegram_card::TelegramCtx;
use crate::telegram::{
    self, Choice, DEFAULT_QUIET, DETAILS, EVENTS, LEVELS, QuietEdge, minutes_to_time,
    time_to_minutes,
};

const DETAIL_HINT: &str = "Telegram can read bot messages. Detail adds short diffs and agent snippets of at most 6 lines. File contents, keys, and Brain note text are never sent, and key-like text is hidden.";
const QUIET_HINT: &str = "Holds everything except approvals and budget alerts.";

#[component]
pub fn LevelToggles(ctx: TelegramCtx) -> impl IntoView {
    LEVELS
        .iter()
        .map(|choice| choice_row(ctx, *choice, levels_of, levels_edit))
        .collect_view()
}

#[component]
pub fn EventToggles(ctx: TelegramCtx) -> impl IntoView {
    EVENTS
        .iter()
        .map(|choice| choice_row(ctx, *choice, events_of, events_edit))
        .collect_view()
}

#[component]
pub fn DetailRow(ctx: TelegramCtx) -> impl IntoView {
    let select = view! {
        <select aria-label="Message detail" disabled=move || ctx.locked()
            prop:value=move || detail_key(ctx)
            on:change=move |event| {
                let key = event_target_value(&event);
                ctx.edit(|update| update.detail = key);
            }>
            {DETAILS
                .iter()
                .map(|choice| view! { <option value=choice.key>{choice.label}</option> })
                .collect_view()}
        </select>
    };
    field_row("Detail", DETAIL_HINT, select)
}

#[component]
pub fn QuietHours(ctx: TelegramCtx) -> impl IntoView {
    let on = Signal::derive(move || {
        ctx.state
            .with(|state| state.as_ref().is_some_and(|s| s.quiet_hours.is_some()))
    });
    let control = switch(
        on,
        Signal::derive(move || ctx.locked()),
        Callback::new(move |next: bool| {
            ctx.edit(|update| {
                update.quiet_hours = telegram::quiet_switched(update.quiet_hours, next);
            });
        }),
    );
    view! {
        {field_row("Quiet hours", QUIET_HINT, control)}
        <Show when=move || on.get() fallback=|| ()>
            <QuietWindow ctx />
        </Show>
    }
}

#[component]
fn QuietWindow(ctx: TelegramCtx) -> impl IntoView {
    let window = move || {
        ctx.state
            .with(|state| state.as_ref().and_then(|s| s.quiet_hours))
            .unwrap_or(DEFAULT_QUIET)
    };
    view! {
        <div class="row wrap" style="gap:.6em;padding:.4em 0">
            <span class="sub">"From"</span>
            <input class="field" type="time" aria-label="Quiet hours start"
                style="max-width:9em" disabled=move || ctx.locked()
                prop:value=move || minutes_to_time(window().start_minute)
                on:change=move |event| set_quiet_edge(ctx, QuietEdge::Start, &event_target_value(&event)) />
            <span class="sub">"until"</span>
            <input class="field" type="time" aria-label="Quiet hours end"
                style="max-width:9em" disabled=move || ctx.locked()
                prop:value=move || minutes_to_time(window().end_minute)
                on:change=move |event| set_quiet_edge(ctx, QuietEdge::End, &event_target_value(&event)) />
        </div>
    }
}

/// One row with a switch, for a level or an event. `names` says which list the key belongs to,
/// and `edit` changes that list.
fn choice_row(
    ctx: TelegramCtx,
    choice: Choice,
    names: fn(&TelegramState) -> &[String],
    edit: fn(&mut SettingsUpdate, &str, bool),
) -> impl IntoView {
    let on = listed(ctx, names, choice.key);
    let control = switch(
        on,
        Signal::derive(move || ctx.locked()),
        Callback::new(move |next: bool| ctx.edit(|update| edit(update, choice.key, next))),
    );
    field_row(choice.label, choice.hint, control)
}

fn levels_of(state: &TelegramState) -> &[String] {
    &state.levels
}

fn events_of(state: &TelegramState) -> &[String] {
    &state.events
}

fn levels_edit(update: &mut SettingsUpdate, key: &str, on: bool) {
    update.levels = telegram::levels_with(&update.levels, key, on);
}

fn events_edit(update: &mut SettingsUpdate, key: &str, on: bool) {
    update.events = telegram::events_with(&update.events, key, on);
}

/// Whether `key` is in the list `names` picks from the last known state.
fn listed(
    ctx: TelegramCtx,
    names: fn(&TelegramState) -> &[String],
    key: &'static str,
) -> Signal<bool> {
    Signal::derive(move || {
        ctx.state.with(|state| {
            state
                .as_ref()
                .is_some_and(|s| names(s).iter().any(|name| name.as_str() == key))
        })
    })
}

fn detail_key(ctx: TelegramCtx) -> String {
    ctx.state
        .with(|state| state.as_ref().map(|s| s.detail.clone()).unwrap_or_default())
}

fn set_quiet_edge(ctx: TelegramCtx, edge: QuietEdge, text: &str) {
    // A half-typed time is not a value yet. Nothing is sent until the input is complete.
    let Some(minute) = time_to_minutes(text) else {
        return;
    };
    ctx.edit(|update| {
        update.quiet_hours = Some(telegram::quiet_edited(update.quiet_hours, edge, minute));
    });
}

/// The prototype's `tog` switch. `Toggle` has no locked state, so the markup lives here.
fn switch(on: Signal<bool>, locked: Signal<bool>, on_change: Callback<bool>) -> impl IntoView {
    view! {
        <button class="tog" role="switch"
            aria-checked=move || on.get().to_string()
            disabled=move || locked.get()
            on:click=move |_| on_change.run(!on.get_untracked())></button>
    }
}

/// One settings row: words on the left, the control on the right. The prototype's `fld`.
fn field_row(label: &'static str, hint: &'static str, control: impl IntoView) -> impl IntoView {
    view! {
        <div class="fld">
            <div class="l">
                <b>{label}</b>
                {(!hint.is_empty()).then(|| view! { <span>{hint}</span> })}
            </div>
            {control}
        </div>
    }
}
