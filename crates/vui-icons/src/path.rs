//! SVG path data: parsing it, turning arcs into curves, and flattening curves into line segments.

use std::fmt;
use std::ops::{Add, Mul, Sub};

/// A point in an icon's coordinate space.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct P {
    pub x: f32,
    pub y: f32,
}

pub const fn p(x: f32, y: f32) -> P {
    P { x, y }
}

impl Add for P {
    type Output = P;
    fn add(self, o: P) -> P {
        p(self.x + o.x, self.y + o.y)
    }
}

impl Sub for P {
    type Output = P;
    fn sub(self, o: P) -> P {
        p(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for P {
    type Output = P;
    fn mul(self, k: f32) -> P {
        p(self.x * k, self.y * k)
    }
}

impl P {
    pub fn cross(self, o: P) -> f32 {
        self.x * o.y - self.y * o.x
    }

    pub fn dot(self, o: P) -> f32 {
        self.x * o.x + self.y * o.y
    }

    pub fn len(self) -> f32 {
        self.x.hypot(self.y)
    }

    fn mid(self, o: P) -> P {
        p((self.x + o.x) * 0.5, (self.y + o.y) * 0.5)
    }
}

/// One drawing step, in absolute coordinates. Arcs are already cubic curves.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Seg {
    Move(P),
    Line(P),
    /// Control point, end.
    Quad(P, P),
    /// Two control points, end.
    Cubic(P, P, P),
    Close,
}

impl Seg {
    pub fn scaled(self, k: f32) -> Seg {
        match self {
            Seg::Move(a) => Seg::Move(a * k),
            Seg::Line(a) => Seg::Line(a * k),
            Seg::Quad(a, b) => Seg::Quad(a * k, b * k),
            Seg::Cubic(a, b, c) => Seg::Cubic(a * k, b * k, c * k),
            Seg::Close => Seg::Close,
        }
    }
}

/// What is wrong with a path string, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathError {
    pub offset: usize,
    pub message: &'static str,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bad path data at byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for PathError {}

struct Scanner<'a> {
    s: &'a [u8],
    i: usize,
}

impl Scanner<'_> {
    fn err(&self, message: &'static str) -> PathError {
        PathError { offset: self.i, message }
    }

    fn skip_separators(&mut self) {
        while self.s.get(self.i).is_some_and(|c| c.is_ascii_whitespace() || *c == b',') {
            self.i += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_separators();
        self.s.get(self.i).copied()
    }

    fn number(&mut self) -> Result<f32, PathError> {
        self.skip_separators();
        let (s, start) = (self.s, self.i);
        let mut i = start;
        if matches!(s.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let mut digits = 0;
        while s.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
            digits += 1;
        }
        if s.get(i) == Some(&b'.') {
            i += 1;
            while s.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
                digits += 1;
            }
        }
        if digits == 0 {
            return Err(self.err("expected a number"));
        }
        if matches!(s.get(i), Some(b'e' | b'E')) {
            let mut j = i + 1;
            if matches!(s.get(j), Some(b'+' | b'-')) {
                j += 1;
            }
            let exp = j;
            while s.get(j).is_some_and(u8::is_ascii_digit) {
                j += 1;
            }
            if j > exp {
                i = j;
            }
        }
        // Only ASCII digits, signs, dots and exponents were consumed.
        let text = std::str::from_utf8(&s[start..i]).map_err(|_| self.err("expected a number"))?;
        let value = text.parse::<f32>().map_err(|_| self.err("expected a number"))?;
        self.i = i;
        Ok(value)
    }

    fn pair(&mut self) -> Result<P, PathError> {
        let x = self.number()?;
        let y = self.number()?;
        Ok(p(x, y))
    }

    /// Arc flags are single characters and may touch the number after them.
    fn flag(&mut self) -> Result<bool, PathError> {
        self.skip_separators();
        match self.s.get(self.i) {
            Some(b'0') => {
                self.i += 1;
                Ok(false)
            }
            Some(b'1') => {
                self.i += 1;
                Ok(true)
            }
            _ => Err(self.err("expected an arc flag, 0 or 1")),
        }
    }
}

