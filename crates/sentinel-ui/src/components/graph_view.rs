//! `GraphView`: one dot per note, grouped by type, with a line for each link. Tapping a dot opens
//! the note. The notes list is the text alternative, so the graph has no text of its own.

use leptos::prelude::*;
use sentinel_core::brain_api::GraphView as GraphData;

use crate::brain::{GRAPH_HEIGHT, GRAPH_WIDTH, Placed, edge_points, layout};
use crate::views::brain::{BrainCtx, open_note};

#[component]
pub fn GraphView(graph: GraphData, ctx: BrainCtx) -> impl IntoView {
    let GraphData { nodes, links } = graph;
    let placed = layout(&nodes, GRAPH_WIDTH, GRAPH_HEIGHT);
    let lines: Vec<[(f64, f64); 2]> = links
        .iter()
        .filter_map(|link| edge_points(&placed, link))
        .collect();
    view! {
        <div class="card" style="padding:.4em">
            <svg viewBox=format!("0 0 {GRAPH_WIDTH} {GRAPH_HEIGHT}")
                style="width:100%;height:auto;display:block" role="img"
                aria-label="Graph of linked notes">
                {lines.into_iter().map(edge_line).collect_view()}
                {placed
                    .into_iter()
                    .map(|dot| dot_view(dot, ctx))
                    .collect_view()}
            </svg>
        </div>
    }
}

fn edge_line(ends: [(f64, f64); 2]) -> impl IntoView {
    let [(x1, y1), (x2, y2)] = ends;
    view! {
        <line x1=format!("{x1:.1}") y1=format!("{y1:.1}")
            x2=format!("{x2:.1}") y2=format!("{y2:.1}")
            stroke="var(--border)" stroke-width="1" />
    }
}

fn dot_view(dot: Placed, ctx: BrainCtx) -> impl IntoView {
    let Placed {
        id,
        title,
        x,
        y,
        radius,
        color,
    } = dot;
    let key_id = id.clone();
    let label = title.clone();
    view! {
        <g role="button" tabindex="0" style="cursor:pointer" aria-label=label
            on:click=move |_| open_note(ctx, id.clone())
            on:keydown=move |event: web_sys::KeyboardEvent| {
                if event.key() == "Enter" {
                    open_note(ctx, key_id.clone());
                }
            }>
            <circle cx=format!("{x:.1}") cy=format!("{y:.1}") r=format!("{radius:.1}")
                fill=format!("var(--{color})") stroke="var(--bg)" stroke-width="1.5">
                <title>{title}</title>
            </circle>
        </g>
    }
}
