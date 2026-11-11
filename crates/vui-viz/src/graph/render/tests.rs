use super::*;
use crate::graph::{Edge, Node};
use vui_core::{Command, Fonts, Theme};

fn graph() -> (GraphData, Compiled) {
    let mut d = GraphData::new().kinds(&["Company", "Person"]);
    for i in 0..12u64 {
        d = d.node(
            Node::new(i, format!("Node {i}"), (i % 2) as u8).weight(1.0 + (i % 4) as f32),
        );
    }
    for i in 0..12u64 {
        d = d.edge(Edge::new(i, (i + 1) % 12, "rel"));
    }
    let c = Compiled::new(&d);
    (d, c)
}

fn ring_pos(n: usize, r: f32) -> Vec<Point> {
    (0..n)
        .map(|i| {
            pt(
                r * (i as f32 / n as f32 * std::f32::consts::TAU).cos(),
                r * (i as f32 / n as f32 * std::f32::consts::TAU).sin(),
            )
        })
        .collect()
}

struct Rig {
    data: GraphData,
    graph: Compiled,
    pos: Vec<Point>,
    grown: Vec<f32>,
    lit: Lit,
    style: VizStyle,
    fonts: Fonts,
}

impl Rig {
    fn new() -> Rig {
        let (data, graph) = graph();
        let n = data.nodes.len();
        Rig {
            data,
            graph,
            pos: ring_pos(n, 120.0),
            grown: vec![1.0; n],
            lit: Lit::default(),
            style: VizStyle::from_palette(&Theme::light().palette),
            fonts: Fonts::bundled_only(),
        }
    }

    fn frame(
        &self,
        hover: Option<usize>,
        selected: Option<usize>,
        as_of: Option<f64>,
    ) -> Frame<'_> {
        Frame {
            data: &self.data,
            graph: &self.graph,
            pos: &self.pos,
            camera: Camera::HOME,
            area: rect(0.0, 0.0, 400.0, 300.0),
            hover,
            selected,
            lit: &self.lit,
            as_of,
            grown: &self.grown,
            clock: 0.0,
            style: &self.style,
        }
    }

    fn draw(&self, f: &Frame) -> Vec<Command> {
        let mut c = Canvas::new(&self.fonts, 2.0);
        draw(&mut c, f);
        c.finish()
    }
}

