//! `NoteView`: one note. Its text with links, the lesson card for a lesson, Edit and pin on the
//! computer, the strength, and the notes that link to it (`docs/ui-rules.md`, Brain).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::NoteId;
use sentinel_core::brain_api::{NoteDetail, NoteLink, NoteSummary, PinNote};

use crate::api;
use crate::brain::{
    LOADING_NOTE, LoadState, note_meta_line, pin_done_text, type_color_token, type_label,
};
use crate::components::icon::Icon;
use crate::components::lesson_card::LessonCard;
use crate::components::note_editor::NoteEditor;
use crate::components::strength_bar::StrengthBar;
use crate::state::{ToastKind, use_ui};
use crate::views::brain::{BrainCtx, bump, load_note, load_notice, open_note, report_failure};

#[component]
pub fn NoteView(id: NoteId, ctx: BrainCtx) -> impl IntoView {
    let detail = RwSignal::new(LoadState::Loading);
    Effect::new(move |_| {
        ctx.revision.track();
        load_note(ctx, id.clone(), detail);
    });
    view! {
        <button class="btn sm" on:click=move |_| ctx.open_note.set(None)>
            <Icon name="back" size=13 />" All notes"
        </button>
        {move || match detail.get() {
            LoadState::Ready(note) => view! { <NoteDetailView note=note ctx=ctx /> }.into_any(),
            state @ (LoadState::Loading | LoadState::Failed(_)) => {
                load_notice(&state, LOADING_NOTE)
            }
        }}
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

/// One piece of a note body: plain text, or a `[[id]]` or `[[id|label]]` link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Text(String),
    Link { id: String, label: Option<String> },
}

/// Splits a note body into text and links. An unclosed `[[` stays as text.
pub fn link_segments(body: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            break;
        };
        if start > 0 {
            segments.push(Segment::Text(rest[..start].to_owned()));
        }
        segments.push(link_segment(&after[..end]));
        rest = &after[end + 2..];
    }
    if !rest.is_empty() {
        segments.push(Segment::Text(rest.to_owned()));
    }
    segments
}

fn link_segment(inner: &str) -> Segment {
    match inner.split_once('|') {
        Some((id, label)) => Segment::Link {
            id: id.trim().to_owned(),
            label: non_empty(label.trim()),
        },
        None => Segment::Link {
            id: inner.trim().to_owned(),
            label: None,
        },
    }
}

fn non_empty(text: &str) -> Option<String> {
    (!text.is_empty()).then(|| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(id: &str, label: Option<&str>) -> Segment {
        Segment::Link {
            id: id.to_owned(),
            label: label.map(str::to_owned),
        }
    }

    #[test]
    fn plain_text_is_one_text_segment() {
        assert_eq!(
            link_segments("Run the tests."),
            vec![Segment::Text("Run the tests.".to_owned())]
        );
    }

    #[test]
    fn a_link_splits_the_text_around_it() {
        assert_eq!(
            link_segments("See [[conv-tests]] first."),
            vec![
                Segment::Text("See ".to_owned()),
                link("conv-tests", None),
                Segment::Text(" first.".to_owned()),
            ]
        );
    }

    #[test]
    fn a_pipe_label_names_the_link() {
        assert_eq!(
            link_segments("[[conv-tests|the test rule]]"),
            vec![link("conv-tests", Some("the test rule"))]
        );
    }

    #[test]
    fn an_unclosed_link_stays_as_text() {
        assert_eq!(
            link_segments("open [[never closed"),
            vec![Segment::Text("open [[never closed".to_owned())]
        );
    }

    #[test]
    fn an_empty_body_has_no_segments() {
        assert_eq!(link_segments(""), Vec::new());
    }
}
