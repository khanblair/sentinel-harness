//! The Notes tab: a search box and a type filter over the list, a New note form on the computer,
//! and the note view when one is open. The search waits 250 ms after the last key. A vault with no
//! notes offers to build them from the project.

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::NoteType;
use sentinel_core::brain_api::{NewNote, NoteList, NoteSummary};

use crate::api;
use crate::brain::{
    ASK_COMPUTER_BUILD_TEXT, BUILD_NOTES_LABEL, EmptyChoice, LEARN_SWITCH_TEXT, LOADING_NOTES,
    LoadState, NO_NOTES_MATCH, NO_NOTES_TEXT, NOTE_TYPES, READING_PROJECT_LABEL,
    empty_notes_choice, is_unfiltered, link_count, mode_needs_switch, new_note_error, notes_path,
    pinned_first, type_from_value, type_label, type_query_value,
};
use crate::components::note_row::NoteRow;
use crate::components::note_view::NoteView;
use crate::state::ToastKind;
use crate::views::brain::{BrainCtx, build_notes, bump, fail_load, load_notice, report_failure};

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
    rows: RwSignal<LoadState<Vec<NoteSummary>>>,
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
                Ok(found) => self.rows.set(LoadState::Ready(found.notes)),
                Err(failure) => fail_load(self.ctx, self.rows, &failure),
            }
        });
    }
}

#[component]
fn NoteListView(ctx: BrainCtx) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let filter = RwSignal::new(String::new());
    let rows = RwSignal::new(LoadState::Loading);
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
        <SearchRow query=query filter=filter />
        <div class="row sp" style="margin-bottom:.6em">
            <span class="sub">"Notes are plain markdown files. Edits in Obsidian show up here."</span>
            {move || (!ctx.ui.on_phone()).then(|| view! {
                <button class="btn sm" on:click=move |_| new_open.update(|open| *open = !*open)>
                    "New note"
                </button>
            })}
        </div>
        {move || new_open.get().then(|| view! { <NewNoteForm ctx=ctx open=new_open /> })}
        <div id="brainList">{move || {
            let unfiltered = is_unfiltered(&query.get(), &filter.get());
            note_list(ctx, rows.get(), unfiltered)
        }}</div>
    }
}

/// The search box and the type filter above the note list.
#[component]
fn SearchRow(query: RwSignal<String>, filter: RwSignal<String>) -> impl IntoView {
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
    }
}

/// The list, or the line it shows while it loads, or the text of its failure. An empty vault and
/// an empty search are different: the first offers to build the notes.
fn note_list(ctx: BrainCtx, rows: LoadState<Vec<NoteSummary>>, unfiltered: bool) -> AnyView {
    match rows {
        LoadState::Ready(notes) if notes.is_empty() && unfiltered => {
            match empty_notes_choice(ctx.ui.on_phone()) {
                EmptyChoice::BuildNotes => build_card(ctx).into_any(),
                EmptyChoice::AskComputer => ask_card().into_any(),
            }
        }
        LoadState::Ready(notes) if notes.is_empty() => {
            view! { <div class="note"><span>{NO_NOTES_MATCH}</span></div> }.into_any()
        }
        LoadState::Ready(mut notes) => {
            pinned_first(&mut notes);
            note_rows(ctx, notes).into_any()
        }
        state @ (LoadState::Loading | LoadState::Failed(_)) => load_notice(&state, LOADING_NOTES),
    }
}

fn note_rows(ctx: BrainCtx, notes: Vec<NoteSummary>) -> impl IntoView {
    notes
        .into_iter()
        .map(|note| {
            let count = link_count_of(ctx, &note.id);
            view! { <NoteRow note=note ctx=ctx link_count=count /> }
        })
        .collect_view()
}

/// How many links touch a note, from the graph the header loaded. Zero until the graph arrives.
fn link_count_of(ctx: BrainCtx, id: &sentinel_core::NoteId) -> usize {
    ctx.graph.with(|state| match state {
        LoadState::Ready(graph) => link_count(&graph.links, id),
        LoadState::Loading | LoadState::Failed(_) => 0,
    })
}

/// The empty vault card on the computer: the build button, and the switch note while the Brain is
/// not in learn mode.
fn build_card(ctx: BrainCtx) -> impl IntoView {
    view! {
        <div class="card">
            <b>{NO_NOTES_TEXT}</b>
            <div class="row wrap" style="margin-top:.6em">
                <button class="btn pri" disabled=move || ctx.seeding.get()
                    on:click=move |_| build_notes(ctx)>
                    {move || if ctx.seeding.get() { READING_PROJECT_LABEL } else { BUILD_NOTES_LABEL }}
                </button>
            </div>
            {move || {
                let mode = ctx.ui.snapshot.with(|snapshot| snapshot.brain_mode);
                mode_needs_switch(mode).then(|| view! {
                    <div class="sub" style="margin-top:.4em">{LEARN_SWITCH_TEXT}</div>
                })
            }}
        </div>
    }
}

/// The empty vault card on a phone: the notes are built on the computer.
fn ask_card() -> impl IntoView {
    view! {
        <div class="card">
            <b>{NO_NOTES_TEXT}</b>
            <div class="sub" style="margin-top:.3em">{ASK_COMPUTER_BUILD_TEXT}</div>
        </div>
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
            <NewNoteHead note_type=note_type title=title />
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

/// The type select and the title box at the top of the New note form.
#[component]
fn NewNoteHead(note_type: RwSignal<String>, title: RwSignal<String>) -> impl IntoView {
    view! {
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
