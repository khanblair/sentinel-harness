//! `LessonCard`: the typed fields of a lesson with Approve, Reject, and Retire. A phone may review
//! lessons, so the buttons show on every layout (`docs/mobile.md`).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::brain_api::{LessonReview, LessonView, ReviewLesson};
use sentinel_core::{LessonState, NoteId};

use crate::api;
use crate::brain::{
    lesson_state_color, lesson_state_label, review_done_text, review_label, review_offered,
};
use crate::state::ToastKind;
use crate::views::brain::{BrainCtx, bump, report_failure};

/// The inner SVG of a lesson state's shape, from the prototype's `shape()` function. The four
/// lesson shapes are not in `status_badge`, which serves task states.
pub fn lesson_shape_markup(state: LessonState) -> String {
    let col = format!("var(--{})", lesson_state_color(state));
    match state {
        LessonState::Candidate => format!(
            r#"<circle cx="6" cy="6" r="4.2" fill="none" stroke="{col}" stroke-width="1.6" stroke-dasharray="1.2 1.6"/>"#
        ),
        LessonState::Proven => format!(
            r#"<circle cx="6" cy="6" r="4.6" fill="none" stroke="{col}" stroke-width="1.4"/><path d="M3.8 6.2l1.6 1.6 2.9-3.4" fill="none" stroke="{col}" stroke-width="1.4"/>"#
        ),
        LessonState::Rejected => format!(
            r#"<circle cx="6" cy="6" r="4.6" fill="none" stroke="{col}" stroke-width="1.4"/><path d="M3.6 6h4.8" stroke="{col}" stroke-width="1.5"/>"#
        ),
        LessonState::Retired => format!(
            r#"<circle cx="6" cy="6" r="4.4" fill="none" stroke="{col}" stroke-width="1.4" stroke-dasharray="2.4 1.6"/>"#
        ),
    }
}

/// The state badge: a shape, a color, and a label.
#[component]
pub fn LessonBadge(state: LessonState) -> impl IntoView {
    let color = lesson_state_color(state);
    view! {
        <span class="stb" style=format!("color:var(--{color})")>
            <svg class="shp" viewBox="0 0 12 12" aria-hidden="true"
                inner_html=lesson_shape_markup(state)></svg>
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_lesson_state_has_its_own_shape() {
        for state in [
            LessonState::Candidate,
            LessonState::Proven,
            LessonState::Rejected,
            LessonState::Retired,
        ] {
            assert!(lesson_shape_markup(state).starts_with("<circle"));
        }
        assert_ne!(
            lesson_shape_markup(LessonState::Proven),
            lesson_shape_markup(LessonState::Rejected)
        );
    }
}
