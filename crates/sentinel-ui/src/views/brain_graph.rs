//! The Graph tab: the note graph from `GET /api/brain/graph`, a legend of the types, and one
//! sentence on how to read it.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::brain_api::GraphView as GraphData;

use crate::api;
use crate::brain::{NOTE_TYPES, type_color_token, type_label};
use crate::components::graph_view::GraphView;
use crate::views::brain::{BrainCtx, report_failure};

const READ_HELP: &str = "Each dot is a note. Lines are links. Bigger dots have more links pointing at them. Tap a dot to open the note.";

#[component]
pub fn GraphTab(ctx: BrainCtx) -> impl IntoView {
    let graph = RwSignal::new(None::<GraphData>);
    Effect::new(move |_| {
        ctx.revision.track();
        load_graph(ctx, graph);
    });
    view! {
        {move || graph.get().map(|data| view! { <GraphView graph=data ctx=ctx /> })}
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

fn load_graph(ctx: BrainCtx, graph: RwSignal<Option<GraphData>>) {
    spawn_local(async move {
        match api::get::<GraphData>("/api/brain/graph").await {
            Ok(data) => graph.set(Some(data)),
            Err(failure) => report_failure(ctx.ui, ctx.missing, &failure),
        }
    });
}
