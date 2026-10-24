//! `NoteEditor`: the text box that replaces a note's text while you edit it. Save sends the text
//! and the version it was read at, so the daemon refuses an edit over a newer change (409).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::NoteId;
use sentinel_core::brain_api::{NoteDetail, SaveNote};

use crate::api;
use crate::brain::is_missing;
use crate::state::ToastKind;
use crate::views::brain::{BrainCtx, bump, failure_text};

/// The signals of one edit: whether the box is open, and the message of the last failed save.
#[derive(Clone, Copy)]
struct EditSignals {
    editing: RwSignal<bool>,
    error: RwSignal<Option<String>>,
}

#[component]
pub fn NoteEditor(note: NoteDetail, editing: RwSignal<bool>, ctx: BrainCtx) -> impl IntoView {
    let draft = RwSignal::new(note.body);
    let edit = EditSignals {
        editing,
        error: RwSignal::new(None),
    };
    let id = note.summary.id;
    let version = note.version;
    let save = move |_| {
        let request = SaveNote {
            body: draft.get_untracked(),
            version: version.clone(),
        };
        save_note(ctx, edit, id.clone(), request);
    };
    view! {
        <textarea class="field" rows="7" aria-label="Note text"
            prop:value=move || draft.get()
            on:input=move |event| draft.set(event_target_value(&event))></textarea>
        {move || edit.error.get().map(|message| view! {
            <div class="note" style="color:var(--danger)"><span>{message}</span></div>
        })}
        <div class="row" style="margin-top:.5em">
            <button class="btn pri" on:click=save>"Save note"</button>
            <button class="btn" on:click=move |_| editing.set(false)>"Cancel"</button>
        </div>
        <div class="sub" style="margin-top:.4em">
            "Link to another note with double square brackets, like [[conv-tests]]."
        </div>
    }
}

fn save_note(ctx: BrainCtx, edit: EditSignals, id: NoteId, request: SaveNote) {
    spawn_local(async move {
        let path = format!("/api/brain/notes/{id}");
        match api::send::<_, serde_json::Value>("PUT", &path, &request).await {
            Ok(_) => {
                ctx.ui.show_toast("Note saved.", ToastKind::Ok);
                edit.editing.set(false);
                bump(ctx);
            }
            Err(failure) if is_missing(&failure) => ctx.missing.set(true),
            Err(failure) => edit.error.set(Some(failure_text(&failure))),
        }
    });
}
