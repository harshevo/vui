//! SVG path data: parsing it, turning arcs into curves, and flattening curves into line segments.

use std::fmt;
use std::ops::{Add, Mul, Sub};

mod flatten;

pub use flatten::{Poly, flatten};

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

#[cfg(test)]
mod tests;
