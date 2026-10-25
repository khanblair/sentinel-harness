//! `ConfirmClear`: asks before a data reset runs. The reset that clears everything needs the owner
//! to type `RESET` first. Resets that touch the app, the wizard, or the keys reload the page, so
//! the next screen comes from the daemon's new state.

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::{ClearData, DataScope};
use serde_json::Value;

use crate::api::{self, ApiFailure};
use crate::components::icon::Icon;
use crate::state::{ToastKind, Ui, refresh, use_ui};

/// How long the "Cleared." toast stays up before a reloading reset reloads the page.
const RELOAD_DELAY_MS: u32 = 1200;

/// What one confirmed reset removes: one scope, or everything in every project and in the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearAction {
    One(DataScope),
    Everything,
}

/// The route for an action. Project scopes go to the active project. App scopes go to the app.
pub fn action_path(action: ClearAction) -> &'static str {
    match action {
        ClearAction::One(scope) if scope.is_project() => "/api/data/project",
        ClearAction::One(_) => "/api/data/app",
        ClearAction::Everything => "/api/data/all",
    }
}

/// The JSON body of a scoped reset, with the scopes in the order given.
pub fn body_for(scopes: &[DataScope]) -> ClearData {
    ClearData {
        scopes: scopes.to_vec(),
    }
}

/// Chats, history, and settings only change what the open page shows, so a snapshot refresh is
/// enough. Every other reset changes the app, the wizard, or the keys, so the page reloads.
pub fn needs_reload(action: ClearAction) -> bool {
    !matches!(
        action,
        ClearAction::One(DataScope::Chats | DataScope::History | DataScope::Settings)
    )
}

/// Only the reset that clears everything asks the owner to type the word.
pub fn needs_reset_word(action: ClearAction) -> bool {
    action == ClearAction::Everything
}

/// The typed word must be `RESET`. Spaces at the ends do not count, and case does.
pub fn reset_word_ok(text: &str) -> bool {
    text.trim() == "RESET"
}

#[component]
pub fn ConfirmClear(title: String, body: String, action: ClearAction) -> impl IntoView {
    let ui = use_ui();
    let typed = RwSignal::new(String::new());
    let word_missing = move || needs_reset_word(action) && !reset_word_ok(&typed.get());
    view! {
        <div class="modal-hd">
            <span style="color:var(--danger)"><Icon name="trash" size=18 /></span>
            <h3>{title}</h3>
        </div>
        <div class="modal-body" style="padding:1em 1.1em">
            <p style="margin:0;line-height:1.6">{body}</p>
            {needs_reset_word(action).then(|| view! { <ResetWordField typed=typed /> })}
        </div>
        <div class="modal-ft">
            <button class="btn" on:click=move |_| ui.modal.set(None)>"Cancel"</button>
            <button class="btn dng" disabled=word_missing on:click=move |_| run_clear(ui, action)>
                "Clear"
            </button>
        </div>
    }
}

#[component]
fn ResetWordField(typed: RwSignal<String>) -> impl IntoView {
    view! {
        <label class="fld col" style="margin-top:1em">
            <span>"Type RESET to confirm."</span>
            <input
                type="text"
                autocomplete="off"
                prop:value=move || typed.get()
                on:input=move |event| typed.set(event_target_value(&event))
            />
        </label>
    }
}

/// Closes the dialog, runs the reset, and shows the result. A failed reset keeps the page as it
/// was and shows the daemon's message.
fn run_clear(ui: Ui, action: ClearAction) {
    ui.modal.set(None);
    spawn_local(async move {
        match send_clear(action).await {
            Ok(_) => finish_clear(ui, action).await,
            Err(failure) => ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

/// The everything reset has no body. The scoped resets send their one scope.
async fn send_clear(action: ClearAction) -> Result<Value, ApiFailure> {
    let path = action_path(action);
    match action {
        ClearAction::One(scope) => api::post(path, &body_for(&[scope])).await,
        ClearAction::Everything => api::post_empty(path).await,
    }
}

/// Shows the toast. A reloading reset waits so the toast can be read before the page reloads.
async fn finish_clear(ui: Ui, action: ClearAction) {
    ui.show_toast("Cleared.", ToastKind::Ok);
    if needs_reload(action) {
        TimeoutFuture::new(RELOAD_DELAY_MS).await;
        reload_page();
    } else {
        refresh(ui);
    }
}

fn reload_page() {
    if let Some(window) = web_sys::window() {
        let _ = window.location().reload();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_scope_clears_through_the_project_route() {
        assert_eq!(
            action_path(ClearAction::One(DataScope::Chats)),
            "/api/data/project"
        );
        assert_eq!(
            action_path(ClearAction::One(DataScope::Settings)),
            "/api/data/project"
        );
    }

    #[test]
    fn an_app_scope_clears_through_the_app_route() {
        assert_eq!(
            action_path(ClearAction::One(DataScope::Keys)),
            "/api/data/app"
        );
        assert_eq!(
            action_path(ClearAction::One(DataScope::Onboarding)),
            "/api/data/app"
        );
    }

    #[test]
    fn everything_clears_through_the_all_route() {
        assert_eq!(action_path(ClearAction::Everything), "/api/data/all");
    }

    #[test]
    fn the_body_keeps_the_scopes_in_the_order_given() {
        let body = body_for(&[DataScope::Brain, DataScope::Chats]);
        assert_eq!(body.scopes, vec![DataScope::Brain, DataScope::Chats]);
    }

    #[test]
    fn chats_history_and_settings_refresh_the_page_without_a_reload() {
        assert!(!needs_reload(ClearAction::One(DataScope::Chats)));
        assert!(!needs_reload(ClearAction::One(DataScope::History)));
        assert!(!needs_reload(ClearAction::One(DataScope::Settings)));
    }

    #[test]
    fn brain_app_scopes_and_everything_reload_the_page() {
        assert!(needs_reload(ClearAction::One(DataScope::Brain)));
        assert!(needs_reload(ClearAction::One(DataScope::Keys)));
        assert!(needs_reload(ClearAction::One(DataScope::Onboarding)));
        assert!(needs_reload(ClearAction::Everything));
    }

    #[test]
    fn only_the_everything_reset_asks_for_the_reset_word() {
        assert!(needs_reset_word(ClearAction::Everything));
        assert!(!needs_reset_word(ClearAction::One(DataScope::Chats)));
    }

    #[test]
    fn the_reset_word_must_be_upper_case_and_may_have_spaces_at_the_ends() {
        assert!(reset_word_ok("RESET"));
        assert!(reset_word_ok("  RESET\n"));
        assert!(!reset_word_ok("reset"));
        assert!(!reset_word_ok("RESETS"));
        assert!(!reset_word_ok(""));
    }
}
