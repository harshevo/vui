//! Lines with width: caps, joins, dashes, and a soft one-pixel edge.
//!
//! A stroke becomes a run of cross-sections, one at each vertex (or several where a join needs them).
//! Each section is four vertices across the line: transparent, solid, solid, transparent. Joining
//! consecutive sections with quads gives the body and the antialiased edge in one go.

use std::f32::consts::{PI, TAU};

use crate::geom::{Point, Vec2, pt, vec2};

use super::mesh::{Mesh, Rgba, scaled};
use super::path::Ring;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Cap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Join {
    #[default]
    Miter,
    Round,
    Bevel,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub cap: Cap,
    pub join: Join,
    /// A miter longer than this many half-widths is cut to a bevel.
    pub miter_limit: f32,
    /// Lengths of dash and gap, alternating; empty for a solid line.
    pub dash: Vec<f32>,
    pub dash_offset: f32,
}

impl Stroke {
    pub fn new(width: f32) -> Stroke {
        Stroke { width, cap: Cap::Butt, join: Join::Miter, miter_limit: 4.0, dash: Vec::new(), dash_offset: 0.0 }
    }

    pub fn cap(mut self, cap: Cap) -> Stroke {
        self.cap = cap;
        self
    }

    pub fn join(mut self, join: Join) -> Stroke {
        self.join = join;
        self
    }

    pub fn dashed(mut self, pattern: &[f32], offset: f32) -> Stroke {
        self.dash = pattern.to_vec();
        self.dash_offset = offset;
        self
    }
}

struct Ctx {
    /// Half the line's width (at least half a pixel).
    hw: f32,
    /// Half the width of the soft edge.
    f: f32,
    solid: Rgba,
    px: f32,
}

#[derive(Copy, Clone)]
struct Section {
    c: Point,
    n: Vec2,
    m: f32,
}

fn normal(d: Vec2) -> Vec2 {
    vec2(-d.y, d.x)
}

fn unit(v: Vec2) -> Vec2 {
    let l = v.length();
    if l < 1e-12 { vec2(1.0, 0.0) } else { vec2(v.x / l, v.y / l) }
}

fn rotate(v: Vec2, a: f32) -> Vec2 {
    let (s, c) = a.sin_cos();
    vec2(v.x * c - v.y * s, v.x * s + v.y * c)
}

fn at(c: Point, d: Vec2, k: f32) -> Point {
    pt(c.x + d.x * k, c.y + d.y * k)
}

/// Appends the triangles for `rings` drawn with `stroke`. `px` is the size of a device pixel in the
/// rings' space: it sets the softness of the edge and the finest arcs worth drawing.
pub fn stroke_rings(mesh: &mut Mesh, rings: &[Ring], stroke: &Stroke, color: Rgba, px: f32) {
    if stroke.width <= 0.0 || color[3] <= 0.0 {
        return;
    }
    let dashed;
    let rings = if stroke.dash.iter().sum::<f32>() > 0.0 {
        dashed = dash(rings, &stroke.dash, stroke.dash_offset);
        &dashed[..]
    } else {
        rings
    };
    // Lines thinner than a pixel are drawn a pixel wide and faded, which is how thin lines look right.
    let (hw, k) = if stroke.width < px { (px / 2.0, stroke.width / px) } else { (stroke.width / 2.0, 1.0) };
    let cx = Ctx { hw, f: px / 2.0, solid: scaled(color, k), px };
    for ring in rings {
        stroke_ring(mesh, ring, stroke, &cx);
    }
}

