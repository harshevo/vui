//! Drawing a graph: edges under nodes, a few labels chosen so they never pile on each other, and the
//! neighbourhood of whatever is hovered or selected kept bright while the rest steps back.

use std::collections::HashSet;

use vui_core::vector::{Path, Stroke, Transform};
use vui_core::{Color, Corners, Point, Quad, Rect, pt, rect};

use super::camera::Camera;
use super::data::{Compiled, GraphData, alive};
use crate::canvas::{Canvas, HAnchor, VAnchor};
use crate::style::VizStyle;

mod labels;

use labels::*;

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

#[cfg(test)]
mod tests;
