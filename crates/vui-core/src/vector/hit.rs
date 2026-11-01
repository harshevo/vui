//! Picking: which shape is under the pointer. Works on flattened rings in the same space as the
//! pointer, so a zoomed graph asks the same questions as a still chart.

use crate::geom::{Point, pt};

use super::path::Ring;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum FillRule {
    /// Inside if a ray crosses the outline an odd number of times.
    #[default]
    EvenOdd,
    /// Inside if the outline winds around the point at all.
    NonZero,
}

/// Whether `p` is inside the filled shape the rings describe.
pub fn contains(rings: &[Ring], p: Point, rule: FillRule) -> bool {
    let mut crossings = 0;
    let mut winding = 0;
    for ring in rings {
        let n = ring.points.len();
        if n < 3 {
            continue;
        }
        for i in 0..n {
            let (a, b) = (ring.points[i], ring.points[(i + 1) % n]);
            if (a.y <= p.y) != (b.y <= p.y) {
                let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
                if x > p.x {
                    crossings += 1;
                    winding += if b.y > a.y { 1 } else { -1 };
                }
            }
        }
    }
    match rule {
        FillRule::EvenOdd => crossings % 2 == 1,
        FillRule::NonZero => winding != 0,
    }
}

/// Distance from `p` to the segment `a`–`b`.
pub fn segment_distance(a: Point, b: Point, p: Point) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= f32::EPSILON { 0.0 } else { (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0) };
    p.distance(pt(a.x + t * dx, a.y + t * dy))
}

/// Distance from `p` to the nearest point of the outline of the rings: how near a line is.
pub fn distance(rings: &[Ring], p: Point) -> f32 {
    let mut best = f32::MAX;
    for ring in rings {
        for w in ring.points.windows(2) {
            best = best.min(segment_distance(w[0], w[1], p));
        }
        if ring.closed && ring.points.len() > 2 {
            best = best.min(segment_distance(*ring.points.last().expect("non-empty"), ring.points[0], p));
        }
        if ring.points.len() == 1 {
            best = best.min(p.distance(ring.points[0]));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::rect;
    use crate::vector::{Path, Transform};

    fn rings(p: &Path) -> Vec<Ring> {
        p.flatten(&Transform::IDENTITY, 0.05)
    }

    #[test]
    fn inside_outside_and_holes() {
        let mut donut = Path::circle(pt(50.0, 50.0), 40.0);
        let hole = Path::circle(pt(50.0, 50.0), 20.0);
        donut = {
            let mut p = Path::new();
            for s in donut.segments().iter().chain(hole.segments()) {
                match *s {
                    crate::vector::Seg::Move(a) => p.move_to(a),
                    crate::vector::Seg::Line(a) => p.line_to(a),
                    crate::vector::Seg::Quad(c, a) => p.quad_to(c, a),
                    crate::vector::Seg::Cubic(a, b, c) => p.cubic_to(a, b, c),
                    crate::vector::Seg::Close => p.close(),
                };
            }
            p
        };
        let r = rings(&donut);
        assert!(contains(&r, pt(50.0, 20.0), FillRule::EvenOdd), "in the ring");
        assert!(!contains(&r, pt(50.0, 50.0), FillRule::EvenOdd), "in the hole");
        assert!(!contains(&r, pt(0.0, 0.0), FillRule::EvenOdd), "outside");
        // The two circles wind the same way, so non-zero fills the hole while even-odd does not.
        assert!(contains(&r, pt(50.0, 50.0), FillRule::NonZero));
    }

    #[test]
    fn distance_to_a_line_and_a_box() {
        let line = Path::polyline(&[pt(0.0, 0.0), pt(10.0, 0.0)], false);
        let r = rings(&line);
        assert!((distance(&r, pt(5.0, 3.0)) - 3.0).abs() < 1e-5);
        assert!((distance(&r, pt(13.0, 4.0)) - 5.0).abs() < 1e-5, "past the end it is the distance to the end");
        let b = rings(&Path::rect(rect(0.0, 0.0, 4.0, 4.0)));
        assert!((distance(&b, pt(2.0, 1.0)) - 1.0).abs() < 1e-5);
    }
}
