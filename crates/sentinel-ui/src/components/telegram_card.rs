//! The Telegram card of Settings, Integrations. It saves the bot, waits for `/start`, and once the
//! chat is connected, shows what the chat may do. Changes and calls go through `TelegramCtx`.

use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::telegram_api::TelegramState;

use crate::api::ApiFailure;
use crate::components::icon::Icon;
use crate::components::telegram_levels::{DetailRow, EventToggles, LevelToggles, QuietHours};
use crate::layout::PanelId;
use crate::state::{Modal, ToastKind};
use crate::telegram::{self, TelegramCtx, chip_label, valid_user_id};

const EXPLAIN: &str = "One private chat with you. It notifies you, lets you answer approvals with one tap, adds tasks, starts runs, and passes short instructions to an agent. Sentinel calls Telegram outward, so nothing is exposed and no VPN is needed. Groups and channels are not supported.";

#[component]
pub fn TelegramCard() -> impl IntoView {
    let ctx = TelegramCtx::load(crate::state::use_ui());
    telegram::poll_while_waiting(ctx);
    view! {
        <Show when=move || ctx.state.with(Option::is_some) fallback=|| ()>
            <div class="card" style="margin-bottom:.8em">
                <CardHeader ctx />
                <p class="sub" style="margin:.4em 0">{EXPLAIN}</p>
                <Show when=move || !has_bot(ctx) fallback=move || view! { <BotFound ctx /> }>
                    <SetupForm ctx />
                </Show>
            </div>
        </Show>
        <Show when=move || ctx.state.with(|s| s.as_ref().is_some_and(|s| s.waiting_for_start))
            fallback=|| ()>
            <StartChatCard ctx />
        </Show>
        <Show when=move || ctx.state.with(|s| s.as_ref().is_some_and(|s| s.connected))
            fallback=|| ()>
            <ConnectedCard ctx />
        </Show>
    }
}

#[component]
fn CardHeader(ctx: TelegramCtx) -> impl IntoView {
    let chip = move || {
        ctx.state
            .with(|s| s.as_ref().map_or("Not connected", chip_label))
    };
    view! {
        <div class="row sp wrap">
            <b class="row"><Icon name="send" size=16 /> "Telegram"</b>
            <span class="chip">{chip}</span>
        </div>
    }
}

/// The three steps a user takes before the bot exists.
#[component]
fn SetupSteps() -> impl IntoView {
    view! {
        <ol class="ol">
            <li><span>"In Telegram, message " <b>"@BotFather"</b>", send " <span class="k">"/newbot"</span>", and copy the " <b>"bot token"</b>"."</span></li>
            <li><span>"Find your " <b>"Telegram user ID"</b>". It is a number, not your @username. A bot such as " <b>"@userinfobot"</b>" will tell you."</span></li>
            <li><span>"Enter both below. Only this one user ID can talk to the bot."</span></li>
        </ol>
    }
}

/// The token and user ID fields, and the button that saves and checks them.
#[component]
fn SetupForm(ctx: TelegramCtx) -> impl IntoView {
    let token_input = NodeRef::<html::Input>::new();
    let user_input = NodeRef::<html::Input>::new();
    view! {
        <SetupSteps />
        <div class="fld col" style="border:0;padding:.3em 0">
            <label class="sub" for="tgToken">"Bot token"</label>
            <input class="field" id="tgToken" type="password" autocomplete="off"
                placeholder="123456789:ABCdefGhIJKlmNoPQRsTUVwxyz" node_ref=token_input
                disabled=move || ctx.locked() />
        </div>
        <div class="fld col" style="border:0;padding:.3em 0">
            <label class="sub" for="tgUid">"Your Telegram user ID"</label>
            <input class="field" id="tgUid" inputmode="numeric" autocomplete="off"
                placeholder="1048293" style="max-width:14em" node_ref=user_input
                disabled=move || ctx.locked() />
        </div>
        <div class="row wrap" style="margin-top:.5em">
            <button class="btn pri" disabled=move || ctx.locked()
                on:click=move |_| submit_token(ctx, token_input, user_input)>
                "Save and check"
            </button>
        </div>
    }
}

/// The bot exists. It shows the bot name and the remove button.
#[component]
fn BotFound(ctx: TelegramCtx) -> impl IntoView {
    let bot = move || format!("@{}", bot_of(ctx));
    let user = move || user_id_of(ctx).map_or_else(String::new, |id| id.to_string());
    view! {
        <div class="row wrap" style="margin-top:.5em">
            <span class="row" style="color:var(--ok)">
                <Icon name="check" size=15 />
                <span>"Bot " <b>{bot}</b> " found. The token is in the OS keychain. Allowed user ID: " <span class="k">{user}</span> "."</span>
            </span>
            <span style="flex:1"></span>
            <button class="btn sm dng" disabled=move || ctx.locked() || user_id_of(ctx).is_none()
                on:click=move |_| revoke_current(ctx)>
                "Remove bot"
            </button>
        </div>
    }
}

