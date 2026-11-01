//! Drawing a graph: edges under nodes, a few labels chosen so they never pile on each other, and the
//! neighbourhood of whatever is hovered or selected kept bright while the rest steps back.

use std::collections::HashSet;

use vui_core::vector::{Path, Stroke, Transform};
use vui_core::{Color, Corners, Point, Quad, Rect, pt, rect};

use super::camera::Camera;
use super::data::{Compiled, GraphData, alive};
use crate::canvas::{Canvas, HAnchor, VAnchor};
use crate::style::VizStyle;

/// How much of its colour something outside the lit neighbourhood keeps.
const DIM: f32 = 0.2;

/// What an app has asked to light up, as positions in the node list rather than ids.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lit {
    pub nodes: HashSet<usize>,
    pub links: HashSet<usize>,
    /// Nodes that also send out rings, to say "look here".
    pub pulse: Vec<usize>,
}

impl Lit {
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.links.is_empty() && self.pulse.is_empty()
    }
}

pub struct Frame<'a> {
    pub data: &'a GraphData,
    pub graph: &'a Compiled,
    pub pos: &'a [Point],
    pub camera: Camera,
    pub area: Rect,
    pub hover: Option<usize>,
    pub selected: Option<usize>,
    pub lit: &'a Lit,
    pub as_of: Option<f64>,
    /// How far each node has grown in, from 0 to 1.
    pub grown: &'a [f32],
    pub clock: f32,
    pub style: &'a VizStyle,
}

/// The colour a kind of node is drawn in and whether it is a ring rather than a disc.
pub fn kind_look(style: &VizStyle, kind: u8) -> (Color, bool) {
    match kind % 6 {
        0 => (style.ink, false),
        1 => (style.accent, false),
        2 => (style.ink, true),
        3 => (style.accent, true),
        4 => (style.ink_3, false),
        _ => (style.ink_3, true),
    }
}

fn ease(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

impl Frame<'_> {
    fn mid(&self) -> Point {
        self.area.center()
    }

    pub fn screen(&self, i: usize) -> Point {
        self.camera.to_screen(self.pos[i], self.mid())
    }

    fn node_alive(&self, i: usize) -> bool {
        let n = &self.data.nodes[i];
        alive(n.since, n.until, self.as_of)
    }

    fn link_alive(&self, link: usize) -> bool {
        let (a, b, e) = self.graph.links[link];
        let edge = &self.data.edges[e];
        self.node_alive(a) && self.node_alive(b) && alive(edge.since, edge.until, self.as_of)
    }

    /// A node's radius on screen once fully grown.
    pub fn radius(&self, i: usize) -> f32 {
        let share = (self.data.nodes[i].weight / self.graph.heaviest).sqrt();
        (3.5 + 5.0 * share) * (0.8 + 0.2 * self.camera.zoom.clamp(0.5, 3.0))
    }

    /// The node under `p`, the nearest if several overlap.
    pub fn hit(&self, p: Point) -> Option<usize> {
        let mut best: Option<(f32, usize)> = None;
        for i in 0..self.pos.len() {
            if !self.node_alive(i) {
                continue;
            }
            let d = self.screen(i).distance(p);
            if d <= self.radius(i) + 3.0 && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, i));
            }
        }
        best.map(|(_, i)| i)
    }
}

pub fn draw(canvas: &mut Canvas, f: &Frame) {
    let style = f.style;
    let centre = f.hover.or(f.selected);
    // The nodes kept bright: what is lit, else a node and its neighbours.
    let bright: Option<HashSet<usize>> = if !f.lit.is_empty() {
        Some(f.lit.nodes.iter().chain(&f.lit.pulse).copied().collect())
    } else {
        centre.map(|c| {
            std::iter::once(c)
                .chain(f.graph.around[c].iter().map(|&(n, _)| n))
                .collect()
        })
    };
    let link_bright = |link: usize| -> bool {
        let (a, b, _) = f.graph.links[link];
        if !f.lit.is_empty() {
            f.lit.links.contains(&link)
                || (f.lit.links.is_empty() && f.lit.nodes.contains(&a) && f.lit.nodes.contains(&b))
        } else if let Some(c) = centre {
            a == c || b == c
        } else {
            true
        }
    };
    let node_alpha = |i: usize| -> f32 {
        let base = ease(f.grown.get(i).copied().unwrap_or(1.0));
        base * if bright.as_ref().is_none_or(|s| s.contains(&i)) {
            1.0
        } else {
            DIM
        }
    };
    let showing = f.area.inflate(48.0, 48.0);

    canvas.set_clip(Some(f.area));
    links(
        canvas,
        f,
        &link_bright,
        &node_alpha,
        centre.is_some() || !f.lit.is_empty(),
        &showing,
    );
    for i in 0..f.pos.len() {
        let s = f.screen(i);
        if !f.node_alive(i) || !showing.contains(s) {
            continue;
        }
        let grow = ease(f.grown.get(i).copied().unwrap_or(1.0));
        let r = f.radius(i) * (0.4 + 0.6 * grow);
        let (mut color, hollow) = kind_look(style, f.data.nodes[i].kind);
        if f.lit.nodes.contains(&i) || f.lit.pulse.contains(&i) {
            color = style.accent;
        }
        let a = node_alpha(i);
        if hollow {
            canvas.ring(s, r, 1.6, style.bg.mul_alpha(a), color.mul_alpha(a));
        } else {
            canvas.dot(s, r, color.mul_alpha(a));
        }
    }
    marks(canvas, f, centre);
    labels(canvas, f, centre, bright.as_ref(), &link_bright);
    canvas.set_clip(None);
    legend(canvas, f);
    if let Some(h) = f.hover {
        tooltip(canvas, f, h);
    }
}

