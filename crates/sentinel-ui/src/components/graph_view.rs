//! `GraphView`: one dot per note, a line for each link, and the title under each dot. The layout is
//! a pure function, so the same notes always draw the same picture. Tapping a dot opens the note.
//! The notes list is the text alternative, so the graph has no text of its own.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::f64::consts::{FRAC_PI_2, TAU};

use leptos::prelude::*;
use sentinel_core::NoteId;
use sentinel_core::brain_api::{GraphNodeView, GraphView as GraphData};

use crate::brain::type_color_token;
use crate::views::brain::{BrainCtx, open_note};

/// Canvas size of the graph, in SVG units.
pub const GRAPH_WIDTH: f64 = 400.0;
pub const GRAPH_HEIGHT: f64 = 340.0;

/// Space kept between the outer ring and the canvas edge, so dots and labels stay on the canvas.
const EDGE_MARGIN: f64 = 28.0;

/// A note dot on the canvas.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedNode {
    pub id: NoteId,
    pub title: String,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub color: &'static str,
}

/// Places every note. The project note sits at the center, its direct links on the first ring, and
/// each further step of links on the next ring out. A note the center cannot reach goes on the
/// outer ring. Equal input gives equal output, because nothing depends on a hash seed.
pub fn layout_nodes(nodes: &[GraphNodeView], links: &[(NoteId, NoteId)]) -> Vec<PlacedNode> {
    let mut sorted: Vec<&GraphNodeView> = nodes.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let known: BTreeSet<&NoteId> = sorted.iter().map(|node| &node.id).collect();
    let linked: Vec<(NoteId, NoteId)> = links
        .iter()
        .filter(|(from, to)| known.contains(from) && known.contains(to))
        .cloned()
        .collect();
    let adjacency = adjacency(&linked);
    let Some(center) = pick_center(&sorted, &adjacency) else {
        return Vec::new();
    };
    let ring_of = rings(&sorted, &center, &adjacency);
    let outer = ring_of.values().max().copied().unwrap_or(0);
    let mut by_ring: BTreeMap<usize, Vec<&GraphNodeView>> = BTreeMap::new();
    for node in &sorted {
        let ring = ring_of.get(&node.id).copied().unwrap_or(outer);
        by_ring.entry(ring).or_default().push(node);
    }
    by_ring
        .into_iter()
        .flat_map(|(ring, group)| place_ring(ring, outer, &group))
        .collect()
}

fn adjacency(links: &[(NoteId, NoteId)]) -> BTreeMap<NoteId, Vec<NoteId>> {
    let mut map: BTreeMap<NoteId, Vec<NoteId>> = BTreeMap::new();
    for (from, to) in links {
        map.entry(from.clone()).or_default().push(to.clone());
        map.entry(to.clone()).or_default().push(from.clone());
    }
    map
}

/// The project note when there is one, otherwise the most linked note. A tie goes to the lower id.
fn pick_center(
    nodes: &[&GraphNodeView],
    adjacency: &BTreeMap<NoteId, Vec<NoteId>>,
) -> Option<NoteId> {
    if let Some(project) = nodes.iter().find(|node| is_project_id(&node.id)) {
        return Some(project.id.clone());
    }
    nodes
        .iter()
        .max_by_key(|node| (degree(adjacency, &node.id), Reverse(node.id.clone())))
        .map(|node| node.id.clone())
}

/// The seed names the project note `project`, or `project-<name>` when the folder has a name.
fn is_project_id(id: &NoteId) -> bool {
    id.as_str() == "project" || id.as_str().starts_with("project-")
}

fn degree(adjacency: &BTreeMap<NoteId, Vec<NoteId>>, id: &NoteId) -> usize {
    adjacency.get(id).map_or(0, Vec::len)
}

