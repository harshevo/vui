//! Paths: lines and curves, built up and flattened into polylines for filling and stroking.

use crate::geom::{Point, Rect, pt, rect};

use super::Transform;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Seg {
    Move(Point),
    Line(Point),
    Quad(Point, Point),
    Cubic(Point, Point, Point),
    Close,
}

/// A connected run of points, closed or not: what a path becomes once its curves are flattened.
#[derive(Clone, Debug, PartialEq)]
pub struct Ring {
    pub points: Vec<Point>,
    pub closed: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    segs: Vec<Seg>,
}

/// The cubic Bézier control distance for a quarter circle, as a share of the radius.
const KAPPA: f32 = 0.552_284_8;

impl Path {
    pub fn new() -> Path {
        Path::default()
    }

    pub fn segments(&self) -> &[Seg] {
        &self.segs
    }

    pub fn is_empty(&self) -> bool {
        self.segs.is_empty()
    }

    pub fn move_to(&mut self, p: Point) -> &mut Path {
        self.segs.push(Seg::Move(p));
        self
    }

    pub fn line_to(&mut self, p: Point) -> &mut Path {
        self.segs.push(Seg::Line(p));
        self
    }

    pub fn quad_to(&mut self, control: Point, to: Point) -> &mut Path {
        self.segs.push(Seg::Quad(control, to));
        self
    }

    pub fn cubic_to(&mut self, c1: Point, c2: Point, to: Point) -> &mut Path {
        self.segs.push(Seg::Cubic(c1, c2, to));
        self
    }

    pub fn close(&mut self) -> &mut Path {
        self.segs.push(Seg::Close);
        self
    }

    pub fn polyline(points: &[Point], closed: bool) -> Path {
        let mut p = Path::new();
        for (i, q) in points.iter().enumerate() {
            if i == 0 { p.move_to(*q) } else { p.line_to(*q) };
        }
        if closed && !points.is_empty() {
            p.close();
        }
        p
    }

    pub fn rect(r: Rect) -> Path {
        let mut p = Path::new();
        p.move_to(pt(r.x, r.y)).line_to(pt(r.right(), r.y)).line_to(pt(r.right(), r.bottom())).line_to(pt(r.x, r.bottom())).close();
        p
    }

    pub fn rounded_rect(r: Rect, radius: f32) -> Path {
        let k = radius.min(r.w / 2.0).min(r.h / 2.0).max(0.0);
        if k <= 0.0 {
            return Path::rect(r);
        }
        let c = k * KAPPA;
        let (x0, y0, x1, y1) = (r.x, r.y, r.right(), r.bottom());
        let mut p = Path::new();
        p.move_to(pt(x0 + k, y0))
            .line_to(pt(x1 - k, y0))
            .cubic_to(pt(x1 - k + c, y0), pt(x1, y0 + k - c), pt(x1, y0 + k))
            .line_to(pt(x1, y1 - k))
            .cubic_to(pt(x1, y1 - k + c), pt(x1 - k + c, y1), pt(x1 - k, y1))
            .line_to(pt(x0 + k, y1))
            .cubic_to(pt(x0 + k - c, y1), pt(x0, y1 - k + c), pt(x0, y1 - k))
            .line_to(pt(x0, y0 + k))
            .cubic_to(pt(x0, y0 + k - c), pt(x0 + k - c, y0), pt(x0 + k, y0))
            .close();
        p
    }

    pub fn ellipse(center: Point, rx: f32, ry: f32) -> Path {
        let (cx, cy) = (center.x, center.y);
        let (kx, ky) = (rx * KAPPA, ry * KAPPA);
        let mut p = Path::new();
        p.move_to(pt(cx + rx, cy))
            .cubic_to(pt(cx + rx, cy + ky), pt(cx + kx, cy + ry), pt(cx, cy + ry))
            .cubic_to(pt(cx - kx, cy + ry), pt(cx - rx, cy + ky), pt(cx - rx, cy))
            .cubic_to(pt(cx - rx, cy - ky), pt(cx - kx, cy - ry), pt(cx, cy - ry))
            .cubic_to(pt(cx + kx, cy - ry), pt(cx + rx, cy - ky), pt(cx + rx, cy))
            .close();
        p
    }

    pub fn circle(center: Point, radius: f32) -> Path {
        Path::ellipse(center, radius, radius)
    }

    /// An arc of a circle from `start` by `sweep` radians (positive is clockwise on screen), as a new
    /// subpath.
    pub fn arc(center: Point, radius: f32, start: f32, sweep: f32) -> Path {
        let mut p = Path::new();
        p.move_to(pt(center.x + radius * start.cos(), center.y + radius * start.sin()));
        p.arc_to(center, radius, start, sweep);
        p
    }

