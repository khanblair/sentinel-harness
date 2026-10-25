//! `KeyCard`: one provider's row in the API keys section. The typed text lives only in the
//! password input. The daemon answers with a hint (the last four characters), never the key.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{self, ApiFailure};
use crate::components::keys_section::{KeyBody, KeyInfo, KeyTest, failure_message, key_path};
use crate::components::status_badge::shape_markup;
use crate::state::{ToastKind, Ui, use_ui};

/// What the card waits for. One action at a time keeps the buttons honest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Busy {
    Idle,
    Saving,
    Testing,
    Removing,
}

/// The line under the buttons: the last save, test, or removal result. It stays until the next
/// action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub ok: bool,
    pub text: String,
}

/// Everything an action of one card reads and writes. Every field is a signal or a handle, so
/// the struct is cloned into each closure and each async block.
#[derive(Clone)]
pub struct KeyCtx {
    pub ui: Ui,
    pub provider: String,
    pub draft: RwSignal<String>,
    pub busy: RwSignal<Busy>,
    pub notice: RwSignal<Option<Notice>>,
    pub confirming: RwSignal<bool>,
    pub on_change: Callback<()>,
}

/// The status line: no key, the last four characters, or a plain note when the daemon gives no
/// hint.
pub fn status_text(info: &KeyInfo) -> String {
    if !info.has_key {
        return "No key saved.".to_owned();
    }
    let last_four = info
        .hint
        .as_deref()
        .map(|hint| hint.trim_start_matches('.'))
        .filter(|hint| !hint.is_empty());
    match last_four {
        Some(last) => format!("Saved key ending in {last}."),
        None => "A key is saved.".to_owned(),
    }
}

/// Save is allowed for any input that is not only spaces.
pub fn can_save(input: &str) -> bool {
    !input.trim().is_empty()
}

/// The color and shape of a result line. Green with a dot when ok, red with an octagon when not.
pub fn result_look(ok: bool) -> (&'static str, &'static str) {
    if ok { ("ok", "dot") } else { ("danger", "oct") }
}

/// The result line when a test did not run or the daemon refused it.
pub fn test_failed_text(error: &str) -> String {
    let reason = error.trim();
    if reason.is_empty() {
        "Test did not run.".to_owned()
    } else {
        format!("Test did not run. {reason}")
    }
}

/// The result line for a test call. A refused call and a test that ran both become a notice.
pub fn test_notice(outcome: Result<KeyTest, ApiFailure>) -> Notice {
    match outcome {
        Ok(test) => Notice {
            ok: test.ok,
            text: test.message,
        },
        Err(failure) => Notice {
            ok: false,
            text: test_failed_text(&failure_message(&failure)),
        },
    }
}

#[component]
pub fn KeyCard(
    /// The provider's row as the section last loaded it.
    info: Memo<KeyInfo>,
    /// Runs after a save or a removal, so the section reloads its list.
    on_change: Callback<()>,
) -> impl IntoView {
    let ctx = KeyCtx {
        ui: use_ui(),
        provider: info.get_untracked().provider,
        draft: RwSignal::new(String::new()),
        busy: RwSignal::new(Busy::Idle),
        notice: RwSignal::new(None),
        confirming: RwSignal::new(false),
        on_change,
    };
    let notice = ctx.notice;
    view! {
        <div class="card" style="margin-bottom:.6em">
            <div class="row sp wrap"><b>{move || info.get().label}</b></div>
            <div class="sub" style="margin:.35em 0">{move || status_text(&info.get())}</div>
            <KeyInput ctx=ctx.clone() info=info />
            <KeyActions ctx=ctx.clone() info=info />
            {move || notice.get().map(notice_line)}
        </div>
    }
}

/// The password field and the Save button. The field is cleared after a save.
#[component]
fn KeyInput(ctx: KeyCtx, info: Memo<KeyInfo>) -> impl IntoView {
    let draft = ctx.draft;
    let busy = ctx.busy;
    let save = ctx.clone();
    view! {
        <div class="row wrap">
            <input
                class="field"
                type="password"
                placeholder="Paste your key"
                aria-label=move || format!("{} API key", info.get().label)
                autocomplete="off"
                spellcheck="false"
                style="max-width:16em"
                prop:value=draft
                on:input=move |ev| draft.set(event_target_value(&ev))
            />
            <button
                class="btn"
                disabled=move || !can_save(&draft.get()) || busy.get() != Busy::Idle
                on:click=move |_| save_key(save.clone())
            >
                "Save"
            </button>
        </div>
    }
}

/// The Test button, and the Remove control when a key is saved.
#[component]
fn KeyActions(ctx: KeyCtx, info: Memo<KeyInfo>) -> impl IntoView {
    let busy = ctx.busy;
    let test = ctx.clone();
    view! {
        <div class="row wrap">
            <button
                class="btn"
                disabled=move || !info.get().has_key || busy.get() != Busy::Idle
                on:click=move |_| test_key(test.clone())
            >
                {move || if busy.get() == Busy::Testing { "Testing" } else { "Test" }}
            </button>
            {move || info.get().has_key.then(|| view! { <RemoveControl ctx=ctx.clone() /> })}
        </div>
    }
}

