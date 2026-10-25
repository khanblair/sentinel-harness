//! The Graph tab: the note graph the panel header loaded, a legend of the types, and one sentence on
//! how to read it. With no notes it shows the empty text in place of the canvas.

use leptos::prelude::*;

use crate::brain::{
    LOADING_GRAPH, LoadState, NO_LINKS_TEXT, NOTE_TYPES, graph_is_empty, type_color_token,
    type_label,
};
use crate::components::graph_view::GraphView;
use crate::views::brain::{BrainCtx, load_notice};

const READ_HELP: &str = "Each dot is a note. Lines are links. Bigger dots have more links pointing at them. Tap a dot to open the note.";

#[component]
pub fn GraphTab(ctx: BrainCtx) -> impl IntoView {
    view! {
        {move || match ctx.graph.get() {
            LoadState::Ready(data) if graph_is_empty(&data.nodes) => no_links_card().into_any(),
            LoadState::Ready(data) => view! { <GraphView graph=data ctx=ctx /> }.into_any(),
            state @ (LoadState::Loading | LoadState::Failed(_)) => {
                load_notice(&state, LOADING_GRAPH)
            }
        }}
        <div class="row wrap" style="gap:.5em;margin:.6em 0">
            {NOTE_TYPES
                .into_iter()
                .map(|note_type| view! {
                    <span class="sub row" style="gap:.3em">
                        <span class="dot" style=format!("background:var(--{})", type_color_token(note_type))></span>
                        {type_label(note_type)}
                    </span>
                })
                .collect_view()}
        </div>
        <div class="sub">{READ_HELP}</div>
    }
}

fn no_links_card() -> impl IntoView {
    view! {
        <div class="note"><span>{NO_LINKS_TEXT}</span></div>
    }
}