/// Parses SVG path data: `M L H V C S Q T A Z`, absolute and relative, with repeated implicit
/// commands and compact number and flag spacing.
pub fn parse(d: &str) -> Result<Vec<Seg>, PathError> {
    let mut sc = Scanner { s: d.as_bytes(), i: 0 };
    let mut segs = Vec::new();
    let mut cur = P::default();
    let mut start = P::default();
    let mut prev: Option<u8> = None;
    // The control point a smooth curve command reflects.
    let mut cubic_ctrl: Option<P> = None;
    let mut quad_ctrl: Option<P> = None;
    // A subpath is under way: after Z the next drawing command begins a new one at the old start.
    let mut open = false;

    while let Some(c) = sc.peek() {
        let cmd = if c.is_ascii_alphabetic() {
            sc.i += 1;
            c
        } else {
            // Numbers where a command should be repeat the last one; after a moveto they draw lines.
            match prev {
                Some(b'M') => b'L',
                Some(b'm') => b'l',
                Some(b'Z' | b'z') | None => return Err(sc.err("expected a command")),
                Some(other) => other,
            }
        };
        if prev.is_none() && !matches!(cmd, b'M' | b'm') {
            return Err(sc.err("a path starts with a moveto"));
        }
        let rel = cmd.is_ascii_lowercase();
        let base = if rel { cur } else { P::default() };
        let upper = cmd.to_ascii_uppercase();
        if !matches!(upper, b'M' | b'Z') && !open {
            segs.push(Seg::Move(cur));
            open = true;
            start = cur;
        }
        let (was_cubic, was_quad) = (cubic_ctrl.take(), quad_ctrl.take());
        match upper {
            b'M' => {
                cur = base + sc.pair()?;
                start = cur;
                open = true;
                segs.push(Seg::Move(cur));
            }
            b'L' => {
                cur = base + sc.pair()?;
                segs.push(Seg::Line(cur));
            }
            b'H' => {
                let x = sc.number()?;
                cur = p(if rel { cur.x + x } else { x }, cur.y);
                segs.push(Seg::Line(cur));
            }
            b'V' => {
                let y = sc.number()?;
                cur = p(cur.x, if rel { cur.y + y } else { y });
                segs.push(Seg::Line(cur));
            }
            b'C' => {
                let (c1, c2, end) = (base + sc.pair()?, base + sc.pair()?, base + sc.pair()?);
                segs.push(Seg::Cubic(c1, c2, end));
                cubic_ctrl = Some(c2);
                cur = end;
            }
            b'S' => {
                let c1 = was_cubic.map_or(cur, |c| cur * 2.0 - c);
                let (c2, end) = (base + sc.pair()?, base + sc.pair()?);
                segs.push(Seg::Cubic(c1, c2, end));
                cubic_ctrl = Some(c2);
                cur = end;
            }
            b'Q' => {
                let (c1, end) = (base + sc.pair()?, base + sc.pair()?);
                segs.push(Seg::Quad(c1, end));
                quad_ctrl = Some(c1);
                cur = end;
            }
            b'T' => {
                let c1 = was_quad.map_or(cur, |c| cur * 2.0 - c);
                let end = base + sc.pair()?;
                segs.push(Seg::Quad(c1, end));
                quad_ctrl = Some(c1);
                cur = end;
            }
            b'A' => {
                let (rx, ry, rotation) = (sc.number()?, sc.number()?, sc.number()?);
                let (large, sweep) = (sc.flag()?, sc.flag()?);
                let end = base + sc.pair()?;
                arc(cur, rx, ry, rotation, large, sweep, end, &mut segs);
                cur = end;
            }
            b'Z' => {
                if open {
                    segs.push(Seg::Close);
                }
                cur = start;
                open = false;
            }
            _ => return Err(PathError { offset: sc.i - 1, message: "unknown command" }),
        }
        prev = Some(cmd);
    }
    Ok(segs)
}

