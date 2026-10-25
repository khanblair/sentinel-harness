//! `ResetSection`: the Reset block at the end of General. Each row clears one kind of data, and the
//! last row clears all of it. Clearing needs the computer, so a phone sees one note instead.

use leptos::prelude::*;
use sentinel_core::api::DataScope;

use crate::components::confirm_clear::ClearAction;
use crate::state::{Modal, Ui, use_ui};

/// One row of the Reset block: its name, what it removes, and what stays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetRow {
    pub action: ClearAction,
    pub name: &'static str,
    pub goes: &'static str,
    pub keeps: &'static str,
}

impl ResetRow {
    fn new(scope: DataScope, name: &'static str, goes: &'static str, keeps: &'static str) -> Self {
        Self {
            action: ClearAction::One(scope),
            name,
            goes,
            keeps,
        }
    }
}

/// The rows that clear one project. The active project is the one they touch.
pub fn project_rows() -> Vec<ResetRow> {
    vec![
        ResetRow::new(
            DataScope::Chats,
            "Chats",
            "Every chat and its uploads. One empty chat stays.",
            "Run history, settings, and your project files stay.",
        ),
        ResetRow::new(
            DataScope::Brain,
            "Brain",
            "The notes and their index. The Brain builds itself again.",
            "Chats and your project files stay.",
        ),
        ResetRow::new(
            DataScope::History,
            "Run history",
            "Tasks, reservations, checkpoints, approvals, costs, and the report.",
            "Chats and your project files stay.",
        ),
        ResetRow::new(
            DataScope::Settings,
            "Project settings",
            "Back to the defaults. The agents are found again.",
            "Chats, notes, and your project files stay.",
        ),
    ]
}

/// The rows that clear app-wide data. They live in the primary project.
pub fn app_rows() -> Vec<ResetRow> {
    vec![
        ResetRow::new(
            DataScope::Devices,
            "Paired phones",
            "Every paired phone. This computer stays signed in.",
            "Chats and your project files stay.",
        ),
        ResetRow::new(
            DataScope::Keys,
            "Saved keys",
            "The keys in the system keychain.",
            "Chats and your project files stay.",
        ),
        ResetRow::new(
            DataScope::Telegram,
            "Telegram",
            "Its settings and log.",
            "Chats and your project files stay.",
        ),
        ResetRow::new(
            DataScope::Routines,
            "Routines",
            "The routines and their briefs.",
            "Chats and your project files stay.",
        ),
        ResetRow::new(
            DataScope::Projects,
            "Extra projects",
            "The list of extra projects. The folders stay.",
            "Chats and your project files stay.",
        ),
        ResetRow::new(
            DataScope::Onboarding,
            "Setup wizard",
            "The setup steps show again.",
            "Chats, keys, and your project files stay.",
        ),
    ]
}

/// The last row. It clears every project and the app, and it says so in one line.
pub fn danger_row() -> ResetRow {
    ResetRow {
        action: ClearAction::Everything,
        name: "Clear everything and start over",
        goes: "All of the above, in every project. The setup wizard starts again. \
               Your project files are not touched.",
        keeps: "",
    }
}

/// The title of the confirm dialog. The danger row's name already reads as a title.
pub fn confirm_title(row: &ResetRow) -> String {
    match row.action {
        ClearAction::Everything => format!("{}?", row.name),
        ClearAction::One(_) => format!("Clear {}?", row.name.to_lowercase()),
    }
}

/// The body of the confirm dialog: what goes, then what stays.
pub fn confirm_body(row: &ResetRow) -> String {
    format!("{} {}", row.goes, row.keeps).trim_end().to_owned()
}

#[component]
pub fn ResetSection() -> impl IntoView {
    let ui = use_ui();
    move || {
        if ui.on_phone() {
            view! { <p class="note">"Clearing data needs the computer."</p> }.into_any()
        } else {
            view! { <ResetLists /> }.into_any()
        }
    }
}

#[component]
fn ResetLists() -> impl IntoView {
    view! {
        <div class="psec">
            <h3>"Reset"</h3>
            <ResetGroup heading="This project" rows=project_rows() />
            <ResetGroup heading="The app" rows=app_rows() />
            <ResetRowView row=danger_row() />
        </div>
    }
}

#[component]
fn ResetGroup(heading: &'static str, rows: Vec<ResetRow>) -> impl IntoView {
    let items = rows
        .into_iter()
        .map(|row| view! { <ResetRowView row=row /> })
        .collect_view();
    view! {
        <h5>{heading}</h5>
        {items}
    }
}

#[component]
fn ResetRowView(row: ResetRow) -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="card row sp wrap">
            <div>
                <h4>{row.name}</h4>
                <div class="sub">{row.goes}</div>
            </div>
            <button class="btn sm dng" on:click=move |_| ask(ui, &row)>"Clear"</button>
        </div>
    }
}

/// Opens the confirm dialog for one row. Nothing is cleared until the owner confirms.
fn ask(ui: Ui, row: &ResetRow) {
    ui.modal.set(Some(Modal::ConfirmClear {
        title: confirm_title(row),
        body: confirm_body(row),
        action: row.action,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_project_rows_clear_the_four_project_scopes_in_order() {
        let actions: Vec<ClearAction> = project_rows().iter().map(|row| row.action).collect();
        assert_eq!(actions, DataScope::PROJECT.map(ClearAction::One).to_vec());
    }

    #[test]
    fn the_app_rows_clear_the_six_app_scopes_in_order() {
        let actions: Vec<ClearAction> = app_rows().iter().map(|row| row.action).collect();
        assert_eq!(actions, DataScope::APP.map(ClearAction::One).to_vec());
    }

    #[test]
    fn the_danger_row_clears_everything_and_has_no_second_line() {
        let row = danger_row();
        assert_eq!(row.action, ClearAction::Everything);
        assert_eq!(row.keeps, "");
        assert!(row.goes.ends_with("Your project files are not touched."));
    }

    #[test]
    fn a_project_row_title_names_the_row_in_lower_case() {
        assert_eq!(confirm_title(&project_rows()[2]), "Clear run history?");
    }

    #[test]
    fn the_danger_row_title_is_its_own_name() {
        assert_eq!(
            confirm_title(&danger_row()),
            "Clear everything and start over?"
        );
    }

    #[test]
    fn a_row_body_says_what_goes_then_what_stays() {
        assert_eq!(
            confirm_body(&project_rows()[0]),
            "Every chat and its uploads. One empty chat stays. \
             Run history, settings, and your project files stay."
        );
    }

    #[test]
    fn the_danger_body_has_no_trailing_space() {
        let row = danger_row();
        assert_eq!(confirm_body(&row), row.goes);
    }
}
