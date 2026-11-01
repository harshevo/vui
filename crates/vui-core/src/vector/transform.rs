//! A 2D affine transform: how a drawing's own coordinates become window coordinates, so a chart or a
//! graph can be panned, zoomed and rotated without being drawn again by its author.

use crate::geom::{Point, Vec2, pt, vec2};

/// Maps `(x, y)` to `(a·x + c·y + e, b·x + d·y + f)`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Transform {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Transform::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Transform = Transform { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    pub fn translate(dx: f32, dy: f32) -> Transform {
        Transform { e: dx, f: dy, ..Transform::IDENTITY }
    }

    pub fn scale(sx: f32, sy: f32) -> Transform {
        Transform { a: sx, d: sy, ..Transform::IDENTITY }
    }

    /// A turn by `radians`; positive turns the x axis toward the y axis (clockwise on screen).
    pub fn rotate(radians: f32) -> Transform {
        let (s, c) = radians.sin_cos();
        Transform { a: c, b: s, c: -s, d: c, e: 0.0, f: 0.0 }
    }

    /// Does `self` first, then `next`.
    pub fn then(self, next: Transform) -> Transform {
        Transform {
            a: next.a * self.a + next.c * self.b,
            b: next.b * self.a + next.d * self.b,
            c: next.a * self.c + next.c * self.d,
            d: next.b * self.c + next.d * self.d,
            e: next.a * self.e + next.c * self.f + next.e,
            f: next.b * self.e + next.d * self.f + next.f,
        }
    }

    /// A zoom by `k` that keeps the window point `fixed` where it is: the wheel and pinch.
    pub fn zoom_about(self, fixed: Point, k: f32) -> Transform {
        self.then(Transform::translate(-fixed.x, -fixed.y)).then(Transform::scale(k, k)).then(Transform::translate(fixed.x, fixed.y))
    }

    pub fn apply(&self, p: Point) -> Point {
        pt(self.a * p.x + self.c * p.y + self.e, self.b * p.x + self.d * p.y + self.f)
    }

    /// Moves a displacement: no translation.
    pub fn apply_vec(&self, v: Vec2) -> Vec2 {
        vec2(self.a * v.x + self.c * v.y, self.b * v.x + self.d * v.y)
    }

    pub fn determinant(&self) -> f32 {
        self.a * self.d - self.b * self.c
    }

    /// How much lengths grow on average: the square root of the area scale. Used to keep stroke widths
    /// and flattening tolerances right under a zoom.
    pub fn scale_factor(&self) -> f32 {
        self.determinant().abs().sqrt()
    }

    pub fn invert(&self) -> Option<Transform> {
        let det = self.determinant();
        if det.abs() < 1e-12 {
            return None;
        }
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Some(Transform { a, b, c, d, e: -(a * self.e + c * self.f), f: -(b * self.e + d * self.f) })
    }

    pub fn is_identity(&self) -> bool {
        *self == Transform::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4
    }

    #[test]
    fn then_applies_left_to_right_and_invert_undoes_it() {
        let t = Transform::scale(2.0, 3.0).then(Transform::translate(10.0, 20.0));
        assert!(near(t.apply(pt(1.0, 1.0)), pt(12.0, 23.0)));
        assert!(near(t.invert().unwrap().apply(t.apply(pt(5.0, -7.0))), pt(5.0, -7.0)));
        let r = Transform::rotate(std::f32::consts::FRAC_PI_2);
        assert!(near(r.apply(pt(1.0, 0.0)), pt(0.0, 1.0)), "x turns toward y");
        assert!(Transform::scale(0.0, 1.0).invert().is_none());
    }

    #[test]
    fn zooming_about_a_point_keeps_that_point_still() {
        let t = Transform::translate(30.0, 40.0).zoom_about(pt(100.0, 100.0), 2.0);
        let before = Transform::translate(30.0, 40.0);
        // The model point that was under (100,100) is still there after the zoom.
        let model = before.invert().unwrap().apply(pt(100.0, 100.0));
        assert!(near(t.apply(model), pt(100.0, 100.0)));
        assert!((t.scale_factor() - 2.0).abs() < 1e-5);
    }
}
