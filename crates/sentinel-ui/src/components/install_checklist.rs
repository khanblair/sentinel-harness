//! `InstallChecklist`: the install check rows, each found or missing, with the command that
//! installs what is missing (`POST /api/doctor`, prototype `INST.checks`).

use leptos::prelude::*;
use sentinel_core::api::{CheckRow, CheckStatus};

use crate::components::repo_checks::{CheckShape, describe_check};

/// True when every row is found. An empty check is not found, so Continue stays off.
pub fn all_found(rows: &[CheckRow]) -> bool {
    !rows.is_empty() && rows.iter().all(|row| row.status == CheckStatus::Ok)
}

/// The commands the user must run for the rows that are missing. The daemon puts the command
/// in `detail` for a missing row.
pub fn missing_commands(rows: &[CheckRow]) -> Vec<String> {
    rows.iter()
        .filter(|row| row.status != CheckStatus::Ok)
        .map(|row| row.detail.clone())
        .filter(|command| !command.is_empty())
        .collect()
}

pub fn found_label(row: &CheckRow) -> &'static str {
    if row.status == CheckStatus::Ok {
        "Found"
    } else {
        "Missing"
    }
}

#[component]
pub fn InstallChecklist(rows: Vec<CheckRow>) -> impl IntoView {
    rows.into_iter()
        .map(|row| {
            let (class, _) = describe_check(&row);
            let label = found_label(&row);
            view! {
                <div class="chk">
                    <CheckShape class=class />
                    <div class="n">
                        <b style="font-weight:500">{row.name.clone()}</b>
                        <small class="mono">{row.detail.clone()}</small>
                    </div>
                    <span class="sub">{label}</span>
                </div>
            }
        })
        .collect_view()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, status: CheckStatus, command: &str) -> CheckRow {
        CheckRow {
            name: name.to_string(),
            status,
            detail: command.to_string(),
            fix: None,
        }
    }

    #[test]
    fn an_empty_check_is_not_all_found() {
        assert!(!all_found(&[]));
    }

    #[test]
    fn all_found_needs_every_row_ok() {
        let rows = [
            row("git", CheckStatus::Ok, "plumbing commands only"),
            row(
                "Tailscale",
                CheckStatus::NeedsAction,
                "brew install tailscale",
            ),
        ];
        assert!(!all_found(&rows));
        assert!(all_found(&rows[..1]));
    }

    #[test]
    fn missing_rows_give_their_install_command() {
        let rows = [
            row("git", CheckStatus::Ok, "plumbing commands only"),
            row(
                "wasm32 target",
                CheckStatus::Invalid,
                "rustup target add wasm32-unknown-unknown",
            ),
        ];
        assert_eq!(
            missing_commands(&rows),
            ["rustup target add wasm32-unknown-unknown"]
        );
    }

    #[test]
    fn a_missing_row_with_no_command_gives_nothing_to_run() {
        let rows = [row("Tailscale", CheckStatus::NeedsAction, "")];
        assert_eq!(missing_commands(&rows), Vec::<String>::new());
    }

    #[test]
    fn a_row_says_found_only_when_ok() {
        assert_eq!(found_label(&row("git", CheckStatus::Ok, "")), "Found");
        assert_eq!(
            found_label(&row("git", CheckStatus::Invalid, "")),
            "Missing"
        );
    }
}