/// Appends the cubic curves for an SVG elliptical arc from `from` to `to` (SVG 1.1, F.6.5).
#[allow(clippy::too_many_arguments)]
fn arc(from: P, rx: f32, ry: f32, degrees: f32, large: bool, sweep: bool, to: P, out: &mut Vec<Seg>) {
    if from == to {
        return;
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 {
        out.push(Seg::Line(to));
        return;
    }
    let phi = degrees.to_radians();
    let (sin, cos) = phi.sin_cos();
    // Step 1: the chord in the ellipse's own axes, halved.
    let half = (from - to) * 0.5;
    let (x1, y1) = (cos * half.x + sin * half.y, -sin * half.x + cos * half.y);
    // Radii too small to reach are scaled up until they just do.
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        let k = lambda.sqrt();
        rx *= k;
        ry *= k;
    }
    // Step 2: the centre in the ellipse's axes.
    let (rx2, ry2) = (rx * rx, ry * ry);
    let num = rx2 * ry2 - rx2 * y1 * y1 - ry2 * x1 * x1;
    let den = rx2 * y1 * y1 + ry2 * x1 * x1;
    let mut k = if den == 0.0 { 0.0 } else { (num / den).max(0.0).sqrt() };
    if large == sweep {
        k = -k;
    }
    let (cx1, cy1) = (k * rx * y1 / ry, -k * ry * x1 / rx);
    // Step 3: back to user space.
    let mid = from.mid(to);
    let centre = p(cos * cx1 - sin * cy1 + mid.x, sin * cx1 + cos * cy1 + mid.y);
    // Step 4: where the arc starts and how far it goes.
    let angle = |ux: f32, uy: f32, vx: f32, vy: f32| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let (ux, uy) = ((x1 - cx1) / rx, (y1 - cy1) / ry);
    let (vx, vy) = ((-x1 - cx1) / rx, (-y1 - cy1) / ry);
    let theta = angle(1.0, 0.0, ux, uy);
    let mut delta = angle(ux, uy, vx, vy);
    if !sweep && delta > 0.0 {
        delta -= std::f32::consts::TAU;
    } else if sweep && delta < 0.0 {
        delta += std::f32::consts::TAU;
    }
    // Pieces of at most a quarter turn approximate an ellipse to within a thousandth.
    let pieces = (delta.abs() / std::f32::consts::FRAC_PI_2 - 1e-3).ceil().max(1.0) as usize;
    let step = delta / pieces as f32;
    let handle = 4.0 / 3.0 * (step / 4.0).tan();
    let map = |x: f32, y: f32| p(cos * rx * x - sin * ry * y + centre.x, sin * rx * x + cos * ry * y + centre.y);
    for i in 0..pieces {
        let (a, b) = (theta + step * i as f32, theta + step * (i + 1) as f32);
        let (sa, ca) = a.sin_cos();
        let (sb, cb) = b.sin_cos();
        let c1 = map(ca - handle * sa, sa + handle * ca);
        let c2 = map(cb + handle * sb, sb - handle * cb);
        let end = if i + 1 == pieces { to } else { map(cb, sb) };
        out.push(Seg::Cubic(c1, c2, end));
    }
}

/// A run of straight segments, closed or open.
#[derive(Clone, Debug, PartialEq)]
pub struct Poly {
    pub pts: Vec<P>,
    pub closed: bool,
}

impl Poly {
    /// The line segments, including the one that closes a closed outline. A single point gives one
    /// zero-length segment, which a round-capped stroke draws as a dot.
    pub fn edges(&self) -> impl Iterator<Item = (P, P)> + '_ {
        let n = self.pts.len();
        let count = if n == 1 { 1 } else if self.closed { n } else { n.saturating_sub(1) };
        (0..count).map(move |i| (self.pts[i], self.pts[(i + 1) % n]))
    }
}