fn stroke_ring(mesh: &mut Mesh, ring: &Ring, st: &Stroke, cx: &Ctx) {
    let mut pts: Vec<Point> = Vec::with_capacity(ring.points.len());
    for p in &ring.points {
        if pts.last().is_none_or(|q| q.distance(*p) > 1e-4) {
            pts.push(*p);
        }
    }
    let mut closed = ring.closed;
    if closed && pts.len() > 1 && pts[0].distance(pts[pts.len() - 1]) <= 1e-4 {
        pts.pop();
    }
    if pts.len() == 1 {
        if st.cap != Cap::Butt {
            cap_fan(mesh, pts[0], vec2(1.0, 0.0), TAU, cx);
        }
        return;
    }
    if pts.len() < 2 {
        return;
    }
    if closed && pts.len() < 3 {
        closed = false;
    }
    let n = pts.len();
    let segments = if closed { n } else { n - 1 };
    let dirs: Vec<Vec2> = (0..segments).map(|i| unit(pts[(i + 1) % n] - pts[i])).collect();
    let mut sections: Vec<Section> = Vec::new();
    if closed {
        for i in 0..n {
            join(&mut sections, pts[i], dirs[(i + n - 1) % n], dirs[i], st, cx);
        }
    } else {
        let start = if st.cap == Cap::Square { at(pts[0], dirs[0], -cx.hw) } else { pts[0] };
        sections.push(Section { c: start, n: normal(dirs[0]), m: 1.0 });
        for i in 1..n - 1 {
            join(&mut sections, pts[i], dirs[i - 1], dirs[i], st, cx);
        }
        let end = if st.cap == Cap::Square { at(pts[n - 1], dirs[n - 2], cx.hw) } else { pts[n - 1] };
        sections.push(Section { c: end, n: normal(dirs[n - 2]), m: 1.0 });
    }
    emit(mesh, &sections, closed, cx);
    if !closed && st.cap == Cap::Round {
        cap_fan(mesh, pts[0], normal(dirs[0]), PI, cx);
        cap_fan(mesh, pts[n - 1], normal(dirs[n - 2]), -PI, cx);
    }
}

/// Pushes the section(s) for a corner where the line turns from direction `d0` to `d1`.
fn join(out: &mut Vec<Section>, c: Point, d0: Vec2, d1: Vec2, st: &Stroke, cx: &Ctx) {
    let (n0, n1) = (normal(d0), normal(d1));
    let dot = d0.x * d1.x + d0.y * d1.y;
    if dot > 0.9999 {
        out.push(Section { c, n: n1, m: 1.0 });
        return;
    }
    let sum = vec2(n0.x + n1.x, n0.y + n1.y);
    let len = sum.length();
    if st.join == Join::Miter && len > 1e-4 {
        let m = 2.0 / len;
        if m <= st.miter_limit {
            out.push(Section { c, n: vec2(sum.x / len, sum.y / len), m });
            return;
        }
    }
    let theta = (n0.x * n1.y - n0.y * n1.x).atan2(n0.x * n1.x + n0.y * n1.y);
    let steps = if st.join == Join::Round { arc_steps(theta, cx) } else { 1 };
    for k in 0..=steps {
        out.push(Section { c, n: rotate(n0, theta * k as f32 / steps as f32), m: 1.0 });
    }
}

/// How many pieces an arc of `angle` needs for its chords to stay a quarter pixel from the circle.
fn arc_steps(angle: f32, cx: &Ctx) -> usize {
    let tol = (cx.px * 0.25).max(1e-4);
    if cx.hw <= tol {
        return 1;
    }
    let step = 2.0 * (1.0 - tol / cx.hw).clamp(-1.0, 1.0).acos();
    ((angle.abs() / step.max(0.05)).ceil() as usize).clamp(1, 64)
}

/// Joins consecutive sections with quads: body plus the soft edges on both sides.
fn emit(mesh: &mut Mesh, sections: &[Section], closed: bool, cx: &Ctx) {
    let clear = [0.0; 4];
    let ids: Vec<[u32; 4]> = sections
        .iter()
        .map(|s| {
            let outer = cx.hw * s.m + cx.f;
            let inner = (cx.hw * s.m - cx.f).max(0.0);
            [
                mesh.vertex(at(s.c, s.n, outer), clear),
                mesh.vertex(at(s.c, s.n, inner), cx.solid),
                mesh.vertex(at(s.c, s.n, -inner), cx.solid),
                mesh.vertex(at(s.c, s.n, -outer), clear),
            ]
        })
        .collect();
    let link = |mesh: &mut Mesh, a: &[u32; 4], b: &[u32; 4]| {
        for j in 0..3 {
            mesh.quad(a[j], a[j + 1], b[j + 1], b[j]);
        }
    };
    for w in ids.windows(2) {
        link(mesh, &w[0], &w[1]);
    }
    if closed && ids.len() > 2 {
        link(mesh, &ids[ids.len() - 1], &ids[0]);
    }
}

/// A round cap: the sector of a disc around `c`, starting along `n` and turning by `sweep`.
fn cap_fan(mesh: &mut Mesh, c: Point, n: Vec2, sweep: f32, cx: &Ctx) {
    let clear = [0.0; 4];
    let steps = arc_steps(sweep, cx).max(2);
    let center = mesh.vertex(c, cx.solid);
    let (inner_r, outer_r) = ((cx.hw - cx.f).max(0.0), cx.hw + cx.f);
    let ring: Vec<(u32, u32)> = (0..=steps)
        .map(|k| {
            let d = rotate(n, sweep * k as f32 / steps as f32);
            (mesh.vertex(at(c, d, inner_r), cx.solid), mesh.vertex(at(c, d, outer_r), clear))
        })
        .collect();
    for w in ring.windows(2) {
        mesh.triangle(center, w[0].0, w[1].0);
        mesh.quad(w[0].0, w[0].1, w[1].1, w[1].0);
    }
}

