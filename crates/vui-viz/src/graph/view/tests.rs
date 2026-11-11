use super::*;
use crate::graph::{Edge, Node};

#[test]
fn highlight_resolves_ids_to_places_and_lights_the_edges_between() {
    let d = GraphData::new()
        .node(Node::new(5, "a", 0))
        .node(Node::new(6, "b", 0))
        .node(Node::new(7, "c", 0))
        .edge(Edge::new(5, 6, "x"))
        .edge(Edge::new(6, 7, "y"));
    let g = Compiled::new(&d);
    let lit = resolve(
        &g,
        &Highlight {
            nodes: vec![7, 99],
            edges: vec![(6, 5)],
            pulse: vec![7],
        },
    );
    assert_eq!(lit.nodes, [0, 1, 2].into_iter().collect());
    assert_eq!(
        lit.links,
        [0].into_iter().collect(),
        "the edge counts either way round"
    );
    assert_eq!(lit.pulse, [2]);
    assert!(resolve(&g, &Highlight::default()).is_empty());
}

#[test]
fn a_new_state_starts_arranged_and_nothing_has_grown_yet() {
    let mut d = GraphData::new();
    for i in 0..30u64 {
        d = d.node(Node::new(i, format!("n{i}"), 0));
        if i > 0 {
            d = d.edge(Edge::new(i, i / 2, "r"));
        }
    }
    let s = GraphState::new(&d, &Highlight::default());
    assert!(s.grown.iter().all(|g| *g == 0.0) && s.follow);
    assert!(!s.layout.is_settled(), "it keeps settling on screen");
    let spread = s
        .layout
        .positions()
        .map(|p| p.x.abs().max(p.y.abs()))
        .fold(0.0, f32::max);
    assert!(spread > 60.0, "already spread out: {spread}");
}
