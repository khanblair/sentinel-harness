//! The empty stream: what the run does, and the three first actions.

use leptos::prelude::*;
use sentinel_core::{PermissionMode, RepoInfo};

use crate::components::icon::Icon;
use crate::state::{Modal, use_ui};
use crate::views::composer::launch;

/// The sentence about the backlog, from the repository the daemon works on.
pub fn backlog_sentence(repo: Option<&RepoInfo>) -> String {
    match repo {
        Some(repo) if repo.has_backlog => {
            format!("Backlog: {}, {} tasks.", repo.name, repo.backlog_tasks)
        }
        Some(_) => "This repository has no backlog file yet.".to_owned(),
        None => "Choose a repository to start.".to_owned(),
    }
}

#[component]
pub fn EmptyRunState() -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="empty">
            <h2>"Run the backlog"</h2>
            <p>
                "Sentinel plans the work, reserves files for each agent so two agents never write the same file, and only calls a task done when the tests say so. "
                {move || ui.snapshot.with(|s| backlog_sentence(s.repo.as_ref()))}
            </p>
            <div class="sugg">
                <button on:click=move |_| launch(ui, None, None)>
                    <span style="color:var(--accent)"><Icon name="play2" size=20 /></span>
                    <div>
                        <b>"Start the run"</b>
                        <span>{move || format!("Uses your current permission mode: {}.", ui.snapshot.with(|s| s.permission_mode.label()))}</span>
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

    fn repo(has_backlog: bool) -> RepoInfo {
        RepoInfo {
            path: "/repo".to_owned(),
            name: "fee-ledger-api".to_owned(),
            is_git: true,
            has_backlog,
            clean: true,
            backlog_tasks: 12,
        }
    }

    #[test]
    fn backlog_sentence_names_the_repo_and_task_count() {
        assert_eq!(
            backlog_sentence(Some(&repo(true))),
            "Backlog: fee-ledger-api, 12 tasks."
        );
        assert!(backlog_sentence(Some(&repo(false))).contains("no backlog"));
        assert!(backlog_sentence(None).starts_with("Choose"));
    }
}
