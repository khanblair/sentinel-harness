//! The Notes tab: a search box and a type filter over the list, a New note form on the computer,
//! and the note view when one is open. The search waits 250 ms after the last key.

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::NoteType;
use sentinel_core::brain_api::{NewNote, NoteList, NoteSummary};

use crate::api;
use crate::brain::{
    NOTE_TYPES, new_note_error, notes_path, type_from_value, type_label, type_query_value,
};
use crate::components::note_row::NoteRow;
use crate::components::note_view::NoteView;
use crate::layout::Layout;
use crate::state::ToastKind;
use crate::views::brain::{BrainCtx, bump, report_failure};

/// How long the search waits after the last key, in milliseconds.
const SEARCH_DELAY_MS: u32 = 250;

#[component]
pub fn NotesTab(ctx: BrainCtx) -> impl IntoView {
    move || match ctx.open_note.get() {
        Some(id) => view! { <NoteView id=id ctx=ctx /> }.into_any(),
        None => view! { <NoteListView ctx=ctx /> }.into_any(),
    }
}

/// The signals of one search: the rows, and a ticket so only the newest search is sent.
#[derive(Clone, Copy)]
struct NoteSearch {
    ctx: BrainCtx,
    rows: RwSignal<Option<Vec<NoteSummary>>>,
    ticket: RwSignal<u32>,
}

impl NoteSearch {
    fn run(self, query: String, note_type: Option<NoteType>) {
        let mine = self.ticket.get_untracked() + 1;
        self.ticket.set(mine);
        spawn_local(async move {
            TimeoutFuture::new(SEARCH_DELAY_MS).await;
            if self.ticket.get_untracked() != mine {
                return;
            }
            match api::get::<NoteList>(&notes_path(&query, note_type)).await {
                Ok(found) => self.rows.set(Some(found.notes)),
                Err(failure) => report_failure(self.ctx.ui, self.ctx.missing, &failure),
            }
        });
    }
}

#[component]
fn NoteListView(ctx: BrainCtx) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let filter = RwSignal::new(String::new());
    let rows = RwSignal::new(None::<Vec<NoteSummary>>);
    let ticket = RwSignal::new(0_u32);
    let search = NoteSearch { ctx, rows, ticket };
    let new_open = RwSignal::new(false);
    Effect::new(move |_| {
        let text = query.get();
        let value = filter.get();
        ctx.revision.track();
        search.run(text, type_from_value(&value));
    });
    view! {
        <div class="row" style="gap:.4em;margin-bottom:.6em">
            <input class="field" placeholder="Search notes" aria-label="Search notes"
                prop:value=move || query.get()
                on:input=move |event| query.set(event_target_value(&event)) />
            <select class="fs" aria-label="Note type"
                prop:value=move || filter.get()
                on:change=move |event| filter.set(event_target_value(&event))>
                <option value="">"All types"</option>
                {NOTE_TYPES
                    .into_iter()
                    .map(|note_type| view! {
                        <option value=type_query_value(note_type)>{type_label(note_type)}</option>
                    })
                    .collect_view()}
            </select>
        </div>
        <div class="row sp" style="margin-bottom:.6em">
            <span class="sub">"Notes are plain markdown files. Edits in Obsidian show up here."</span>
            {move || (ctx.ui.layout.get() != Layout::Phone).then(|| view! {
                <button class="btn sm" on:click=move |_| new_open.update(|open| *open = !*open)>
                    "New note"
                </button>
            })}
        </div>
        {move || new_open.get().then(|| view! { <NewNoteForm ctx=ctx open=new_open /> })}
        <div id="brainList">{move || note_rows(ctx, rows.get())}</div>
    }
}

fn note_rows(ctx: BrainCtx, rows: Option<Vec<NoteSummary>>) -> AnyView {
    match rows {
        None => ().into_any(),
        Some(notes) if notes.is_empty() => view! {
            <div class="note"><span>"No notes match."</span></div>
        }
        .into_any(),
        Some(notes) => notes
            .into_iter()
            .map(|note| view! { <NoteRow note=note ctx=ctx /> })
            .collect_view()
            .into_any(),
    }
}

#[component]
fn NewNoteForm(ctx: BrainCtx, open: RwSignal<bool>) -> impl IntoView {
    let note_type = RwSignal::new(type_query_value(NoteType::Convention).to_owned());
    let title = RwSignal::new(String::new());
    let body = RwSignal::new(String::new());
    let error = RwSignal::new(None::<&'static str>);
    let save = move |_| {
        if let Some(message) = new_note_error(&title.get_untracked()) {
            error.set(Some(message));
            return;
        }
        error.set(None);
        let request = NewNote {
            note_type: type_from_value(&note_type.get_untracked()).unwrap_or(NoteType::Convention),
            title: title.get_untracked().trim().to_owned(),
            body: body.get_untracked(),
        };
        submit_note(ctx, open, request);
    };
    view! {
        <div class="card" style="margin-bottom:.6em">
            <div class="row" style="gap:.4em;margin-bottom:.4em">
                <select class="fs" aria-label="Note type"
                    prop:value=move || note_type.get()
                    on:change=move |event| note_type.set(event_target_value(&event))>
                    {NOTE_TYPES
                        .into_iter()
                        .map(|kind| view! {
                            <option value=type_query_value(kind)>{type_label(kind)}</option>
                        })
                        .collect_view()}
                </select>
                <input class="field" placeholder="Title" aria-label="Note title"
                    prop:value=move || title.get()
                    on:input=move |event| title.set(event_target_value(&event)) />
            </div>
            <textarea class="field" rows="5" aria-label="Note text"
                prop:value=move || body.get()
                on:input=move |event| body.set(event_target_value(&event))></textarea>
            {move || error.get().map(|message| view! { <div class="sub">{message}</div> })}
            <div class="row" style="margin-top:.5em">
                <button class="btn pri" on:click=save>"Save note"</button>
                <button class="btn" on:click=move |_| open.set(false)>"Cancel"</button>
            </div>
        </div>
    }
}

fn submit_note(ctx: BrainCtx, open: RwSignal<bool>, request: NewNote) {
    spawn_local(async move {
        match api::post::<_, serde_json::Value>("/api/brain/notes", &request).await {
            Ok(_) => {
                ctx.ui.show_toast("Note saved.", ToastKind::Ok);
                open.set(false);
                bump(ctx);
            }
            Err(failure) => report_failure(ctx.ui, ctx.missing, &failure),
        }
    });
}