/// Each note's ring: its link distance from the center. Unreached notes go one ring past the
/// farthest reached one.
fn rings(
    nodes: &[&GraphNodeView],
    center: &NoteId,
    adjacency: &BTreeMap<NoteId, Vec<NoteId>>,
) -> BTreeMap<NoteId, usize> {
    let mut distance: BTreeMap<NoteId, usize> = BTreeMap::new();
    let mut queue = VecDeque::from([center.clone()]);
    distance.insert(center.clone(), 0);
    while let Some(id) = queue.pop_front() {
        let Some(step) = distance.get(&id).copied() else {
            continue;
        };
        for neighbor in adjacency.get(&id).into_iter().flatten() {
            if !distance.contains_key(neighbor) {
                distance.insert(neighbor.clone(), step + 1);
                queue.push_back(neighbor.clone());
            }
        }
    }
    let outer = distance.values().max().map_or(1, |far| far + 1);
    for node in nodes {
        distance.entry(node.id.clone()).or_insert(outer);
    }
    distance
}

/// Spreads the notes of one ring evenly around its ellipse, in id order.
fn place_ring(ring: usize, outer: usize, group: &[&GraphNodeView]) -> Vec<PlacedNode> {
    let fraction = if outer == 0 {
        0.0
    } else {
        as_f64(ring) / as_f64(outer)
    };
    let (cx, cy) = (GRAPH_WIDTH / 2.0, GRAPH_HEIGHT / 2.0);
    let (rx, ry) = (cx - EDGE_MARGIN, cy - EDGE_MARGIN);
    let count = as_f64(group.len());
    group
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let angle = TAU * as_f64(index) / count - FRAC_PI_2;
            PlacedNode {
                id: node.id.clone(),
                title: node.title.clone(),
                x: cx + angle.cos() * rx * fraction,
                y: cy + angle.sin() * ry * fraction,
                radius: dot_radius(node.backlinks),
                color: type_color_token(node.note_type),
            }
        })
        .collect()
}

/// Dots grow with the number of backlinks, up to 10 px.
fn dot_radius(backlinks: u32) -> f64 {
    4.0 + (f64::from(backlinks) * 1.2).min(6.0)
}

fn as_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).unwrap_or(u32::MAX))
}

/// The two ends of a link, or `None` when an end is not on the canvas.
pub fn edge_points(placed: &[PlacedNode], link: &(NoteId, NoteId)) -> Option<[(f64, f64); 2]> {
    let from = placed.iter().find(|dot| dot.id == link.0)?;
    let to = placed.iter().find(|dot| dot.id == link.1)?;
    Some([(from.x, from.y), (to.x, to.y)])
}

