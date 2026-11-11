//! 2D affine transforms: what scales, rotates and moves a layer of the scene.

use crate::{Point, Rect, pt};

/// Maps `(x, y)` to `(a·x + c·y + e, b·x + d·y + f)`, in logical pixels.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Affine {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl Affine {
    pub const IDENTITY: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    pub const fn translate(x: f32, y: f32) -> Affine {
        Affine { e: x, f: y, ..Affine::IDENTITY }
    }

    pub const fn scale(sx: f32, sy: f32) -> Affine {
        Affine { a: sx, d: sy, ..Affine::IDENTITY }
    }

    /// Turns by `degrees`, clockwise on screen (y points down).
    pub fn rotate(degrees: f32) -> Affine {
        let (s, c) = degrees.to_radians().sin_cos();
        Affine { a: c, b: s, c: -s, d: c, e: 0.0, f: 0.0 }
    }

    /// This transform, then `next`.
    pub fn then(self, next: Affine) -> Affine {
        Affine {
            a: next.a * self.a + next.c * self.b,
            b: next.b * self.a + next.d * self.b,
            c: next.a * self.c + next.c * self.d,
            d: next.b * self.c + next.d * self.d,
            e: next.a * self.e + next.c * self.f + next.e,
            f: next.b * self.e + next.d * self.f + next.f,
        }
    }

    /// This transform applied about `origin` instead of about (0, 0): scaling a card about its
    /// centre.
    pub fn around(self, origin: Point) -> Affine {
        Affine::translate(-origin.x, -origin.y).then(self).then(Affine::translate(origin.x, origin.y))
    }

    pub fn apply(&self, p: Point) -> Point {
        pt(self.a * p.x + self.c * p.y + self.e, self.b * p.x + self.d * p.y + self.f)
    }

    /// The transform that undoes this one, if it can be undone.
    pub fn inverse(&self) -> Option<Affine> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < 1e-9 {
            return None;
        }
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Some(Affine { a, b, c, d, e: -(a * self.e + c * self.f), f: -(b * self.e + d * self.f) })
    }

    pub fn is_identity(&self) -> bool {
        *self == Affine::IDENTITY
    }

    /// The four corners of `r` once transformed: top left, top right, bottom left, bottom right.
    pub fn corners(&self, r: Rect) -> [Point; 4] {
        [pt(r.x, r.y), pt(r.x + r.w, r.y), pt(r.x, r.y + r.h), pt(r.x + r.w, r.y + r.h)].map(|p| self.apply(p))
    }

    /// The smallest upright rect holding `r` once transformed.
    pub fn bounds(&self, r: Rect) -> Rect {
        let c = self.corners(r);
        let (x0, x1) = c.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| (lo.min(p.x), hi.max(p.x)));
        let (y0, y1) = c.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| (lo.min(p.y), hi.max(p.y)));
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
}

impl Default for Affine {
    fn default() -> Affine {
        Affine::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(p: Point, q: Point) -> bool {
        (p.x - q.x).abs() < 1e-4 && (p.y - q.y).abs() < 1e-4
    }

    #[test]
    fn transforms_compose_in_order() {
        let m = Affine::scale(2.0, 2.0).then(Affine::translate(10.0, 0.0));
        assert!(close(m.apply(pt(1.0, 1.0)), pt(12.0, 2.0)));
        let r = Affine::rotate(90.0);
        assert!(close(r.apply(pt(1.0, 0.0)), pt(0.0, 1.0)), "clockwise on screen");
    }

    #[test]
    fn around_keeps_the_origin_still_and_inverse_undoes() {
        let m = Affine::scale(0.5, 0.5).then(Affine::rotate(30.0)).around(pt(50.0, 20.0));
        assert!(close(m.apply(pt(50.0, 20.0)), pt(50.0, 20.0)));
        let inv = m.inverse().unwrap();
        for p in [pt(0.0, 0.0), pt(13.0, -7.0), pt(100.0, 40.0)] {
            assert!(close(inv.apply(m.apply(p)), p));
        }
        assert!(Affine::scale(0.0, 1.0).inverse().is_none());
    }

    #[test]
    fn bounds_hold_a_rotated_square() {
        let b = Affine::rotate(45.0).around(pt(5.0, 5.0)).bounds(Rect::new(0.0, 0.0, 10.0, 10.0));
        assert!((b.w - 10.0 * 2f32.sqrt()).abs() < 1e-3);
    }
}
