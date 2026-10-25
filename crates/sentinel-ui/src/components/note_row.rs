//! `NoteRow`: one line of the notes list. The type, the title, a pin chip, the link count, and either
//! the strength bar or, for a lesson, its state (`docs/ui-rules.md`, Brain).

use leptos::prelude::*;
use sentinel_core::brain_api::NoteSummary;

use crate::brain::{links_text, type_color_token, type_label};
use crate::components::lesson_card::LessonBadge;
use crate::components::strength_bar::StrengthBar;
use crate::views::brain::{BrainCtx, open_note};

#[component]
pub fn NoteRow(note: NoteSummary, ctx: BrainCtx, link_count: usize) -> impl IntoView {
    let NoteSummary {
        id,
        note_type,
        title,
        pinned,
        lesson_state,
        strength,
        ..
    } = note;
    let type_style = format!("color:var(--{})", type_color_token(note_type));
    view! {
        <button class="nrow" on:click=move |_| open_note(ctx, id.clone())>
            <span class="ntype" style=type_style>{type_label(note_type)}</span>
            <span class="nt">{title}</span>
            {pinned.then(|| view! { <span class="chip">"Always loaded"</span> })}
            <span class="sub">{links_text(link_count)}</span>
            {match lesson_state {
                Some(state) => view! { <LessonBadge state=state /> }.into_any(),
                None => view! { <StrengthBar strength=strength /> }.into_any(),
            }}
        </button>
    }
}