fn links(
    canvas: &mut Canvas,
    f: &Frame,
    bright: &dyn Fn(usize) -> bool,
    node_alpha: &dyn Fn(usize) -> f32,
    lens: bool,
    showing: &Rect,
) {
    let style = f.style;
    let zoom = f.camera.zoom;
    for pass in [false, true] {
        for (link, &(a, b, e)) in f.graph.links.iter().enumerate() {
            let strong = bright(link);
            if strong != pass && lens || (!lens && pass) || !f.link_alive(link) {
                continue;
            }
            let (pa, pb) = (f.screen(a), f.screen(b));
            let span = Rect::from_min_max(
                pt(pa.x.min(pb.x), pa.y.min(pb.y)),
                pt(pa.x.max(pb.x), pa.y.max(pb.y)),
            );
            if !showing.intersects(span) {
                continue;
            }
            let edge = &f.data.edges[e];
            let weight = (edge.weight / 4.0).clamp(0.0, 1.0);
            let lit = f.lit.links.contains(&link);
            let (color, width) = match (lens, strong) {
                (true, true) if lit => (style.accent, 1.8),
                (true, true) => (style.ink_2.mul_alpha(0.9), 1.3 + weight * 0.6),
                (true, false) => (style.ink_3.mul_alpha(0.08), 0.8),
                _ => (style.ink_3.mul_alpha(0.5), 0.7 + weight * 0.8),
            };
            let fade = node_alpha(a).min(node_alpha(b)).clamp(0.0, 1.0);
            let color = color.mul_alpha(if lens { 1.0 } else { fade });
            let mut stroke = Stroke::new(width);
            if edge.dashed {
                stroke = stroke.dashed(&[4.0, 3.0], 0.0);
            }
            canvas.vector().polyline(&[pa, pb], &stroke, color);
            let len = pa.distance(pb);
            if len > 26.0 && (zoom >= 1.1 || strong && lens) {
                arrow(canvas, pa, pb, f.radius(b) + 2.0, color);
            }
        }
    }
}

/// An arrowhead at `to`, pulled back by `gap` so it does not sit under the node.
fn arrow(canvas: &mut Canvas, from: Point, to: Point, gap: f32, color: Color) {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let len = (dx * dx + dy * dy).sqrt().max(f32::EPSILON);
    let (ux, uy) = (dx / len, dy / len);
    let tip = pt(to.x - ux * gap, to.y - uy * gap);
    let base = pt(tip.x - ux * 7.0, tip.y - uy * 7.0);
    let head = Path::polyline(
        &[
            tip,
            pt(base.x - uy * 3.2, base.y + ux * 3.2),
            pt(base.x + uy * 3.2, base.y - ux * 3.2),
        ],
        true,
    );
    canvas.vector().fill(&head, &Transform::IDENTITY, color);
}

/// Rings on the hovered and selected nodes, and rings spreading out of the nodes the app wants noticed.
fn marks(canvas: &mut Canvas, f: &Frame, centre: Option<usize>) {
    let style = f.style;
    let clear = Color::rgba(0.0, 0.0, 0.0, 0.0);
    if let Some(h) = f.hover.filter(|h| f.node_alive(*h)) {
        canvas.ring(
            f.screen(h),
            f.radius(h) + 5.0,
            1.0,
            clear,
            style.accent.mul_alpha(0.4),
        );
    }
    if let Some(s) = f.selected.filter(|s| f.node_alive(*s)) {
        canvas.ring(f.screen(s), f.radius(s) + 3.5, 2.0, clear, style.accent);
    }
    for &i in &f.lit.pulse {
        if !f.node_alive(i) {
            continue;
        }
        for offset in [0.0, 0.5] {
            let phase = (f.clock * 0.8 + offset) % 1.0;
            canvas.ring(
                f.screen(i),
                f.radius(i) + 4.0 + 18.0 * ease(phase),
                1.5,
                clear,
                style.accent.mul_alpha((1.0 - phase) * 0.55),
            );
        }
    }
    let _ = centre;
}

