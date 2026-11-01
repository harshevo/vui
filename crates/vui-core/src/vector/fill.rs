//! Filled shapes. A polygon (with holes) is split into triangles by ear clipping; a one-pixel soft edge
//! is added by moving the outline half a pixel inward for the solid part and half a pixel outward for the
//! fringe, so the visible edge stays where the geometry says it is.

use crate::geom::{Point, pt};

use super::hit::{FillRule, contains};
use super::mesh::{Mesh, Rgba};
use super::path::Ring;

/// The signed area of a ring: positive when the left of each edge is the inside.
pub fn signed_area(points: &[Point]) -> f32 {
    let n = points.len();
    let mut a = 0.0;
    for i in 0..n {
        let (p, q) = (points[i], points[(i + 1) % n]);
        a += p.x * q.y - q.x * p.y;
    }
    a / 2.0
}

fn unit(x: f32, y: f32) -> (f32, f32) {
    let l = x.hypot(y);
    if l < 1e-12 { (0.0, 0.0) } else { (x / l, y / l) }
}

/// For each vertex, the points half a pixel into the filled side and half a pixel out of it.
/// `left_is_solid` says which side of the ring's edges is filled: the left for an outline wound the
/// positive way and for a hole wound the other way.
fn offset_ring(points: &[Point], left_is_solid: bool, f: f32) -> (Vec<Point>, Vec<Point>) {
    let n = points.len();
    let side = if left_is_solid { 1.0 } else { -1.0 };
    let edge_normal = |a: Point, b: Point| {
        let (dx, dy) = unit(b.x - a.x, b.y - a.y);
        (-dy * side, dx * side)
    };
    let (mut inner, mut outer) = (Vec::with_capacity(n), Vec::with_capacity(n));
    for i in 0..n {
        let (prev, here, next) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n]);
        let (n0, n1) = (edge_normal(prev, here), edge_normal(here, next));
        let (sx, sy) = (n0.0 + n1.0, n0.1 + n1.1);
        let half = (sx.hypot(sy) / 2.0).max(0.3);
        let (ux, uy) = unit(sx, sy);
        let k = f / half;
        inner.push(pt(here.x + ux * k, here.y + uy * k));
        outer.push(pt(here.x - ux * k, here.y - uy * k));
    }
    (inner, outer)
}

/// Adds a ring's soft edge and returns the ids of its solid vertices.
fn rim(mesh: &mut Mesh, points: &[Point], left_is_solid: bool, color: Rgba, px: f32) -> Vec<u32> {
    let (inner, outer) = offset_ring(points, left_is_solid, px / 2.0);
    let solid: Vec<u32> = inner.iter().map(|p| mesh.vertex(*p, color)).collect();
    let clear: Vec<u32> = outer.iter().map(|p| mesh.vertex(*p, [0.0; 4])).collect();
    let n = solid.len();
    for i in 0..n {
        let j = (i + 1) % n;
        mesh.quad(solid[i], clear[i], clear[j], solid[j]);
    }
    solid
}

fn dedup(points: &[Point]) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for p in points {
        if out.last().is_none_or(|q| q.distance(*p) > 1e-4) {
            out.push(*p);
        }
    }
    if out.len() > 1 && out[0].distance(out[out.len() - 1]) <= 1e-4 {
        out.pop();
    }
    out
}

/// Fills the rings. Rings nested inside one another alternate between filled and empty (even-odd), so a
/// ring inside another is a hole.
pub fn fill_rings(mesh: &mut Mesh, rings: &[Ring], color: Rgba, px: f32) {
    if color[3] <= 0.0 {
        return;
    }
    let polys: Vec<Vec<Point>> = rings.iter().map(|r| dedup(&r.points)).filter(|p| p.len() >= 3 && signed_area(p).abs() > 1e-6).collect();
    let as_rings: Vec<Ring> = polys.iter().map(|p| Ring { points: p.clone(), closed: true }).collect();
    // Depth: how many other rings enclose the ring's first point. Its parent is the smallest of them.
    let depth_and_parent: Vec<(usize, Option<usize>)> = (0..polys.len())
        .map(|i| {
            let probe = polys[i][0];
            let mut encl: Vec<usize> = (0..polys.len()).filter(|j| *j != i && contains(std::slice::from_ref(&as_rings[*j]), probe, FillRule::EvenOdd)).collect();
            encl.sort_by(|a, b| signed_area(&polys[*a]).abs().total_cmp(&signed_area(&polys[*b]).abs()));
            (encl.len(), encl.first().copied())
        })
        .collect();
    for (i, (depth, _)) in depth_and_parent.iter().enumerate() {
        if depth % 2 == 1 {
            continue;
        }
        let holes: Vec<&Vec<Point>> = (0..polys.len()).filter(|j| depth_and_parent[*j].0 % 2 == 1 && depth_and_parent[*j].1 == Some(i)).map(|j| &polys[j]).collect();
        polygon(mesh, &polys[i], &holes, color, px);
    }
}

