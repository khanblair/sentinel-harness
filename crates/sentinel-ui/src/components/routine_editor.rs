//! `RoutineEditor`: the dialog body that adds a reminder or edits one (`docs/product-scope.md`,
//! F25). The form checks its own fields first. The daemon checks them again.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::routines::{EditRoutine, NewRoutine, Routine, RoutineKind, WEEKDAYS};

use crate::components::icon::Icon;
use crate::components::routine_row::{DaySelect, RoutineSwitch, TimeSelect};
use crate::routines::{self, TEXT_LIMIT};
use crate::state::{ToastKind, Ui, use_ui};

/// The first time a new reminder offers: 09:00, in minutes after midnight.
const DEFAULT_MINUTE: u16 = 9 * 60;

/// Which dialog the editor shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorMode {
    Add,
    Edit(Routine),
}

impl EditorMode {
    fn kind(&self) -> RoutineKind {
        match self {
            EditorMode::Add => RoutineKind::Reminder,
            EditorMode::Edit(routine) => routine.kind,
        }
    }

    fn title(&self) -> &'static str {
        match self {
            EditorMode::Add => "Add a reminder",
            EditorMode::Edit(routine) if routine.kind == RoutineKind::BacklogRun => {
                "Edit backlog run"
            }
            EditorMode::Edit(_) => "Edit reminder",
        }
    }

    fn form(&self) -> RoutineForm {
        match self {
            EditorMode::Add => RoutineForm::new_reminder(),
            EditorMode::Edit(routine) => RoutineForm::from_routine(routine),
        }
    }
}

/// What the form holds. It turns into a `NewRoutine` or an `EditRoutine`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutineForm {
    pub name: String,
    pub minute_of_day: u16,
    pub days: u8,
    pub text: String,
    pub to_telegram: bool,
}

impl RoutineForm {
    /// A new reminder: named Reminder, at 09:00 on weekdays, sent to Telegram, with no text yet.
    pub fn new_reminder() -> Self {
        Self {
            name: "Reminder".to_owned(),
            minute_of_day: DEFAULT_MINUTE,
            days: WEEKDAYS,
            text: String::new(),
            to_telegram: true,
        }
    }

    pub fn from_routine(routine: &Routine) -> Self {
        Self {
            name: routine.name.clone(),
            minute_of_day: routine.minute_of_day,
            days: routine.days,
            text: routine.text.clone(),
            to_telegram: routine.to_telegram,
        }
    }

    /// The first problem with the form, in words for a toast. A backlog run has no text to check.
    pub fn check(&self, kind: RoutineKind) -> Result<(), &'static str> {
        if self.name.trim().is_empty() {
            return Err("Give it a name first.");
        }
        if self.text.chars().count() > TEXT_LIMIT {
            return Err("The text can be up to 500 characters.");
        }
        if kind == RoutineKind::Reminder && self.text.trim().is_empty() {
            return Err("Write what the reminder should say.");
        }
        Ok(())
    }

    pub fn to_new(&self) -> NewRoutine {
        NewRoutine {
            name: self.name.trim().to_owned(),
            kind: RoutineKind::Reminder,
            minute_of_day: self.minute_of_day,
            days: self.days,
            to_telegram: self.to_telegram,
            text: self.text.trim().to_owned(),
            backlog: String::new(),
        }
    }

    /// Every field of the form. The text is left out for a backlog run, which has none.
    pub fn to_edit(&self, kind: RoutineKind) -> EditRoutine {
        let text = (kind == RoutineKind::Reminder).then(|| self.text.trim().to_owned());
        EditRoutine {
            name: Some(self.name.trim().to_owned()),
            minute_of_day: Some(self.minute_of_day),
            days: Some(self.days),
            enabled: None,
            to_telegram: Some(self.to_telegram),
            text,
            backlog: None,
        }
    }
}

#[component]
pub fn RoutineEditor(mode: EditorMode, on_close: Callback<()>) -> impl IntoView {
    let ui = use_ui();
    let form = RwSignal::new(mode.form());
    let kind = mode.kind();
    let title = mode.title();
    view! {
        <div class="modal-hd">
            <Icon name="edit" size=18 />
            <h3>{title}</h3>
        </div>
        <div class="modal-body" style="padding:1em 1.1em">
            <EditorFields form kind />
        </div>
        <div class="modal-ft">
            <button class="btn" on:click=move |_| on_close.run(())>"Cancel"</button>
            <button class="btn pri" on:click=move |_| save(ui, &mode, form.get_untracked(), on_close)>
                "Save"
            </button>
        </div>
    }
}

