//! `NoteView`: one note. Its text with links, the lesson card for a lesson, Edit and pin on the
//! computer, the strength, and the notes that link to it (`docs/ui-rules.md`, Brain).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::NoteId;
use sentinel_core::brain_api::{NoteDetail, NoteLink, NoteSummary, PinNote};

use crate::api;
use crate::brain::{
    Segment, link_segments, note_meta_line, pin_done_text, type_color_token, type_label,
};
use crate::components::icon::Icon;
use crate::components::lesson_card::LessonCard;
use crate::components::note_editor::NoteEditor;
use crate::components::strength_bar::StrengthBar;
use crate::state::{ToastKind, use_ui};
use crate::views::brain::{BrainCtx, bump, load_note, open_note, report_failure};

#[component]
pub fn NoteView(id: NoteId, ctx: BrainCtx) -> impl IntoView {
    let detail = RwSignal::new(None::<NoteDetail>);
    Effect::new(move |_| {
        ctx.revision.track();
        load_note(ctx, id.clone(), detail);
    });
    view! {
        <button class="btn sm" on:click=move |_| ctx.open_note.set(None)>
            <Icon name="back" size=13 />" All notes"
        </button>
        {move || detail.get().map(|note| view! { <NoteDetailView note=note ctx=ctx /> })}
    }
}

#[component]
fn NoteDetailView(note: NoteDetail, ctx: BrainCtx) -> impl IntoView {
    let editing = RwSignal::new(false);
    let type_style = format!("color:var(--{})", type_color_token(note.summary.note_type));
    let type_name = type_label(note.summary.note_type);
    let title = note.summary.title.clone();
    let meta = note_meta_line(&note.summary.by, note.confirmed.as_deref(), note.uses);
    let strength = note.summary.strength;
    let backlinks = note.backlinks.clone();
    let editor_copy = note.clone();
    let body_copy = note;
    view! {
        <div class="row sp wrap" style="margin:.7em 0 .3em">
            <span class="ntype" style=type_style>{type_name}</span>
            <span class="sub">{meta}</span>
        </div>
        <h3 style="margin:.1em 0 .5em;font-size:1.1em">{title}</h3>
        {move || if editing.get() {
            view! { <NoteEditor note=editor_copy.clone() editing=editing ctx=ctx /> }.into_any()
        } else {
            view! { <NoteBody note=body_copy.clone() editing=editing ctx=ctx /> }.into_any()
        }}
        <div class="psec" style="margin-top:1em">
            <h5>"Strength"</h5>
            <StrengthBar strength=strength wide=true />
            <div class="sub" style="margin-top:.3em">
                "Rises when agents use a note and you confirm it. Falls when it goes unused."
            </div>
        </div>
        <div class="psec">
            <h5>{format!("Linked from · {}", backlinks.len())}</h5>
            {backlink_rows(ctx, backlinks)}
        </div>
    }
}

/// The text, the lesson card, and the actions that change the note.
#[component]
fn NoteBody(note: NoteDetail, editing: RwSignal<bool>, ctx: BrainCtx) -> impl IntoView {
    let segments = link_segments(&note.body);
    let NoteDetail {
        summary,
        lesson,
        links,
        ..
    } = note;
    let NoteSummary {
        id,
        pinned,
        lesson_state,
        ..
    } = summary;
    let lesson_card = match (lesson, lesson_state) {
        (Some(lesson), Some(state)) => {
            view! { <LessonCard ctx=ctx id=id.clone() lesson=lesson state=state /> }.into_any()
        }
        _ => ().into_any(),
    };
    let ui = use_ui();
    view! {
        <div class="msg-sys" style="white-space:pre-wrap">
            {segments
                .into_iter()
                .map(|segment| segment_view(segment, &links, ctx))
                .collect_view()}
        </div>
        {lesson_card}
        {move || (!ui.on_phone())
            .then(|| edit_actions(ctx, editing, id.clone(), pinned))}
    }
}

fn edit_actions(ctx: BrainCtx, editing: RwSignal<bool>, id: NoteId, pinned: bool) -> impl IntoView {
    view! {
        <div class="row wrap" style="margin-top:.7em">
            <button class="btn sm" on:click=move |_| editing.set(true)>"Edit"</button>
            <button class="btn sm" on:click=move |_| toggle_pin(ctx, &id, !pinned)>
                {if pinned { "Stop always loading" } else { "Always load for agents" }}
            </button>
        </div>
    }
}

fn segment_view(segment: Segment, links: &[NoteLink], ctx: BrainCtx) -> AnyView {
    match segment {
        Segment::Text(text) => text.into_any(),
        Segment::Link { id, label } => link_view(id, label, links, ctx),
    }
}

/// A link to a note that exists is a button. A link to a missing note is struck through.
fn link_view(id: String, label: Option<String>, links: &[NoteLink], ctx: BrainCtx) -> AnyView {
    let target = links
        .iter()
        .find(|link| link.id.as_str() == id)
        .filter(|link| !link.dead)
        .cloned();
    if let Some(link) = target {
        let text = label.unwrap_or(link.title);
        let target_id = link.id;
        view! {
            <button class="wl" on:click=move |_| open_note(ctx, target_id.clone())>{text}</button>
        }
        .into_any()
    } else {
        let text = label.unwrap_or(id);
        view! {
            <span class="wl dead" title="No note with this name">{text}</span>
        }
        .into_any()
    }
}

fn backlink_rows(ctx: BrainCtx, links: Vec<NoteLink>) -> AnyView {
    if links.is_empty() {
        return view! { <div class="sub">"Nothing links here yet."</div> }.into_any();
    }
    links
        .into_iter()
        .map(|link| {
            let id = link.id.clone();
            view! {
                <button class="nrow" on:click=move |_| open_note(ctx, id.clone())>
                    <span class="nt">{link.title}</span>
                </button>
            }
        })
        .collect_view()
        .into_any()
}

fn toggle_pin(ctx: BrainCtx, id: &NoteId, pinned: bool) {
    let path = format!("/api/brain/notes/{id}/pin");
    spawn_local(async move {
        match api::post::<_, serde_json::Value>(&path, &PinNote { pinned }).await {
            Ok(_) => {
                ctx.ui.show_toast(pin_done_text(pinned), ToastKind::Ok);
                bump(ctx);
            }
            Err(failure) => report_failure(ctx.ui, ctx.missing, &failure),
        }
    });
}