/// One outline with holes.
pub fn polygon(mesh: &mut Mesh, outer: &[Point], holes: &[&Vec<Point>], color: Rgba, px: f32) {
    // Outer counter-clockwise by the shoelace sign, holes the other way.
    let orient = |p: &[Point], want_positive: bool| -> Vec<Point> {
        let mut v = p.to_vec();
        if (signed_area(&v) > 0.0) != want_positive {
            v.reverse();
        }
        v
    };
    let outer = orient(outer, true);
    let holes: Vec<Vec<Point>> = holes.iter().map(|h| orient(h, false)).collect();

    // The solid vertices carry the inset positions, which is what gets triangulated.
    let mut owner: Vec<u32> = rim(mesh, &outer, true, color, px);
    let mut ranges: Vec<std::ops::Range<usize>> = std::iter::once(0..owner.len()).collect();
    for h in &holes {
        // Wound the other way round, a hole has the filled region on its left too.
        let ids = rim(mesh, h, true, color, px);
        ranges.push(owner.len()..owner.len() + ids.len());
        owner.extend(ids);
    }
    let all: Vec<Point> = owner
        .iter()
        .map(|id| {
            let v = mesh.vertices[*id as usize].pos;
            pt(v[0], v[1])
        })
        .collect();
    for t in triangulate(&all, &ranges) {
        mesh.triangle(owner[t[0]], owner[t[1]], owner[t[2]]);
    }
}

/// The region between two polylines drawn left to right: an area under a line, a band between two
/// curves, a ribbon. No triangulation needed, so it is cheap for thousands of points.
pub fn band(mesh: &mut Mesh, upper: &[Point], lower: &[Point], color: Rgba, px: f32) {
    let n = upper.len().min(lower.len());
    if n < 2 || color[3] <= 0.0 {
        return;
    }
    let mut ring: Vec<Point> = upper[..n].to_vec();
    ring.extend(lower[..n].iter().rev());
    let positive = signed_area(&ring) >= 0.0;
    let ids = rim(mesh, &ring, positive, color, px);
    for i in 0..n - 1 {
        let (u0, u1) = (ids[i], ids[i + 1]);
        let (l0, l1) = (ids[2 * n - 1 - i], ids[2 * n - 2 - i]);
        mesh.triangle(u0, u1, l1);
        mesh.triangle(u0, l1, l0);
    }
}

// --- ear clipping ----------------------------------------------------------------------------------------

fn cross(o: Point, a: Point, b: Point) -> f32 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

fn in_triangle(a: Point, b: Point, c: Point, p: Point) -> bool {
    cross(a, b, p) >= 0.0 && cross(b, c, p) >= 0.0 && cross(c, a, p) >= 0.0
}

fn same(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6
}

fn segments_cross(a: Point, b: Point, c: Point, d: Point) -> bool {
    let (d1, d2, d3, d4) = (cross(a, b, c), cross(a, b, d), cross(c, d, a), cross(c, d, b));
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0)) && d1 != 0.0 && d2 != 0.0 && d3 != 0.0 && d4 != 0.0
}

/// Triangles (as indices into `points`) covering the outline `ranges[0]` minus the holes `ranges[1..]`.
/// The outline must run counter-clockwise by the shoelace sign and the holes clockwise.
fn triangulate(points: &[Point], ranges: &[std::ops::Range<usize>]) -> Vec<[usize; 3]> {
    let mut poly: Vec<usize> = ranges[0].clone().collect();
    // Bridge holes in, rightmost first, each to the nearest vertex it can see.
    let mut holes: Vec<std::ops::Range<usize>> = ranges[1..].to_vec();
    holes.sort_by(|a, b| {
        let right = |r: &std::ops::Range<usize>| r.clone().map(|i| points[i].x).fold(f32::MIN, f32::max);
        right(b).total_cmp(&right(a))
    });
    for hole in holes {
        let h = hole.clone().max_by(|a, b| points[*a].x.total_cmp(&points[*b].x)).expect("a hole has vertices");
        let mut candidates: Vec<usize> = (0..poly.len()).collect();
        candidates.sort_by(|a, b| points[h].distance(points[poly[*a]]).total_cmp(&points[h].distance(points[poly[*b]])));
        let visible = |v: usize| -> bool {
            let (hp, vp) = (points[h], points[poly[v]]);
            let mid = pt((hp.x + vp.x) / 2.0, (hp.y + vp.y) / 2.0);
            let ring = Ring { points: poly.iter().map(|i| points[*i]).collect(), closed: true };
            if !contains(std::slice::from_ref(&ring), mid, FillRule::EvenOdd) {
                return false;
            }
            let n = poly.len();
            for i in 0..n {
                let (a, b) = (points[poly[i]], points[poly[(i + 1) % n]]);
                if !same(a, vp) && !same(b, vp) && segments_cross(hp, vp, a, b) {
                    return false;
                }
            }
            let m = hole.len();
            for k in 0..m {
                let (a, b) = (points[hole.start + k], points[hole.start + (k + 1) % m]);
                if !same(a, hp) && !same(b, hp) && segments_cross(hp, vp, a, b) {
                    return false;
                }
            }
            true
        };
        let at = candidates.iter().copied().find(|v| visible(*v)).unwrap_or(candidates[0]);
        // Splice: ... V, H, (around the hole), H, V ...
        let start = h - hole.start;
        let m = hole.len();
        let mut walk: Vec<usize> = (0..=m).map(|k| hole.start + (start + k) % m).collect();
        walk.push(poly[at]);
        poly.splice(at + 1..at + 1, walk);
    }
    ear_clip(points, &poly)
}

