//! The empty stream of a chat: what to do first, and three first actions.

use leptos::prelude::*;
use sentinel_core::{PermissionMode, RepoInfo};

use crate::components::icon::Icon;
use crate::state::{Modal, use_ui};
use crate::views::composer::launch;

/// The sub text of the backlog card: how many tasks the project's backlog holds.
pub fn backlog_card_text(repo: Option<&RepoInfo>) -> String {
    let tasks = repo.map_or(0, |repo| repo.backlog_tasks);
    format!("{tasks} tasks in this project. Uses your current permission mode.")
}

#[component]
pub fn EmptyChatState() -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="empty">
            <h2>"What should we build?"</h2>
            <p>
                "Describe the work in plain words, or attach a backlog. Sentinel plans it, reserves files for each agent so two agents never write the same file, and only calls a task done when the tests say so."
            </p>
            <div class="sugg">
                <button on:click=move |_| launch(ui, None, None)>
                    <span style="color:var(--accent)"><Icon name="play2" size=20 /></span>
                    <div>
                        <b>"Use backlog.md"</b>
                        <span>{move || ui.snapshot.with(|s| backlog_card_text(s.repo.as_ref()))}</span>
                    </div>
                </button>
                <button on:click=move |_| launch(ui, Some(PermissionMode::PlanOnly), None)>
                    <span><Icon name="eye" size=20 /></span>
                    <div>
                        <b>"Plan only first"</b>
                        <span>"See scopes, dependencies and contradictions. Nothing is written until you approve."</span>
                    </div>
                </button>
                <button on:click=move |_| ui.modal.set(Some(Modal::Guide))>
                    <span><Icon name="book" size=20 /></span>
                    <div>
                        <b>"Take the tour"</b>
                        <span>"Two minutes on the sidebar, panels, agent switcher and permissions."</span>
                    </div>
                </button>
            </div>
            <p style="margin-top:1em;font-size:.9em">
                "Type " <span class="k">"/"</span> " in the box below for commands such as "
                <span class="k">"/pause"</span> " and " <span class="k">"/cap 1.50"</span> "."
            </p>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(backlog_tasks: u32) -> RepoInfo {
        RepoInfo {
            path: "/repo".to_owned(),
            name: "fee-ledger-api".to_owned(),
            is_git: true,
            has_backlog: true,
            clean: true,
            backlog_tasks,
        }
    }

    #[test]
    fn the_backlog_card_counts_the_project_tasks() {
        assert_eq!(
            backlog_card_text(Some(&repo(12))),
            "12 tasks in this project. Uses your current permission mode."
        );
    }

    #[test]
    fn the_backlog_card_counts_zero_without_a_repository() {
        assert!(backlog_card_text(None).starts_with("0 tasks"));
    }
}