fn words(items: &[Command]) -> Vec<String> {
    items
        .iter()
        .filter_map(|c| {
            if let Command::Text(t) = c {
                Some(t.layout.text().to_string())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn hit_finds_the_nearest_node_and_nothing_in_empty_space() {
    let rig = Rig::new();
    let f = rig.frame(None, None, None);
    let at = f.screen(3);
    assert_eq!(f.hit(pt(at.x + 2.0, at.y - 1.0)), Some(3));
    assert_eq!(f.hit(pt(200.0 + 500.0, 150.0)), None);
    assert_eq!(
        f.hit(pt(200.0, 150.0)),
        None,
        "the middle of the ring is empty"
    );
}

#[test]
fn a_small_graph_gets_every_label_and_a_legend() {
    let rig = Rig::new();
    let shown = words(&rig.draw(&rig.frame(None, None, None)));
    for i in 0..12 {
        assert!(
            shown.contains(&format!("Node {i}")),
            "Node {i} in {shown:?}"
        );
    }
    assert!(shown.contains(&"Company".to_string()) && shown.contains(&"Person".to_string()));
}

#[test]
fn a_big_graph_names_only_the_important_nodes_and_labels_never_overlap() {
    let mut d = GraphData::new();
    for i in 0..120u64 {
        d = d.node(Node::new(i, format!("Entity number {i}"), 0).weight(1.0 + (i % 7) as f32));
    }
    let graph = Compiled::new(&d);
    let pos: Vec<Point> = (0..120)
        .map(|i| {
            pt(
                (i % 3) as f32 * 130.0 - 130.0,
                (i / 3) as f32 * 24.0 - 140.0,
            )
        })
        .collect();
    let grown = vec![1.0; 120];
    let lit = Lit::default();
    let style = VizStyle::from_palette(&Theme::light().palette);
    let fonts = Fonts::bundled_only();
    let f = Frame {
        data: &d,
        graph: &graph,
        pos: &pos,
        camera: Camera::HOME,
        area: rect(0.0, 0.0, 400.0, 300.0),
        hover: None,
        selected: None,
        lit: &lit,
        as_of: None,
        grown: &grown,
        clock: 0.0,
        style: &style,
    };
    let mut c = Canvas::new(&fonts, 1.0);
    draw(&mut c, &f);
    let items = c.finish();
    let boxes: Vec<Rect> = items
        .iter()
        .filter_map(|c| {
            if let Command::Text(t) = c {
                Some(rect(
                    t.origin.x,
                    t.origin.y,
                    t.layout.size().w,
                    t.layout.size().h,
                ))
            } else {
                None
            }
        })
        .collect();
    assert!(boxes.len() > 3 && boxes.len() <= 21, "{}", boxes.len());
    for (i, a) in boxes.iter().enumerate() {
        for b in &boxes[i + 1..] {
            assert!(!a.intersects(*b), "{a:?} overlaps {b:?}");
        }
    }
}

#[test]
fn hovering_names_the_node_and_its_neighbours_and_shows_a_card() {
    let rig = Rig::new();
    let shown = words(&rig.draw(&rig.frame(Some(0), None, None)));
    assert!(
        shown.contains(&"Node 0".to_string())
            && shown.contains(&"Node 1".to_string())
            && shown.contains(&"Node 11".to_string())
    );
    assert!(shown.iter().any(|s| s.contains("2 links")), "{shown:?}");
    assert!(
        !shown.contains(&"Node 6".to_string()),
        "a far node steps back and loses its label"
    );
    assert!(
        shown.contains(&"rel".to_string()),
        "the links being looked at say what they are"
    );
}

#[test]
fn a_time_filter_hides_what_is_not_yet_true() {
    let mut rig = Rig::new();
    rig.data.nodes[5] = Node::new(5, "Node 5", 1).during(Some(100.0), None);
    rig.graph = Compiled::new(&rig.data);
    let early = words(&rig.draw(&rig.frame(None, None, Some(50.0))));
    let late = words(&rig.draw(&rig.frame(None, None, Some(150.0))));
    assert!(!early.contains(&"Node 5".to_string()) && late.contains(&"Node 5".to_string()));
    let f = rig.frame(None, None, Some(50.0));
    let at = f.screen(5);
    assert_eq!(f.hit(at), None, "a node that is not there cannot be picked");
}

#[test]
fn what_the_app_lights_up_is_named_and_pulses() {
    let mut rig = Rig::new();
    rig.lit = Lit {
        nodes: [2, 3, 4].into_iter().collect(),
        links: [2, 3].into_iter().collect(),
        pulse: vec![4],
    };
    let items = rig.draw(&rig.frame(None, None, None));
    let shown = words(&items);
    assert!(shown.contains(&"Node 3".to_string()) && !shown.contains(&"Node 9".to_string()));
    let rings = items
        .iter()
        .filter(|c| matches!(c, Command::Quad(q) if q.border.is_some()))
        .count();
    assert_eq!(rings, 2, "two pulse rings around the node to look at");
}

#[test]
fn nodes_not_grown_in_yet_are_faint_and_small() {
    let mut rig = Rig::new();
    rig.grown = vec![0.0; 12];
    let items = rig.draw(&rig.frame(None, None, None));
    let discs: Vec<_> = items
        .iter()
        .filter_map(|c| {
            if let Command::Quad(q) = c {
                Some(q)
            } else {
                None
            }
        })
        .filter(|q| q.rect.w < 20.0 && q.rect.w > 0.0 && q.rect.x > 40.0)
        .collect();
    assert!(!discs.is_empty());
    assert!(
        discs
            .iter()
            .all(|q| q.fill.a < 0.05 || q.border.is_some_and(|b| b.color.a < 0.05)),
        "invisible at the start"
    );
}

#[test]
fn long_names_are_cut_with_an_ellipsis() {
    assert_eq!(truncate("short", 10), "short");
    assert_eq!(
        truncate("a very long company name indeed", 10),
        "a very lo…"
    );
}