/// A Remove button that asks once. The question and its two buttons replace it until answered.
#[component]
fn RemoveControl(ctx: KeyCtx) -> impl IntoView {
    let confirming = ctx.confirming;
    let busy = ctx.busy;
    let remove = ctx.clone();
    view! {
        <Show
            when=move || !confirming.get()
            fallback=move || confirm_row(remove.clone())
        >
            <button
                class="btn sm dng"
                disabled=move || busy.get() != Busy::Idle
                on:click=move |_| confirming.set(true)
            >
                "Remove"
            </button>
        </Show>
    }
}

fn confirm_row(ctx: KeyCtx) -> impl IntoView {
    let confirming = ctx.confirming;
    view! {
        <span class="sub">"Remove this key?"</span>
        <button class="btn sm dng" on:click=move |_| remove_key(ctx.clone())>"Remove"</button>
        <button class="btn sm" on:click=move |_| confirming.set(false)>"Cancel"</button>
    }
}

fn notice_line(notice: Notice) -> impl IntoView {
    let (color, shape) = result_look(notice.ok);
    let style = format!("color:var(--{color})");
    view! {
        <div class="stb" style=style role="status">
            <svg class="shp" viewBox="0 0 12 12" aria-hidden="true" inner_html=shape_markup(shape, color)></svg>
            <span>{notice.text}</span>
        </div>
    }
}

/// Sends the typed key. On success the field is cleared, the list reloads, and the new key is
/// tested once. On failure the field keeps its text, so the owner can fix it.
fn save_key(ctx: KeyCtx) {
    let value = ctx.draft.get_untracked().trim().to_owned();
    if !can_save(&value) {
        return;
    }
    ctx.notice.set(None);
    ctx.busy.set(Busy::Saving);
    spawn_local(async move {
        let body = KeyBody { key: value };
        let result =
            api::send::<_, serde_json::Value>("PUT", &key_path(&ctx.provider), &body).await;
        ctx.busy.set(Busy::Idle);
        match result {
            Ok(_) => {
                ctx.draft.set(String::new());
                ctx.ui.show_toast("Key saved.", ToastKind::Ok);
                ctx.on_change.run(());
                test_key(ctx);
            }
            Err(failure) => ctx.notice.set(Some(Notice {
                ok: false,
                text: failure_message(&failure),
            })),
        }
    });
}

/// Asks the daemon to test the saved key and shows the answer on the card.
fn test_key(ctx: KeyCtx) {
    ctx.notice.set(None);
    ctx.busy.set(Busy::Testing);
    spawn_local(async move {
        let path = format!("{}/test", key_path(&ctx.provider));
        let outcome = api::post_empty::<KeyTest>(&path).await;
        ctx.busy.set(Busy::Idle);
        ctx.notice.set(Some(test_notice(outcome)));
    });
}

fn remove_key(ctx: KeyCtx) {
    ctx.notice.set(None);
    ctx.busy.set(Busy::Removing);
    spawn_local(async move {
        let result = api::send::<_, serde_json::Value>(
            "DELETE",
            &key_path(&ctx.provider),
            &serde_json::json!({}),
        )
        .await;
        ctx.busy.set(Busy::Idle);
        ctx.confirming.set(false);
        match result {
            Ok(_) => {
                ctx.ui.show_toast("Key removed.", ToastKind::Ok);
                ctx.on_change.run(());
            }
            Err(failure) => ctx.notice.set(Some(Notice {
                ok: false,
                text: failure_message(&failure),
            })),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_row(has_key: bool, hint: Option<&str>) -> KeyInfo {
        KeyInfo {
            provider: "claude".to_owned(),
            label: "Claude".to_owned(),
            has_key,
            hint: hint.map(str::to_owned),
        }
    }

    #[test]
    fn a_card_without_a_key_says_no_key_is_saved() {
        assert_eq!(status_text(&key_row(false, None)), "No key saved.");
    }

    #[test]
    fn a_saved_key_shows_its_last_four_characters() {
        assert_eq!(
            status_text(&key_row(true, Some("...abcd"))),
            "Saved key ending in abcd."
        );
    }

    #[test]
    fn a_saved_key_without_a_hint_still_says_it_is_saved() {
        assert_eq!(status_text(&key_row(true, None)), "A key is saved.");
    }

    #[test]
    fn save_stays_off_for_an_empty_or_blank_input() {
        assert!(!can_save(""));
        assert!(!can_save("   "));
        assert!(can_save("sk-live-123"));
    }

    #[test]
    fn a_good_result_is_green_with_a_dot_and_a_bad_one_is_red_with_an_octagon() {
        assert_eq!(result_look(true), ("ok", "dot"));
        assert_eq!(result_look(false), ("danger", "oct"));
    }

    #[test]
    fn a_test_that_did_not_run_keeps_the_daemon_reason() {
        assert_eq!(
            test_failed_text("No key saved yet."),
            "Test did not run. No key saved yet."
        );
        assert_eq!(test_failed_text("  "), "Test did not run.");
    }

    #[test]
    fn a_refused_test_call_becomes_a_red_notice_with_the_reason() {
        let outcome = Err(ApiFailure::Status {
            code: 400,
            message: "No key saved yet.".to_owned(),
        });
        let notice = test_notice(outcome);
        assert!(!notice.ok);
        assert_eq!(notice.text, "Test did not run. No key saved yet.");
    }

    #[test]
    fn a_finished_test_shows_the_daemon_message_and_its_verdict() {
        let test = KeyTest {
            provider: "claude".to_owned(),
            ok: true,
            message: "Key works.".to_owned(),
        };
        assert_eq!(
            test_notice(Ok(test)),
            Notice {
                ok: true,
                text: "Key works.".to_owned()
            }
        );
    }
}