    /// Continues from the current point along an arc of the circle, which must start there.
    pub fn arc_to(&mut self, center: Point, radius: f32, start: f32, sweep: f32) -> &mut Path {
        let pieces = (sweep.abs() / std::f32::consts::FRAC_PI_2).ceil().max(1.0) as usize;
        let step = sweep / pieces as f32;
        let k = 4.0 / 3.0 * (step / 4.0).tan();
        let at = |a: f32| pt(center.x + radius * a.cos(), center.y + radius * a.sin());
        let tangent = |a: f32| pt(-radius * a.sin(), radius * a.cos());
        for i in 0..pieces {
            let (a0, a1) = (start + step * i as f32, start + step * (i + 1) as f32);
            let (p0, p1) = (at(a0), at(a1));
            let (t0, t1) = (tangent(a0), tangent(a1));
            self.cubic_to(pt(p0.x + k * t0.x, p0.y + k * t0.y), pt(p1.x - k * t1.x, p1.y - k * t1.y), p1);
        }
        self
    }

    /// The same path under `t`.
    pub fn transformed(&self, t: &Transform) -> Path {
        let m = |p: &Point| t.apply(*p);
        Path {
            segs: self
                .segs
                .iter()
                .map(|s| match s {
                    Seg::Move(p) => Seg::Move(m(p)),
                    Seg::Line(p) => Seg::Line(m(p)),
                    Seg::Quad(c, p) => Seg::Quad(m(c), m(p)),
                    Seg::Cubic(a, b, p) => Seg::Cubic(m(a), m(b), m(p)),
                    Seg::Close => Seg::Close,
                })
                .collect(),
        }
    }

    /// A box around every point of the path, including curve control points.
    pub fn bounds(&self) -> Rect {
        let (mut lo, mut hi) = (pt(f32::MAX, f32::MAX), pt(f32::MIN, f32::MIN));
        let mut see = |p: &Point| {
            lo = pt(lo.x.min(p.x), lo.y.min(p.y));
            hi = pt(hi.x.max(p.x), hi.y.max(p.y));
        };
        for s in &self.segs {
            match s {
                Seg::Move(p) | Seg::Line(p) => see(p),
                Seg::Quad(c, p) => [c, p].into_iter().for_each(&mut see),
                Seg::Cubic(a, b, p) => [a, b, p].into_iter().for_each(&mut see),
                Seg::Close => {}
            }
        }
        if lo.x > hi.x { Rect::ZERO } else { rect(lo.x, lo.y, hi.x - lo.x, hi.y - lo.y) }
    }