#[component]
pub fn GraphView(graph: GraphData, ctx: BrainCtx) -> impl IntoView {
    let GraphData { nodes, links } = graph;
    let placed = layout_nodes(&nodes, &links);
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

fn dot_view(dot: PlacedNode, ctx: BrainCtx) -> impl IntoView {
    let PlacedNode {
        id,
        title,
        x,
        y,
        radius,
        color,
    } = dot;
    let key_id = id.clone();
    let label_y = y + radius + 10.0;
    let tip = title.clone();
    view! {
        <g role="button" tabindex="0" style="cursor:pointer" aria-label=tip.clone()
            on:click=move |_| open_note(ctx, id.clone())
            on:keydown=move |event: web_sys::KeyboardEvent| {
                if event.key() == "Enter" {
                    open_note(ctx, key_id.clone());
                }
            }>
            <circle cx=format!("{x:.1}") cy=format!("{y:.1}") r=format!("{radius:.1}")
                fill=format!("var(--{color})") stroke="var(--bg)" stroke-width="1.5">
                <title>{title.clone()}</title>
            </circle>
            <text x=format!("{x:.1}") y=format!("{label_y:.1}") text-anchor="middle"
                font-size="9" fill="var(--muted)">
                {title}
            </text>
        </g>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::NoteType;

    fn node(id: &str, backlinks: u32) -> GraphNodeView {
        GraphNodeView {
            id: NoteId::new(id),
            title: id.to_owned(),
            note_type: NoteType::Fix,
            backlinks,
        }
    }

    fn link(from: &str, to: &str) -> (NoteId, NoteId) {
        (NoteId::new(from), NoteId::new(to))
    }

    fn find<'a>(placed: &'a [PlacedNode], id: &str) -> &'a PlacedNode {
        placed
            .iter()
            .find(|dot| dot.id.as_str() == id)
            .expect("the note was placed")
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn the_same_notes_and_links_give_the_same_layout() {
        let nodes = vec![node("project", 0), node("a", 1), node("b", 0)];
        let links = vec![link("project", "a"), link("a", "b")];
        assert_eq!(layout_nodes(&nodes, &links), layout_nodes(&nodes, &links));
    }

    #[test]
    fn the_project_note_sits_at_the_center() {
        let nodes = vec![node("area-src", 0), node("project-sentinel", 0)];
        let links = vec![link("project-sentinel", "area-src")];
        let placed = layout_nodes(&nodes, &links);
        let center = find(&placed, "project-sentinel");
        assert!(near(center.x, GRAPH_WIDTH / 2.0));
        assert!(near(center.y, GRAPH_HEIGHT / 2.0));
    }

    #[test]
    fn without_a_project_note_the_most_linked_note_is_the_center() {
        let nodes = vec![node("a", 0), node("hub", 0), node("b", 0)];
        let links = vec![link("a", "hub"), link("b", "hub")];
        let placed = layout_nodes(&nodes, &links);
        let hub = find(&placed, "hub");
        assert!(near(hub.x, GRAPH_WIDTH / 2.0));
        assert!(near(hub.y, GRAPH_HEIGHT / 2.0));
    }

    #[test]
    fn no_two_notes_share_a_position() {
        let nodes: Vec<GraphNodeView> = (0..12).map(|i| node(&format!("n{i:02}"), 0)).collect();
        let links = vec![link("n00", "n01"), link("n00", "n02"), link("n01", "n03")];
        let placed = layout_nodes(&nodes, &links);
        assert_eq!(placed.len(), 12);
        for (index, dot) in placed.iter().enumerate() {
            for other in placed.iter().skip(index + 1) {
                let same = near(dot.x, other.x) && near(dot.y, other.y);
                assert!(!same, "{} and {} share a spot", dot.id, other.id);
            }
        }
    }

    #[test]
    fn a_direct_link_of_the_center_is_on_a_nearer_ring_than_a_far_note() {
        let nodes = vec![node("project", 0), node("area", 0), node("file", 0)];
        let links = vec![link("project", "area"), link("area", "file")];
        let placed = layout_nodes(&nodes, &links);
        let (cx, cy) = (GRAPH_WIDTH / 2.0, GRAPH_HEIGHT / 2.0);
        let from_center = |id: &str| {
            let dot = find(&placed, id);
            (dot.x - cx).hypot(dot.y - cy)
        };
        assert!(from_center("area") < from_center("file"));
    }

    #[test]
    fn an_empty_graph_has_no_dots() {
        assert_eq!(layout_nodes(&[], &[]), Vec::new());
    }

    #[test]
    fn an_edge_needs_both_ends_on_the_canvas() {
        let nodes = vec![node("a", 0), node("b", 0)];
        let placed = layout_nodes(&nodes, &[]);
        assert!(edge_points(&placed, &link("a", "b")).is_some());
        assert!(edge_points(&placed, &link("a", "zz")).is_none());
    }

    #[test]
    fn a_dot_grows_with_backlinks_and_stops_at_ten() {
        assert!((dot_radius(0) - 4.0).abs() < f64::EPSILON);
        assert!(dot_radius(2) > dot_radius(1));
        assert!((dot_radius(100) - 10.0).abs() < f64::EPSILON);
    }
}