/// Shown while the bot waits for `/start` from the owner.
#[component]
fn StartChatCard(ctx: TelegramCtx) -> impl IntoView {
    let bot = move || format!("@{}", bot_of(ctx));
    let user = move || user_id_of(ctx).map_or_else(String::new, |id| id.to_string());
    view! {
        <div class="card" style="margin-bottom:.8em">
            <b>"Start the chat"</b>
            <ol class="ol">
                <li><span>"Open " <b>{bot}</b> " in Telegram."</span></li>
                <li><span>"Send " <span class="k">"/start"</span> " from the account with user ID " <span class="k">{user}</span>". A bot cannot message you first."</span></li>
            </ol>
            <div class="row wrap">
                <span class="row sub"><span class="spin"></span><span>"Waiting for /start"</span></span>
            </div>
        </div>
    }
}

/// The connected chat: its row, then what it may do.
#[component]
fn ConnectedCard(ctx: TelegramCtx) -> impl IntoView {
    let user = move || user_id_of(ctx).map_or_else(String::new, |id| format!("user ID {id}"));
    view! {
        <div class="card" style="margin-bottom:.8em">
            <div class="row sp wrap">
                <span class="row">
                    <Icon name="send" size=15 />
                    <b>{move || chat_title(ctx)}</b>
                    <span class="sub">{user}</span>
                </span>
                <span class="row">
                    <button class="btn sm" on:click=move |_| open_activity(ctx)>"Open activity"</button>
                    <button class="btn sm dng" disabled=move || ctx.locked() || user_id_of(ctx).is_none()
                        on:click=move |_| revoke_current(ctx)>
                        "Revoke"
                    </button>
                </span>
            </div>
        </div>
        <div class="psec">
            <h5>"What this chat may do"</h5>
            <LevelToggles ctx />
        </div>
        <div class="psec">
            <h5>"How much messages show"</h5>
            <DetailRow ctx />
        </div>
        <div class="psec">
            <h5>"Events"</h5>
            <EventToggles ctx />
            <QuietHours ctx />
        </div>
        <div class="row wrap" style="margin:.8em 0">
            <button class="btn" disabled=move || ctx.locked() on:click=move |_| send_test(ctx)>
                "Send test message"
            </button>
        </div>
    }
}

fn has_bot(ctx: TelegramCtx) -> bool {
    ctx.state
        .with(|s| s.as_ref().is_some_and(|s| s.token_saved))
}

fn bot_of(ctx: TelegramCtx) -> String {
    ctx.state.with(|s| {
        s.as_ref()
            .and_then(|s| s.bot_username.clone())
            .unwrap_or_default()
    })
}

fn user_id_of(ctx: TelegramCtx) -> Option<i64> {
    ctx.state.with(|s| s.as_ref().and_then(|s| s.user_id))
}

fn chat_title(ctx: TelegramCtx) -> String {
    let bot = bot_of(ctx);
    if bot.is_empty() {
        "Telegram chat".to_owned()
    } else {
        format!("@{bot}")
    }
}

/// Reads the two fields once. The token leaves the input here and the field is cleared before
/// the request starts. The token is not kept in any signal or log.
fn submit_token(
    ctx: TelegramCtx,
    token_input: NodeRef<html::Input>,
    user_input: NodeRef<html::Input>,
) {
    let (Some(token_field), Some(user_field)) = (token_input.get(), user_input.get()) else {
        return;
    };
    let token = token_field.value().trim().to_owned();
    if token.is_empty() {
        ctx.ui
            .show_toast("Paste the bot token from BotFather.", ToastKind::Warn);
        return;
    }
    let user_text = user_field.value();
    let Some(user_id) = valid_user_id(user_text.trim()) else {
        ctx.ui.show_toast(
            "Enter your Telegram user ID. It is a number, not your @username.",
            ToastKind::Warn,
        );
        return;
    };
    token_field.set_value("");
    spawn_local(async move {
        let result = telegram::save_and_check(token, user_id).await;
        finish(ctx, result, "Bot found. Send /start to it in Telegram.");
    });
}

fn revoke_current(ctx: TelegramCtx) {
    let Some(user_id) = user_id_of(ctx) else {
        return;
    };
    spawn_local(async move {
        let result = telegram::revoke(user_id).await;
        finish(ctx, result, "Chat revoked.");
    });
}

fn send_test(ctx: TelegramCtx) {
    spawn_local(async move {
        match telegram::send_test().await {
            Ok(_) => ctx.ui.show_toast("Test message sent.", ToastKind::Ok),
            Err(failure) => ctx.ui.show_toast(failure.to_string(), ToastKind::Danger),
        }
    });
}

/// The Activity list lives in the panel. Its tab opens over the settings page, so the page closes.
fn open_activity(ctx: TelegramCtx) {
    ctx.ui.modal.set(None::<Modal>);
    ctx.ui.open_panel(PanelId::Telegram);
}

/// A successful call says `done`. Either way the state is applied, and a failure shows its message.
fn finish(ctx: TelegramCtx, result: Result<TelegramState, ApiFailure>, done: &str) {
    if result.is_ok() {
        ctx.ui.show_toast(done, ToastKind::Ok);
    }
    ctx.apply(result);
}
