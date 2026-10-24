//! `LessonCard`: the typed fields of a lesson with Approve, Reject, and Retire. A phone may review
//! lessons, so the buttons show on every layout (`docs/mobile.md`).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::brain_api::{LessonReview, LessonView, ReviewLesson};
use sentinel_core::{LessonState, NoteId};

use crate::api;
use crate::brain::{
    lesson_state_color, lesson_state_label, lesson_state_shape, review_done_text, review_label,
    review_offered,
};
use crate::components::status_badge::shape_markup;
use crate::state::ToastKind;
use crate::views::brain::{BrainCtx, bump, report_failure};

/// The state badge: a shape, a color, and a label.
#[component]
pub fn LessonBadge(state: LessonState) -> impl IntoView {
    let color = lesson_state_color(state);
    view! {
        <span class="stb" style=format!("color:var(--{color})")>
            <svg class="shp" viewBox="0 0 12 12" aria-hidden="true"
                inner_html=shape_markup(lesson_state_shape(state), color)></svg>
            <span>{lesson_state_label(state)}</span>
        </span>
    }
}

const REVIEWS: [LessonReview; 3] = [
    LessonReview::Approve,
    LessonReview::Reject,
    LessonReview::Retire,
];

#[component]
pub fn LessonCard(
    ctx: BrainCtx,
    id: NoteId,
    lesson: LessonView,
    state: LessonState,
) -> impl IntoView {
    view! {
        <div class="card" style="margin:.6em 0">
            <div class="row sp">
                <span class="chip mono">{lesson.kind}</span>
                <LessonBadge state=state />
            </div>
            <div class="mono" style="margin:.4em 0;font-size:.88em">
                {format!("{} → {}", lesson.key, lesson.value)}
            </div>
            <div class="meta" style="display:flex;gap:.7em;color:var(--muted)">
                <span>{format!("evidence {}", lesson.evidence)}</span>
                <span>{format!("helped {}", lesson.helped)}</span>
                <span>{format!("hurt {}", lesson.hurt)}</span>
            </div>
            <div class="row" style="margin-top:.5em;gap:.4em">
                {REVIEWS
                    .into_iter()
                    .filter(|action| review_offered(state, *action))
                    .map(move |action| review_button(ctx, id.clone(), action))
                    .collect_view()}
            </div>
        </div>
    }
}

fn review_button(ctx: BrainCtx, id: NoteId, action: LessonReview) -> impl IntoView {
    view! {
        <button class="btn sm" on:click=move |_| review(ctx, &id, action)>
            {review_label(action)}
        </button>
    }
}

fn review(ctx: BrainCtx, id: &NoteId, action: LessonReview) {
    let path = format!("/api/brain/lessons/{id}/review");
    spawn_local(async move {
        match api::post::<_, serde_json::Value>(&path, &ReviewLesson { action }).await {
            Ok(_) => {
                ctx.ui.show_toast(review_done_text(action), ToastKind::Ok);
                bump(ctx);
            }
            Err(failure) => report_failure(ctx.ui, ctx.missing, &failure),
        }
    });
}
