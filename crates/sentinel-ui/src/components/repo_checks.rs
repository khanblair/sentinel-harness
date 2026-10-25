//! `RepoChecks`: the rows of a check (the repository's git, backlog.md, and clean tree, or the
//! install check) with the fix button the daemon offers. Also the shared row look for
//! `InstallChecklist` and `PairChecklist` (`docs/ui-registry.md`, Settings and Setup).

use leptos::prelude::*;
use sentinel_core::api::{CheckRow, CheckStatus};

use crate::components::status_badge::shape_markup;

/// The color class and the text of one row. A row without detail gets a plain word.
pub fn describe_check(row: &CheckRow) -> (&'static str, String) {
    let (class, fallback) = match row.status {
        CheckStatus::Ok => ("ok", "Found."),
        CheckStatus::NeedsAction => ("warn", "Needs action."),
        CheckStatus::Invalid => ("danger", "Not usable."),
    };
    let text = if row.detail.is_empty() {
        fallback.to_string()
    } else {
        row.detail.clone()
    };
    (class, text)
}

/// The shape for a color class. Color alone is not enough, so each class has its own shape.
pub fn check_shape(class: &str) -> &'static str {
    match class {
        "ok" => "dot",
        "warn" => "ring",
        _ => "oct",
    }
}

/// The clean tree row and the label it shows when `backlog.md` is its only change. The daemon
/// sends the same words.
const CLEAN_ROW: &str = "Clean working tree";
const COMMIT_LABEL: &str = "Commit backlog.md";

/// What a fix button does. The backlog row creates a starter file. The clean tree row commits
/// that file when it is the only change. Every other fix re-runs the check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixAction {
    CreateBacklog,
    CommitBacklog,
    Recheck,
}

pub fn fix_action(row_name: &str, fix_label: &str) -> FixAction {
    if row_name == CLEAN_ROW && fix_label == COMMIT_LABEL {
        FixAction::CommitBacklog
    } else if row_name.to_lowercase().contains("backlog") {
        FixAction::CreateBacklog
    } else {
        FixAction::Recheck
    }
}

#[component]
pub fn CheckShape(class: &'static str) -> impl IntoView {
    let shape = check_shape(class);
    view! {
        <span style=format!("color:var(--{class})")>
            <svg class="shp" viewBox="0 0 12 12" aria-hidden="true" inner_html=shape_markup(shape, class)></svg>
        </span>
    }
}

/// One row of a repository check. The fix button runs `on_fix` with the row name and the fix label.
fn repo_check_view(row: CheckRow, on_fix: Callback<(String, String)>) -> impl IntoView {
    let (class, text) = describe_check(&row);
    let CheckRow { name, fix, .. } = row;
    let fix = fix.map(|label| {
        let (name, request) = (name.clone(), label.clone());
        view! {
            <button class="btn sm" on:click=move |_| on_fix.run((name.clone(), request.clone()))>{label}</button>
        }
    });
    view! {
        <div class="chk">
            <CheckShape class=class />
            <div class="n">
                <b style="font-weight:500">{name}</b>
                <small>{text}</small>
            </div>
            {fix}
        </div>
    }
}

/// The rows of one check. The parent owns the rows and the fix actions.
#[component]
pub fn RepoChecks(rows: Vec<CheckRow>, on_fix: Callback<(String, String)>) -> impl IntoView {
    rows.into_iter()
        .map(|row| repo_check_view(row, on_fix))
        .collect_view()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(status: CheckStatus, detail: &str) -> CheckRow {
        CheckRow {
            name: "git".to_string(),
            status,
            detail: detail.to_string(),
            fix: None,
        }
    }

    #[test]
    fn a_passing_row_is_green_and_keeps_its_detail() {
        assert_eq!(
            describe_check(&row(CheckStatus::Ok, "A .git folder was found.")),
            ("ok", "A .git folder was found.".to_string())
        );
    }

    #[test]
    fn a_row_that_needs_action_is_amber() {
        assert_eq!(
            describe_check(&row(CheckStatus::NeedsAction, "")),
            ("warn", "Needs action.".to_string())
        );
    }

    #[test]
    fn an_invalid_row_is_red_with_its_detail() {
        assert_eq!(
            describe_check(&row(CheckStatus::Invalid, "Not a folder.")),
            ("danger", "Not a folder.".to_string())
        );
    }

    #[test]
    fn each_color_class_has_its_own_shape() {
        assert_eq!(check_shape("ok"), "dot");
        assert_eq!(check_shape("warn"), "ring");
        assert_eq!(check_shape("danger"), "oct");
    }

    #[test]
    fn only_the_backlog_row_creates_a_file() {
        assert_eq!(
            fix_action("backlog.md", "Create a starter backlog"),
            FixAction::CreateBacklog
        );
    }

    #[test]
    fn the_clean_tree_row_commits_the_backlog_only_with_its_commit_label() {
        assert_eq!(
            fix_action("Clean working tree", "Commit backlog.md"),
            FixAction::CommitBacklog
        );
        assert_eq!(
            fix_action("Clean working tree", "Stash your changes"),
            FixAction::Recheck
        );
    }

    #[test]
    fn a_row_without_a_special_fix_only_rechecks() {
        assert_eq!(fix_action("Git repository", "Retry"), FixAction::Recheck);
    }
}