fn labels(
    canvas: &mut Canvas,
    f: &Frame,
    centre: Option<usize>,
    bright: Option<&HashSet<usize>>,
    link_bright: &dyn Fn(usize) -> bool,
) {
    let n = f.pos.len();
    // Where labels may not go: on top of a node or of another label. Each remembers whose it is, so a
    // node's own label may sit beside its own disc.
    let mut placed: Vec<(usize, Rect)> = Vec::new();
    for i in 0..n {
        let s = f.screen(i);
        if f.node_alive(i) && f.area.inflate(20.0, 20.0).contains(s) {
            let r = f.radius(i) + 1.0;
            placed.push((i, rect(s.x - r, s.y - r, r * 2.0, r * 2.0)));
        }
    }
    let mut done = vec![false; n];
    // The nodes that always get a name, most important first. The hovered one is named by its card.
    let mut forced: Vec<usize> = f.selected.into_iter().collect();
    if let Some(set) = bright {
        forced.extend(
            f.graph
                .order
                .iter()
                .copied()
                .filter(|i| set.contains(i) && Some(*i) != f.selected && Some(*i) != f.hover)
                .take(40),
        );
    }
    if let Some(h) = f.hover {
        done[h] = true;
    }
    for i in forced {
        if done[i] || !f.node_alive(i) {
            continue;
        }
        done[i] = true;
        name(
            canvas,
            f,
            i,
            true,
            Some(i) == f.selected || Some(i) == centre,
            &mut placed,
        );
    }
    let budget = if n <= 60 {
        n
    } else {
        (f.camera.zoom * 14.0) as usize + 6
    };
    let mut shown = 0;
    for &i in &f.graph.order {
        if shown >= budget {
            break;
        }
        if done[i] || !f.node_alive(i) || bright.is_some() {
            continue;
        }
        let s = f.screen(i);
        if f.area.inflate(-4.0, -4.0).contains(s) && name(canvas, f, i, false, false, &mut placed) {
            shown += 1;
        }
        done[i] = true;
    }
    edge_names(canvas, f, bright.is_some(), link_bright, &mut placed);
}

/// Writes node `i`'s label beside it, on the right if there is room and the left if not, unless that would
/// land on a node or a label already there. Returns whether it did.
fn name(
    canvas: &mut Canvas,
    f: &Frame,
    i: usize,
    forced: bool,
    emphatic: bool,
    placed: &mut Vec<(usize, Rect)>,
) -> bool {
    let style = f.style;
    let text = truncate(&f.data.nodes[i].label, 28);
    let s = f.screen(i);
    let r = f.radius(i);
    let w = canvas.measure(&text, &style.label).w;
    let h = style.label.line_height;
    let fits_right = s.x + r + 5.0 + w <= f.area.right() - 2.0;
    let fits_left = s.x - r - 5.0 - w >= f.area.x + 2.0;
    let free = |spot: Rect| {
        !placed
            .iter()
            .any(|(owner, p)| *owner != i && p.intersects(spot.inflate(3.0, 1.0)))
    };
    let right = rect(s.x + r + 5.0, s.y - h / 2.0, w, h);
    let left = rect(s.x - r - 5.0 - w, s.y - h / 2.0, w, h);
    let (spot, on_right) = if fits_right && free(right) {
        (right, true)
    } else if fits_left && free(left) {
        (left, false)
    } else if forced {
        (if fits_right { right } else { left }, fits_right)
    } else {
        return false;
    };
    if forced {
        canvas.quad(
            Quad::new(spot.inflate(4.0, 1.0))
                .fill(style.bg.mul_alpha(0.85))
                .radii(Corners::all(3.0)),
        );
    }
    let color = if emphatic || forced {
        style.ink
    } else {
        style.ink_2
    };
    let (at, anchor) = if on_right {
        (pt(spot.x, s.y), HAnchor::Left)
    } else {
        (pt(spot.right(), s.y), HAnchor::Right)
    };
    canvas.text(&text, &style.label, at, anchor, VAnchor::Middle, color);
    placed.push((usize::MAX, spot.inflate(3.0, 1.0)));
    true
}

