//! The Telegram panel. Once the chat is paired it shows the bot, the detail, the levels that are
//! on, and the Activity list. Before that it shows a card that opens Settings, Integrations.

use leptos::prelude::*;
use sentinel_core::telegram_api::TelegramState;

use crate::components::icon::Icon;
use crate::components::telegram_card::{TelegramCtx, poll_while_waiting};
use crate::components::telegram_log::TelegramLog;
use crate::state::{Modal, Ui, use_ui};
use crate::telegram::{DETAILS, LEVELS, label_of};

#[component]
pub fn TelegramPanel() -> impl IntoView {
    let ctx = TelegramCtx::load(use_ui());
    poll_while_waiting(ctx);
    view! {
        <Show
            when=move || ctx.state.with(|s| s.as_ref().is_some_and(|s| s.connected))
            fallback=move || view! { <NotPaired ctx /> }
        >
            <PairedPanel ctx />
        </Show>
    }
}

/// The labels of the levels that are on. They become the chips under the bot name.
pub fn active_level_labels(state: &TelegramState) -> Vec<&'static str> {
    LEVELS
        .iter()
        .filter(|choice| {
            state
                .levels
                .iter()
                .any(|level| level.as_str() == choice.key)
        })
        .map(|choice| choice.label)
        .collect()
}

#[component]
fn NotPaired(ctx: TelegramCtx) -> impl IntoView {
    let hint = move || {
        let has_bot = ctx
            .state
            .with(|s| s.as_ref().is_some_and(|s| s.token_saved));
        if has_bot {
            "Send /start to your bot to pair a chat."
        } else {
            "Add a bot token, then pair your chat."
        }
    };
    view! {
        <div class="card">
            <b>"Telegram is not paired"</b>
            <div class="sub" style="margin:.4em 0 .7em">{hint}</div>
            <button class="btn pri" on:click=move |_| open_integrations(ctx.ui)>
                <Icon name="send" size=14 />
                " Set up Telegram"
            </button>
        </div>
    }
}

#[component]
fn PairedPanel(ctx: TelegramCtx) -> impl IntoView {
    let bot = move || {
        ctx.state
            .with(|s| s.as_ref().and_then(|s| s.bot_username.clone()))
    };
    let detail = move || {
        ctx.state
            .with(|s| s.as_ref().map_or("", |s| label_of(&DETAILS, &s.detail)))
    };
    let levels = move || {
        ctx.state
            .with(|s| s.as_ref().map(active_level_labels).unwrap_or_default())
    };
    view! {
        <div class="row wrap" style="gap:.4em;margin-bottom:.6em">
            <span class="chip">{move || format!("@{}", bot().unwrap_or_default())}</span>
            <span class="chip">{detail}</span>
            {move || levels().into_iter().map(|label| view! { <span class="chip">{label}</span> }).collect_view()}
        </div>
        <div class="psec">
            <h5>"Activity"</h5>
            <TelegramLog />
        </div>
        <div class="sub" style="margin-top:.7em">"Telegram's servers can read the messages the bot sends."</div>
    }
}

/// Opens Settings on the Integrations page, where the bot is set up.
fn open_integrations(ui: Ui) {
    ui.modal.set(Some(Modal::Settings {
        page: "integrations".to_owned(),
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_levels(levels: &[&str]) -> TelegramState {
        TelegramState {
            connected: true,
            waiting_for_start: false,
            bot_username: Some("sentinel_bot".to_owned()),
            token_saved: true,
            user_id: Some(42),
            levels: levels.iter().map(|level| (*level).to_owned()).collect(),
            events: Vec::new(),
            detail: "summary".to_owned(),
            quiet_hours: None,
        }
    }

    #[test]
    fn only_the_levels_that_are_on_show_as_chips_in_settings_order() {
        let state = state_with_levels(&["reports", "notify"]);
        assert_eq!(active_level_labels(&state), vec!["Notify", "Reports"]);
    }

    #[test]
    fn no_levels_on_means_no_chips() {
        let none: Vec<&str> = Vec::new();
        assert_eq!(active_level_labels(&state_with_levels(&[])), none);
    }
}