/// Cuts rings into dashes. `pattern` alternates dash and gap lengths; an odd count repeats with the roles swapped.
pub fn dash(rings: &[Ring], pattern: &[f32], offset: f32) -> Vec<Ring> {
    let pattern: Vec<f32> = if !pattern.len().is_multiple_of(2) { [pattern, pattern].concat() } else { pattern.to_vec() };
    let total: f32 = pattern.iter().map(|p| p.max(0.0)).sum();
    if total <= 0.0 {
        return rings.to_vec();
    }
    let mut out = Vec::new();
    for ring in rings {
        let mut pts = ring.points.clone();
        if ring.closed && pts.len() > 1 {
            pts.push(pts[0]);
        }
        // Start part-way into the pattern.
        let mut idx = 0usize;
        let mut skip = offset.rem_euclid(total);
        while skip >= pattern[idx % pattern.len()].max(0.0) && skip > 0.0 {
            skip -= pattern[idx % pattern.len()].max(0.0);
            idx += 1;
        }
        let mut remaining = pattern[idx % pattern.len()].max(0.0) - skip;
        let mut cur: Vec<Point> = Vec::new();
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = a.distance(b);
            if len <= 1e-6 {
                continue;
            }
            let d = unit(b - a);
            let mut pos = 0.0;
            while pos < len {
                let on = idx.is_multiple_of(2);
                let step = remaining.min(len - pos);
                if on {
                    if cur.is_empty() {
                        cur.push(at(a, d, pos));
                    }
                    cur.push(at(a, d, pos + step));
                }
                pos += step;
                remaining -= step;
                if remaining <= 1e-6 {
                    if on && !cur.is_empty() {
                        out.push(Ring { points: std::mem::take(&mut cur), closed: false });
                    }
                    idx += 1;
                    remaining = pattern[idx % pattern.len()].max(0.0);
                    // Zero-length elements would never advance; give them a hair of length.
                    if remaining == 0.0 {
                        remaining = 1e-5;
                    }
                }
            }
        }
        if !cur.is_empty() {
            out.push(Ring { points: cur, closed: false });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::mesh::raster::{coverage, sum};
    use crate::vector::{Path, Transform};

    const WHITE: Rgba = [1.0; 4];

    fn line(a: Point, b: Point) -> Vec<Ring> {
        vec![Ring { points: vec![a, b], closed: false }]
    }

    #[test]
    fn a_straight_line_covers_length_times_width() {
        let mut m = Mesh::new();
        stroke_rings(&mut m, &line(pt(5.0, 10.0), pt(35.0, 10.0)), &Stroke::new(4.0), WHITE, 1.0);
        let cov = coverage(&m, 40, 20);
        assert!((sum(&cov) - 120.0).abs() < 4.0, "30 × 4 = 120, got {}", sum(&cov));
        assert!((cov[10 * 40 + 20] - 1.0).abs() < 1e-4);
        assert_eq!(cov[2 * 40 + 20], 0.0);
    }

    #[test]
    fn lines_under_a_pixel_are_faded_not_dropped() {
        let mut m = Mesh::new();
        stroke_rings(&mut m, &line(pt(5.0, 10.3), pt(35.0, 10.3)), &Stroke::new(0.5), WHITE, 1.0);
        let total = sum(&coverage(&m, 40, 20));
        assert!((total - 15.0).abs() < 3.0, "half a pixel of width over 30 long: {total}");
    }

    #[test]
    fn caps_extend_the_line_by_the_right_amount() {
        let area = |cap| {
            let mut m = Mesh::new();
            stroke_rings(&mut m, &line(pt(10.0, 10.0), pt(30.0, 10.0)), &Stroke::new(8.0).cap(cap), WHITE, 1.0);
            sum(&coverage(&m, 40, 20))
        };
        let butt = area(Cap::Butt);
        assert!((butt - 160.0).abs() < 5.0, "{butt}");
        assert!((area(Cap::Square) - butt - 64.0).abs() < 6.0, "two half-widths of 8");
        let round = area(Cap::Round) - butt;
        assert!((round - std::f32::consts::PI * 16.0).abs() < 5.0, "a disc of radius 4: {round}");
    }

    #[test]
    fn a_corner_is_filled_by_every_join_and_the_miter_limit_bevels_spikes() {
        let corner = vec![Ring { points: vec![pt(5.0, 25.0), pt(5.0, 5.0), pt(25.0, 5.0)], closed: false }];
        let area = |join, limit| {
            let mut m = Mesh::new();
            let mut s = Stroke::new(6.0).join(join);
            s.miter_limit = limit;
            stroke_rings(&mut m, &corner, &s, WHITE, 1.0);
            sum(&coverage(&m, 32, 32))
        };
        let (miter, bevel, round) = (area(Join::Miter, 4.0), area(Join::Bevel, 4.0), area(Join::Round, 4.0));
        assert!(miter > round && round > bevel, "square > round > bevel: {miter} {round} {bevel}");
        assert!((miter - 240.0).abs() < 8.0, "two 20×6 bars, their overlap, and the outer corner: {miter}");
        // A very sharp turn exceeds the limit and is cut, so it is no bigger than the bevel version.
        let spike = vec![Ring { points: vec![pt(2.0, 2.0), pt(28.0, 15.0), pt(2.0, 17.0)], closed: false }];
        let mut a = Mesh::new();
        let mut s = Stroke::new(4.0);
        s.miter_limit = 2.0;
        stroke_rings(&mut a, &spike, &s, WHITE, 1.0);
        assert!(a.bounds().right() < 31.0, "no spike past the corner: {:?}", a.bounds());
    }

    #[test]
    fn a_closed_circle_is_a_ring_with_no_gap() {
        let c = Path::circle(pt(20.0, 20.0), 12.0).flatten(&Transform::IDENTITY, 0.1);
        let mut m = Mesh::new();
        stroke_rings(&mut m, &c, &Stroke::new(3.0).join(Join::Round), WHITE, 1.0);
        let cov = coverage(&m, 40, 40);
        let expected = std::f32::consts::PI * (13.5f32.powi(2) - 10.5f32.powi(2));
        assert!((sum(&cov) - expected).abs() < 8.0, "{} vs {expected}", sum(&cov));
        assert!(cov[20 * 40 + 20] == 0.0, "the middle is empty");
    }

    #[test]
    fn dashes_cut_the_line_and_the_offset_slides_them() {
        let rings = line(pt(0.0, 0.0), pt(100.0, 0.0));
        let d = dash(&rings, &[10.0, 10.0], 0.0);
        assert_eq!(d.len(), 5);
        assert!((d[0].points[0].x - 0.0).abs() < 1e-4 && (d[0].points[1].x - 10.0).abs() < 1e-4);
        assert!((d[1].points[0].x - 20.0).abs() < 1e-4);
        let shifted = dash(&rings, &[10.0, 10.0], 5.0);
        assert!((shifted[0].points[1].x - 5.0).abs() < 1e-4, "starts five into the first dash: {:?}", shifted[0]);
        // Dashes on a stroked line cover half of it.
        let mut m = Mesh::new();
        stroke_rings(&mut m, &line(pt(0.0, 5.0), pt(40.0, 5.0)), &Stroke::new(2.0).dashed(&[4.0, 4.0], 0.0), WHITE, 1.0);
        assert!((sum(&coverage(&m, 40, 10)) - 40.0).abs() < 5.0);
    }

    #[test]
    fn degenerate_input_draws_nothing_or_a_dot_and_never_panics() {
        let mut m = Mesh::new();
        stroke_rings(&mut m, &[Ring { points: vec![], closed: false }], &Stroke::new(2.0), WHITE, 1.0);
        stroke_rings(&mut m, &[Ring { points: vec![pt(1.0, 1.0), pt(1.0, 1.0)], closed: false }], &Stroke::new(2.0), WHITE, 1.0);
        assert!(m.is_empty());
        stroke_rings(&mut m, &[Ring { points: vec![pt(5.0, 5.0)], closed: false }], &Stroke::new(6.0).cap(Cap::Round), WHITE, 1.0);
        assert!((sum(&coverage(&m, 12, 12)) - std::f32::consts::PI * 9.0).abs() < 4.0, "a round dot");
        stroke_rings(&mut m, &line(pt(0.0, 0.0), pt(1.0, 0.0)), &Stroke::new(0.0), WHITE, 1.0);
        stroke_rings(&mut m, &line(pt(0.0, 0.0), pt(10.0, 0.0)), &Stroke::new(2.0), [0.0; 4], 1.0);
    }
}