/// What each link says, where there is room: always for the links being looked at, and for the rest once
/// zoomed in far enough to read them.
fn edge_names(
    canvas: &mut Canvas,
    f: &Frame,
    lens: bool,
    link_bright: &dyn Fn(usize) -> bool,
    placed: &mut Vec<(usize, Rect)>,
) {
    let style = f.style;
    let zoomed = f.camera.zoom >= 2.0;
    let mut count = 0;
    for (link, &(a, b, e)) in f.graph.links.iter().enumerate() {
        if count >= 40 {
            break;
        }
        let wanted = if lens { link_bright(link) } else { zoomed };
        let text = &f.data.edges[e].label;
        if !wanted || text.is_empty() || !f.link_alive(link) {
            continue;
        }
        let (pa, pb) = (f.screen(a), f.screen(b));
        let mid = pt((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0);
        let w = canvas.measure(text, &style.label).w;
        if !f.area.contains(mid) || pa.distance(pb) < w + 24.0 {
            continue;
        }
        let spot = rect(
            mid.x - w / 2.0,
            mid.y - style.label.line_height / 2.0,
            w,
            style.label.line_height,
        );
        if placed
            .iter()
            .any(|(_, p)| p.intersects(spot.inflate(2.0, 1.0)))
        {
            continue;
        }
        canvas.quad(
            Quad::new(spot.inflate(4.0, 0.0))
                .fill(style.bg.mul_alpha(0.9))
                .radii(Corners::all(3.0)),
        );
        canvas.text(
            text,
            &style.label,
            mid,
            HAnchor::Center,
            VAnchor::Middle,
            style.ink_3,
        );
        placed.push((usize::MAX, spot));
        count += 1;
    }
}

/// The kinds on show, bottom left: a swatch and a name each. Skipped when there is only one kind.
fn legend(canvas: &mut Canvas, f: &Frame) {
    let style = f.style;
    let kinds: Vec<u8> = {
        let mut seen: Vec<u8> = f.data.nodes.iter().map(|n| n.kind).collect();
        seen.sort_unstable();
        seen.dedup();
        seen.into_iter()
            .filter(|k| (*k as usize) < f.data.kinds.len())
            .collect()
    };
    if kinds.len() < 2 {
        return;
    }
    let row = 18.0;
    let top = f.area.bottom() - 12.0 - row * kinds.len() as f32;
    for (n, k) in kinds.iter().enumerate() {
        let y = top + row * n as f32 + row / 2.0;
        let (color, hollow) = kind_look(style, *k);
        let at = pt(f.area.x + 18.0, y);
        if hollow {
            canvas.ring(at, 4.0, 1.5, style.bg, color);
        } else {
            canvas.dot(at, 4.0, color);
        }
        canvas.text(
            &f.data.kinds[*k as usize],
            &style.label,
            pt(f.area.x + 30.0, y),
            HAnchor::Left,
            VAnchor::Middle,
            style.ink_3,
        );
    }
}

fn tooltip(canvas: &mut Canvas, f: &Frame, i: usize) {
    let style = f.style;
    if !f.node_alive(i) {
        return;
    }
    let node = &f.data.nodes[i];
    let kind = f
        .data
        .kinds
        .get(node.kind as usize)
        .cloned()
        .unwrap_or_default();
    let links = f.graph.around[i].len();
    let mut lines: Vec<(String, Color)> = vec![(truncate(&node.label, 40), style.ink)];
    let sub = if kind.is_empty() {
        format!("{links} links")
    } else {
        format!("{kind} · {links} links")
    };
    lines.push((sub, style.ink_3));
    if !node.detail.is_empty() {
        lines.push((truncate(&node.detail, 48), style.ink_2));
    }
    let pad = 10.0;
    let line_h = 18.0;
    let w = lines
        .iter()
        .map(|(t, _)| canvas.measure(t, &style.label).w)
        .fold(0.0, f32::max)
        + pad * 2.0;
    let h = pad * 2.0 - 4.0 + line_h * lines.len() as f32;
    let s = f.screen(i);
    let r = f.radius(i);
    let x = if s.x + r + 12.0 + w > f.area.right() {
        s.x - r - 12.0 - w
    } else {
        s.x + r + 12.0
    };
    let y = (s.y - h / 2.0).clamp(
        f.area.y + 4.0,
        (f.area.bottom() - h - 4.0).max(f.area.y + 4.0),
    );
    let box_ = rect(x.max(f.area.x + 4.0), y, w, h);
    canvas.quad(
        Quad::new(box_)
            .fill(style.bg)
            .radii(Corners::all(6.0))
            .border(1.0, style.line),
    );
    for (n, (t, c)) in lines.iter().enumerate() {
        canvas.text(
            t,
            &style.label,
            pt(
                box_.x + pad,
                box_.y + pad - 2.0 + line_h * n as f32 + line_h / 2.0,
            ),
            HAnchor::Left,
            VAnchor::Middle,
            *c,
        );
    }
}

/// `text` cut to `max` characters, with an ellipsis when something was left off.
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
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
}