fn ear_clip(points: &[Point], poly: &[usize]) -> Vec<[usize; 3]> {
    let mut out = Vec::with_capacity(poly.len().saturating_sub(2));
    let m = poly.len();
    if m < 3 {
        return out;
    }
    let mut next: Vec<usize> = (0..m).map(|i| (i + 1) % m).collect();
    let mut prev: Vec<usize> = (0..m).map(|i| (i + m - 1) % m).collect();
    let mut remaining = m;
    let mut cur = 0usize;
    let p = |i: usize| points[poly[i]];
    while remaining > 3 {
        let mut clipped = false;
        let mut i = cur;
        let tries = remaining;
        for _ in 0..tries {
            let (a, c) = (prev[i], next[i]);
            let ear = {
                let (pa, pb, pc) = (p(a), p(i), p(c));
                if same(pa, pb) || same(pb, pc) || cross(pa, pb, pc) <= 1e-9 {
                    false
                } else {
                    let mut ok = true;
                    let mut j = next[c];
                    while j != a {
                        let q = p(j);
                        if !same(q, pa) && !same(q, pb) && !same(q, pc) && in_triangle(pa, pb, pc, q) && cross(p(prev[j]), q, p(next[j])) <= 0.0 {
                            ok = false;
                            break;
                        }
                        j = next[j];
                    }
                    ok
                }
            };
            if ear {
                out.push([poly[a], poly[i], poly[c]]);
                next[a] = c;
                prev[c] = a;
                remaining -= 1;
                cur = a;
                clipped = true;
                break;
            }
            i = next[i];
        }
        if !clipped {
            // No ear exists (collinear runs or a self-touching outline). Drop a flat vertex if there is
            // one; otherwise cut the corner anyway so the loop always ends.
            let mut i = cur;
            let mut dropped = false;
            let tries = remaining;
            for _ in 0..tries {
                let (a, c) = (prev[i], next[i]);
                if cross(p(a), p(i), p(c)).abs() <= 1e-9 {
                    next[a] = c;
                    prev[c] = a;
                    remaining -= 1;
                    cur = a;
                    dropped = true;
                    break;
                }
                i = next[i];
            }
            if !dropped {
                let (a, c) = (prev[cur], next[cur]);
                out.push([poly[a], poly[cur], poly[c]]);
                next[a] = c;
                prev[c] = a;
                remaining -= 1;
                cur = a;
            }
        }
    }
    let (a, b, c) = (cur, next[cur], next[next[cur]]);
    if cross(p(a), p(b), p(c)).abs() > 1e-9 {
        out.push([poly[a], poly[b], poly[c]]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::rect;
    use crate::vector::mesh::raster::{coverage, sum};
    use crate::vector::{Path, Transform};

    const WHITE: Rgba = [1.0; 4];

    fn fill(path: &Path) -> Mesh {
        let mut m = Mesh::new();
        fill_rings(&mut m, &path.flatten(&Transform::IDENTITY, 0.05), WHITE, 1.0);
        m
    }

    #[test]
    fn a_rect_a_circle_and_an_l_shape_cover_their_areas() {
        let r = coverage(&fill(&Path::rect(rect(4.0, 4.0, 20.0, 10.0))), 30, 20);
        assert!((sum(&r) - 200.0).abs() < 3.0, "{}", sum(&r));
        let c = coverage(&fill(&Path::circle(pt(20.0, 20.0), 15.0)), 40, 40);
        assert!((sum(&c) - std::f32::consts::PI * 225.0).abs() < 8.0, "{}", sum(&c));
        let l = Path::polyline(&[pt(2.0, 2.0), pt(22.0, 2.0), pt(22.0, 12.0), pt(12.0, 12.0), pt(12.0, 22.0), pt(2.0, 22.0)], true);
        let cov = coverage(&fill(&l), 30, 30);
        assert!((sum(&cov) - 300.0).abs() < 4.0, "20×10 + 10×10: {}", sum(&cov));
        assert_eq!(cov[18 * 30 + 18], 0.0, "the notch is empty");
    }

    #[test]
    fn a_ring_inside_a_ring_is_a_hole() {
        let mut p = Path::circle(pt(25.0, 25.0), 20.0);
        let hole = Path::circle(pt(25.0, 25.0), 10.0);
        let mut both = Path::new();
        for s in p.segments().iter().chain(hole.segments()) {
            match *s {
                crate::vector::Seg::Move(a) => both.move_to(a),
                crate::vector::Seg::Line(a) => both.line_to(a),
                crate::vector::Seg::Quad(c, a) => both.quad_to(c, a),
                crate::vector::Seg::Cubic(a, b, c) => both.cubic_to(a, b, c),
                crate::vector::Seg::Close => both.close(),
            };
        }
        p = both;
        let cov = coverage(&fill(&p), 50, 50);
        let expected = std::f32::consts::PI * (400.0 - 100.0);
        assert!((sum(&cov) - expected).abs() < 12.0, "{} vs {expected}", sum(&cov));
        assert_eq!(cov[25 * 50 + 25], 0.0);
        assert!(cov[25 * 50 + 10] > 0.99);
    }

    #[test]
    fn a_band_between_two_curves_covers_the_gap() {
        let upper: Vec<Point> = (0..=20).map(|i| pt(i as f32, 5.0 + (i as f32 / 3.0).sin() * 2.0)).collect();
        let lower: Vec<Point> = (0..=20).map(|i| pt(i as f32, 15.0)).collect();
        let mut m = Mesh::new();
        band(&mut m, &upper, &lower, WHITE, 1.0);
        let true_area: f32 = upper.windows(2).zip(lower.windows(2)).map(|(u, l)| ((l[0].y - u[0].y) + (l[1].y - u[1].y)) / 2.0).sum();
        let cov = sum(&coverage(&m, 24, 20));
        assert!((cov - true_area).abs() < 0.04 * true_area + 8.0, "{cov} vs {true_area}");
    }

    #[test]
    fn a_band_works_whichever_way_it_winds() {
        let up: Vec<Point> = (0..10).map(|i| pt(i as f32 * 2.0, 4.0)).collect();
        let down: Vec<Point> = (0..10).map(|i| pt(i as f32 * 2.0, 12.0)).collect();
        let (mut a, mut b) = (Mesh::new(), Mesh::new());
        band(&mut a, &up, &down, WHITE, 1.0);
        band(&mut b, &down, &up, WHITE, 1.0);
        let (sa, sb) = (sum(&coverage(&a, 20, 16)), sum(&coverage(&b, 20, 16)));
        assert!((sa - sb).abs() < 1.0 && (sa - 144.0).abs() < 8.0, "{sa} {sb}");
    }

    #[test]
    fn nasty_outlines_end_and_never_panic() {
        // Collinear points, a repeated vertex, a bow-tie and a sliver.
        for pts in [
            vec![pt(0.0, 0.0), pt(5.0, 0.0), pt(10.0, 0.0), pt(10.0, 10.0)],
            vec![pt(0.0, 0.0), pt(0.0, 0.0), pt(10.0, 0.0), pt(10.0, 10.0), pt(0.0, 10.0)],
            vec![pt(0.0, 0.0), pt(10.0, 10.0), pt(10.0, 0.0), pt(0.0, 10.0)],
            vec![pt(0.0, 0.0), pt(100.0, 0.001), pt(100.0, 0.002)],
        ] {
            let mut m = Mesh::new();
            fill_rings(&mut m, &[Ring { points: pts, closed: true }], WHITE, 1.0);
            assert!(m.indices.iter().all(|i| (*i as usize) < m.vertices.len()));
        }
    }

    #[test]
    fn thousands_of_points_stay_fast() {
        let pts: Vec<Point> = (0..3000).map(|i| { let a = i as f32 / 3000.0 * std::f32::consts::TAU; pt(500.0 + a.cos() * (300.0 + (a * 7.0).sin() * 40.0), 400.0 + a.sin() * (300.0 + (a * 7.0).sin() * 40.0)) }).collect();
        let t = std::time::Instant::now();
        let mut m = Mesh::new();
        fill_rings(&mut m, &[Ring { points: pts, closed: true }], WHITE, 1.0);
        assert!(t.elapsed().as_millis() < 1500, "{:?}", t.elapsed());
        assert!(m.triangle_count() > 2900);
    }
}