    /// Flattens under `t` into polylines whose distance from the true curve is at most `tolerance`
    /// (in the transformed space, so pass about a quarter of a device pixel).
    pub fn flatten(&self, t: &Transform, tolerance: f32) -> Vec<Ring> {
        let tol = tolerance.max(1e-4);
        let mut rings: Vec<Ring> = Vec::new();
        let mut cur: Vec<Point> = Vec::new();
        let mut start = Point::ZERO;
        let flush = |cur: &mut Vec<Point>, rings: &mut Vec<Ring>, closed: bool| {
            if cur.len() >= 2 || (closed && !cur.is_empty()) {
                rings.push(Ring { points: std::mem::take(cur), closed });
            } else {
                cur.clear();
            }
        };
        for s in &self.segs {
            match s {
                Seg::Move(p) => {
                    flush(&mut cur, &mut rings, false);
                    start = t.apply(*p);
                    cur.push(start);
                }
                Seg::Line(p) => {
                    if cur.is_empty() {
                        cur.push(start);
                    }
                    cur.push(t.apply(*p));
                }
                Seg::Quad(c, p) => {
                    if cur.is_empty() {
                        cur.push(start);
                    }
                    let p0 = *cur.last().expect("started");
                    let (c, p) = (t.apply(*c), t.apply(*p));
                    let dd = (p0.x - 2.0 * c.x + p.x).hypot(p0.y - 2.0 * c.y + p.y);
                    let n = ((dd / (4.0 * tol)).sqrt().ceil() as usize).clamp(1, 96);
                    for i in 1..=n {
                        let u = i as f32 / n as f32;
                        let w = 1.0 - u;
                        cur.push(pt(w * w * p0.x + 2.0 * w * u * c.x + u * u * p.x, w * w * p0.y + 2.0 * w * u * c.y + u * u * p.y));
                    }
                }
                Seg::Cubic(a, b, p) => {
                    if cur.is_empty() {
                        cur.push(start);
                    }
                    let p0 = *cur.last().expect("started");
                    let (a, b, p) = (t.apply(*a), t.apply(*b), t.apply(*p));
                    let d1 = (p0.x - 2.0 * a.x + b.x).hypot(p0.y - 2.0 * a.y + b.y);
                    let d2 = (a.x - 2.0 * b.x + p.x).hypot(a.y - 2.0 * b.y + p.y);
                    let n = ((0.75 * d1.max(d2) / tol).sqrt().ceil() as usize).clamp(1, 128);
                    for i in 1..=n {
                        let u = i as f32 / n as f32;
                        let w = 1.0 - u;
                        let (w3, w2u, wu2, u3) = (w * w * w, 3.0 * w * w * u, 3.0 * w * u * u, u * u * u);
                        cur.push(pt(w3 * p0.x + w2u * a.x + wu2 * b.x + u3 * p.x, w3 * p0.y + w2u * a.y + wu2 * b.y + u3 * p.y));
                    }
                }
                Seg::Close => {
                    flush(&mut cur, &mut rings, true);
                    // A subpath that continues after a close starts again where the closed one began.
                    cur.clear();
                }
            }
        }
        flush(&mut cur, &mut rings, false);
        rings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_circle_flattens_within_tolerance() {
        let c = Path::circle(pt(50.0, 50.0), 40.0);
        let tol = 0.1;
        let rings = c.flatten(&Transform::IDENTITY, tol);
        assert_eq!(rings.len(), 1);
        assert!(rings[0].closed);
        for p in &rings[0].points {
            let r = (p.x - 50.0).hypot(p.y - 50.0);
            assert!((r - 40.0).abs() < 0.06, "every vertex sits on the circle (the cubic itself is only 0.03% off): {r}");
        }
        // And the chords between vertices stay within tolerance of the circle.
        for w in rings[0].points.windows(2) {
            let mid = pt((w[0].x + w[1].x) / 2.0, (w[0].y + w[1].y) / 2.0);
            let r = (mid.x - 50.0).hypot(mid.y - 50.0);
            assert!(40.0 - r < tol + 0.06, "chord sags {}", 40.0 - r);
        }
    }

    #[test]
    fn zoom_makes_curves_finer() {
        let c = Path::circle(Point::ZERO, 10.0);
        let coarse = c.flatten(&Transform::IDENTITY, 0.1)[0].points.len();
        let fine = c.flatten(&Transform::scale(10.0, 10.0), 0.1)[0].points.len();
        assert!(fine > coarse * 2, "{coarse} vs {fine}");
    }

    #[test]
    fn arcs_start_and_end_where_they_say() {
        let a = Path::arc(pt(0.0, 0.0), 10.0, 0.0, std::f32::consts::PI);
        let ring = &a.flatten(&Transform::IDENTITY, 0.05)[0];
        assert!(!ring.closed);
        let (first, last) = (ring.points[0], *ring.points.last().unwrap());
        assert!((first.x - 10.0).abs() < 1e-4 && first.y.abs() < 1e-4);
        assert!((last.x + 10.0).abs() < 1e-3 && last.y.abs() < 1e-3, "{last:?}");
    }

    #[test]
    fn bounds_and_rounded_rect_corners() {
        let r = Path::rounded_rect(rect(10.0, 20.0, 100.0, 50.0), 8.0);
        assert_eq!(r.bounds(), rect(10.0, 20.0, 100.0, 50.0));
        assert_eq!(Path::new().bounds(), Rect::ZERO);
        // A radius larger than the box is capped to half the short side.
        let pill = Path::rounded_rect(rect(0.0, 0.0, 40.0, 10.0), 99.0);
        assert_eq!(pill.bounds(), rect(0.0, 0.0, 40.0, 10.0));
    }

    #[test]
    fn subpaths_after_a_close_start_fresh() {
        let mut p = Path::new();
        p.move_to(pt(0.0, 0.0)).line_to(pt(1.0, 0.0)).line_to(pt(1.0, 1.0)).close();
        p.move_to(pt(5.0, 5.0)).line_to(pt(6.0, 5.0));
        let rings = p.flatten(&Transform::IDENTITY, 0.1);
        assert_eq!(rings.len(), 2);
        assert!(rings[0].closed && !rings[1].closed);
        assert_eq!(rings[1].points, vec![pt(5.0, 5.0), pt(6.0, 5.0)]);
    }
}