#[component]
fn EditorFields(form: RwSignal<RoutineForm>, kind: RoutineKind) -> impl IntoView {
    view! {
        <label class="sub" for="routine-name">"Name"</label>
        <input class="field" id="routine-name" prop:value=move || form.with(|f| f.name.clone())
            on:input=move |ev| form.update(|f| f.name = event_target_value(&ev)) />
        <div class="row wrap" style="margin:.8em 0">
            <span class="row">
                <label class="sub">"At"</label>
                <TimeSelect
                    minute=move || form.with(|f| f.minute_of_day)
                    disabled=false
                    on_pick=move |minute| form.update(|f| f.minute_of_day = minute)
                />
            </span>
            <DaySelect
                days=move || form.with(|f| f.days)
                disabled=false
                on_pick=move |days| form.update(|f| f.days = days)
            />
        </div>
        {(kind == RoutineKind::Reminder).then(|| view! { <ReminderText form /> })}
        <div class="row">
            <RoutineSwitch
                checked=move || form.with(|f| f.to_telegram)
                label="Send to Telegram"
                disabled=false
                on_change=move |value| form.update(|f| f.to_telegram = value)
            />
            <span class="sub">"Send to Telegram"</span>
        </div>
    }
}

#[component]
fn ReminderText(form: RwSignal<RoutineForm>) -> impl IntoView {
    view! {
        <label class="sub" for="routine-text">"Text"</label>
        <textarea class="field" id="routine-text" rows="4" maxlength=TEXT_LIMIT
            prop:value=move || form.with(|f| f.text.clone())
            on:input=move |ev| form.update(|f| f.text = event_target_value(&ev))></textarea>
    }
}

/// Saves the form. A failed check shows a toast and the dialog stays open.
fn save(ui: Ui, mode: &EditorMode, form: RoutineForm, on_close: Callback<()>) {
    let kind = mode.kind();
    if let Err(message) = form.check(kind) {
        ui.show_toast(message, ToastKind::Warn);
        return;
    }
    let target = mode.clone();
    spawn_local(async move {
        let saved = match &target {
            EditorMode::Add => routines::create(ui, &form.to_new()).await,
            EditorMode::Edit(routine) => routines::edit(ui, &routine.id, &form.to_edit(kind)).await,
        };
        if saved {
            on_close.run(());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::RoutineId;

    fn form(name: &str, text: &str) -> RoutineForm {
        RoutineForm {
            name: name.to_owned(),
            text: text.to_owned(),
            ..RoutineForm::new_reminder()
        }
    }

    #[test]
    fn a_reminder_needs_a_name_and_text() {
        assert_eq!(
            form("  ", "Ship it").check(RoutineKind::Reminder),
            Err("Give it a name first.")
        );
        assert_eq!(
            form("Stand up", "  ").check(RoutineKind::Reminder),
            Err("Write what the reminder should say.")
        );
        assert_eq!(
            form("Stand up", "Ship it").check(RoutineKind::Reminder),
            Ok(())
        );
    }

    #[test]
    fn text_longer_than_the_limit_is_refused() {
        let long = "a".repeat(TEXT_LIMIT + 1);
        assert_eq!(
            form("Stand up", &long).check(RoutineKind::Reminder),
            Err("The text can be up to 500 characters.")
        );
    }

    #[test]
    fn a_backlog_run_needs_no_text() {
        assert_eq!(form("Night run", "").check(RoutineKind::BacklogRun), Ok(()));
    }

    #[test]
    fn a_backlog_edit_leaves_the_text_out() {
        let edit = form("Night run", "ignored").to_edit(RoutineKind::BacklogRun);
        assert_eq!(edit.text, None);
        assert_eq!(edit.name.as_deref(), Some("Night run"));
    }

    #[test]
    fn a_new_reminder_is_trimmed_and_sent_as_a_reminder() {
        let new = form("  Stand up ", " Ship it ").to_new();
        assert_eq!(new.name, "Stand up");
        assert_eq!(new.text, "Ship it");
        assert_eq!(new.kind, RoutineKind::Reminder);
        assert_eq!(new.minute_of_day, DEFAULT_MINUTE);
    }

    #[test]
    fn editing_starts_from_the_routine_fields() {
        let routine = Routine {
            id: RoutineId::new("r-1"),
            name: "Stand up".to_owned(),
            kind: RoutineKind::Reminder,
            minute_of_day: 600,
            days: WEEKDAYS,
            enabled: true,
            to_telegram: false,
            text: "Ship it".to_owned(),
            backlog: String::new(),
            last_fired_on: None,
        };
        let start = RoutineForm::from_routine(&routine);
        assert_eq!(start.minute_of_day, 600);
        assert!(!start.to_telegram);
        assert_eq!(start.text, "Ship it");
    }
}