/// Turns curves into short lines that stay within `tol` of the true curve.
pub fn flatten(segs: &[Seg], tol: f32) -> Vec<Poly> {
    let mut out = Vec::new();
    let mut cur: Option<Poly> = None;
    let mut last = P::default();
    for &seg in segs {
        match seg {
            Seg::Move(a) => {
                out.extend(cur.take());
                cur = Some(Poly { pts: vec![a], closed: false });
                last = a;
            }
            Seg::Close => {
                if let Some(mut poly) = cur.take() {
                    poly.closed = true;
                    last = poly.pts[0];
                    out.push(poly);
                }
            }
            drawing => {
                let poly = cur.get_or_insert_with(|| Poly { pts: vec![last], closed: false });
                match drawing {
                    Seg::Line(a) => poly.pts.push(a),
                    Seg::Quad(c, a) => {
                        let (c1, c2) = (last + (c - last) * (2.0 / 3.0), a + (c - a) * (2.0 / 3.0));
                        cubic(last, c1, c2, a, tol, 0, &mut poly.pts);
                    }
                    Seg::Cubic(c1, c2, a) => cubic(last, c1, c2, a, tol, 0, &mut poly.pts),
                    Seg::Move(_) | Seg::Close => unreachable!("handled above"),
                }
                last = *poly.pts.last().expect("a polyline has a point");
            }
        }
    }
    out.extend(cur);
    // A lone moveto draws nothing.
    out.retain(|poly| poly.pts.len() > 1 || poly.closed);
    out
}

fn cubic(p0: P, p1: P, p2: P, p3: P, tol: f32, depth: u32, out: &mut Vec<P>) {
    let chord = p3 - p0;
    let len = chord.len();
    let (d1, d2) = if len < 1e-6 {
        ((p1 - p0).len(), (p2 - p0).len())
    } else {
        ((p1 - p0).cross(chord).abs() / len, (p2 - p0).cross(chord).abs() / len)
    };
    if depth >= 16 || d1.max(d2) <= tol {
        out.push(p3);
        return;
    }
    let (p01, p12, p23) = (p0.mid(p1), p1.mid(p2), p2.mid(p3));
    let (p012, p123) = (p01.mid(p12), p12.mid(p23));
    let m = p012.mid(p123);
    cubic(p0, p01, p012, m, tol, depth + 1, out);
    cubic(m, p123, p23, p3, tol, depth + 1, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(d: &str) -> Vec<Seg> {
        parse(d).unwrap_or_else(|e| panic!("{d:?}: {e}"))
    }

    fn near(a: P, b: P) -> bool {
        (a - b).len() < 1e-3
    }

    #[test]
    fn absolute_and_relative_lines() {
        assert_eq!(pts("M1 2L3 4"), [Seg::Move(p(1.0, 2.0)), Seg::Line(p(3.0, 4.0))]);
        assert_eq!(pts("m1 2l3 4"), [Seg::Move(p(1.0, 2.0)), Seg::Line(p(4.0, 6.0))]);
        assert_eq!(
            pts("M1 1H5V4h-2v-1"),
            [Seg::Move(p(1.0, 1.0)), Seg::Line(p(5.0, 1.0)), Seg::Line(p(5.0, 4.0)), Seg::Line(p(3.0, 4.0)), Seg::Line(p(3.0, 3.0))]
        );
    }

    #[test]
    fn numbers_come_in_every_spelling() {
        let a = pts("M.5.5L-1.5-2.5 +3,4e0 1E1 -1e-1");
        assert_eq!(a[0], Seg::Move(p(0.5, 0.5)));
        assert_eq!(a[1], Seg::Line(p(-1.5, -2.5)));
        assert_eq!(a[2], Seg::Line(p(3.0, 4.0)), "an implicit lineto repeats");
        assert_eq!(a[3], Seg::Line(p(10.0, -0.1)));
        assert_eq!(pts("M1,2,3,4"), [Seg::Move(p(1.0, 2.0)), Seg::Line(p(3.0, 4.0))], "numbers after a moveto are linetos");
        assert_eq!(pts("m1 2 3 4")[1], Seg::Line(p(4.0, 6.0)), "and relative ones stay relative");
        assert_eq!(pts("M1 1 5.5.5")[1], Seg::Line(p(5.5, 0.5)), "a second dot starts a new number");
    }

    #[test]
    fn closing_returns_to_the_start_and_a_later_command_begins_again_there() {
        let a = pts("M2 2h4v4zl1 1");
        assert_eq!(a[3], Seg::Close);
        assert_eq!(a[4], Seg::Move(p(2.0, 2.0)));
        assert_eq!(a[5], Seg::Line(p(3.0, 3.0)));
        assert!(matches!(pts("M0 0 z Z").as_slice(), [Seg::Move(_), Seg::Close]), "closing twice closes once");
    }

    #[test]
    fn smooth_curves_reflect_the_previous_control_point() {
        let a = pts("M0 0C0 4 4 4 4 0S8-4 8 0");
        assert_eq!(a[2], Seg::Cubic(p(4.0, -4.0), p(8.0, -4.0), p(8.0, 0.0)));
        let b = pts("M0 0Q2 4 4 0T8 0");
        assert_eq!(b[2], Seg::Quad(p(6.0, -4.0), p(8.0, 0.0)));
        // Without a curve before it, the reflected point is the current point.
        let c = pts("M0 0S2 2 4 0");
        assert_eq!(c[1], Seg::Cubic(p(0.0, 0.0), p(2.0, 2.0), p(4.0, 0.0)));
    }

    #[test]
    fn mistakes_say_where() {
        assert_eq!(parse("L1 1").unwrap_err().message, "a path starts with a moveto");
        assert_eq!(parse("M1").unwrap_err().message, "expected a number");
        assert_eq!(parse("M1 1 X").unwrap_err().message, "unknown command");
        assert_eq!(parse("M1 1 z 2 2").unwrap_err().message, "expected a command");
        assert_eq!(parse("M0 0a1 1 0 2 0 1 1").unwrap_err().message, "expected an arc flag, 0 or 1");
        assert_eq!(parse("").unwrap(), []);
        assert_eq!(parse("M1 é").unwrap_err().message, "expected a number", "stray non-ASCII bytes are an error, not a panic");
    }

    /// Distance of every flattened point of `d` from the point `c`.
    fn radii(d: &str, c: P, tol: f32) -> Vec<f32> {
        flatten(&pts(d), tol).iter().flat_map(|poly| poly.pts.iter().map(|q| (*q - c).len())).collect()
    }

    #[test]
    fn arcs_stay_on_their_circle() {
        // Two half circles make a circle of radius 8 around (12, 12).
        for d in ["M12 4a8 8 0 1 0 0 16a8 8 0 1 0 0-16z", "M12 4A8 8 0 0 1 12 20A8 8 0 0 1 12 4z", "M20 12a8 8 0 0 1-8 8 8 8 0 0 1-8-8 8 8 0 0 1 8-8 8 8 0 0 1 8 8z"] {
            let r = radii(d, p(12.0, 12.0), 0.01);
            assert!(r.len() > 30, "{d}: {} points", r.len());
            for r in r {
                assert!((r - 8.0).abs() < 0.02, "{d}: a point is {r} from the centre");
            }
        }
    }

    #[test]
    fn arc_flags_choose_between_the_four_arcs() {
        // From (0,0) to (8,0) with radius 5: the centre is above or below, and the arc is short or long.
        let mid_y = |d: &str| {
            let poly = &flatten(&pts(d), 0.01)[0];
            poly.pts.iter().map(|q| q.y).fold(0.0f32, |a, b| if b.abs() > a.abs() { b } else { a })
        };
        assert!((mid_y("M0 0A5 5 0 0 0 8 0") - 2.0).abs() < 0.05, "short, counter-clockwise: bulges down");
        assert!((mid_y("M0 0A5 5 0 0 1 8 0") + 2.0).abs() < 0.05, "short, clockwise: bulges up");
        assert!((mid_y("M0 0A5 5 0 1 0 8 0") - 8.0).abs() < 0.05, "long, counter-clockwise: bulges down");
        assert!((mid_y("M0 0A5 5 0 1 1 8 0") + 8.0).abs() < 0.05, "long, clockwise: bulges up");
    }

    #[test]
    fn radii_that_are_too_small_grow_until_the_arc_fits() {
        let poly = &flatten(&pts("M0 0A1 1 0 0 1 10 0"), 0.01)[0];
        let top = poly.pts.iter().map(|q| q.y).fold(0.0f32, f32::min);
        assert!((top + 5.0).abs() < 0.05, "a half circle of radius 5, got height {top}");
        assert!(poly.pts.last().is_some_and(|q| near(*q, p(10.0, 0.0))), "and it ends where it should");
    }

    #[test]
    fn an_arc_ends_exactly_where_it_was_told_to() {
        for (d, end) in [
            ("M2 3a6 3 30 0 1 8 4", p(10.0, 7.0)),
            ("M2 3A6 3 -45 1 0 8 4", p(8.0, 4.0)),
            ("M2 3a0 3 0 0 1 4 4", p(6.0, 7.0)),
            ("M2 3a3 0 0 0 1 4 4", p(6.0, 7.0)),
        ] {
            let got = flatten(&pts(d), 0.01)[0].pts.last().copied().unwrap();
            assert!(near(got, end), "{d}: ends at {got:?}");
        }
    }

    #[test]
    fn a_zero_radius_arc_is_a_line() {
        assert_eq!(pts("M0 0a0 5 0 0 1 4 4")[1], Seg::Line(p(4.0, 4.0)));
    }

    #[test]
    fn a_curve_flattens_to_within_the_tolerance_and_finer_tolerance_makes_more_points() {
        let segs = pts("M0 0C0 10 10 10 10 0");
        let coarse = flatten(&segs, 0.5);
        let fine = flatten(&segs, 0.01);
        assert!(fine[0].pts.len() > coarse[0].pts.len() * 2, "{} vs {}", fine[0].pts.len(), coarse[0].pts.len());
        // The curve's apex is at (5, 7.5); the flattened path must come within tolerance of it.
        let apex = fine[0].pts.iter().map(|q| q.y).fold(0.0f32, f32::max);
        assert!((apex - 7.5).abs() < 0.02, "apex {apex}");
        assert!(coarse[0].pts.len() >= 4);
    }

    #[test]
    fn a_straight_curve_is_one_segment() {
        let poly = &flatten(&pts("M0 0C1 1 2 2 3 3"), 0.01)[0];
        assert_eq!(poly.pts.len(), 2);
    }

    #[test]
    fn a_closed_loop_with_coincident_ends_does_not_recurse_forever() {
        let poly = &flatten(&pts("M0 0C10 0 10 10 0 0"), 0.01)[0];
        assert!(poly.pts.len() > 8 && poly.pts.len() < 5000, "{}", poly.pts.len());
    }

    #[test]
    fn flattening_groups_points_by_subpath() {
        let polys = flatten(&pts("M0 0h4v4zM10 10l2 2M20 20"), 0.1);
        assert_eq!(polys.len(), 2, "a lone moveto at the end draws nothing");
        assert!(polys[0].closed && polys[0].pts.len() == 3);
        assert!(!polys[1].closed && polys[1].pts.len() == 2);
        assert_eq!(polys[0].edges().count(), 3, "the closing edge counts");
        assert_eq!(polys[1].edges().count(), 1);
    }

    #[test]
    fn a_dot_is_a_zero_length_segment() {
        let polys = flatten(&pts("M5 5z"), 0.1);
        assert_eq!(polys.len(), 1);
        let edges: Vec<_> = polys[0].edges().collect();
        assert_eq!(edges, [(p(5.0, 5.0), p(5.0, 5.0))]);
        assert_eq!(flatten(&pts("M5 5h0"), 0.1)[0].pts.len(), 2, "a zero-length line is kept");
    }

    #[test]
    fn scaling_moves_every_point() {
        let a = Seg::Cubic(p(1.0, 2.0), p(3.0, 4.0), p(5.0, 6.0)).scaled(2.0);
        assert_eq!(a, Seg::Cubic(p(2.0, 4.0), p(6.0, 8.0), p(10.0, 12.0)));
    }
}
