//! Geometry primitives.
//!
//! Every value is in logical pixels (points). The renderer multiplies by the window's scale factor, so
//! layout and painting never deal with device pixels except where a widget asks to `snap` an edge.

use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

/// A position.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// A displacement or delta. Subtracting two points gives a `Vec2`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

/// A width and height.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Size {
    pub w: f32,
    pub h: f32,
}

pub const fn pt(x: f32, y: f32) -> Point {
    Point { x, y }
}

pub const fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

pub const fn size(w: f32, h: f32) -> Size {
    Size { w, h }
}

impl Point {
    pub const ZERO: Point = pt(0.0, 0.0);

    pub fn distance(self, other: Point) -> f32 {
        (other - self).length()
    }

    pub fn lerp(self, other: Point, t: f32) -> Point {
        pt(self.x + (other.x - self.x) * t, self.y + (other.y - self.y) * t)
    }
}

impl Vec2 {
    pub const ZERO: Vec2 = vec2(0.0, 0.0);

    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }
}

impl Size {
    pub const ZERO: Size = size(0.0, 0.0);

    pub fn is_empty(self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }

    pub fn max(self, other: Size) -> Size {
        size(self.w.max(other.w), self.h.max(other.h))
    }

    pub fn min(self, other: Size) -> Size {
        size(self.w.min(other.w), self.h.min(other.h))
    }
}

impl Add<Vec2> for Point {
    type Output = Point;
    fn add(self, v: Vec2) -> Point {
        pt(self.x + v.x, self.y + v.y)
    }
}

impl Sub<Vec2> for Point {
    type Output = Point;
    fn sub(self, v: Vec2) -> Point {
        pt(self.x - v.x, self.y - v.y)
    }
}

impl Sub<Point> for Point {
    type Output = Vec2;
    fn sub(self, o: Point) -> Vec2 {
        vec2(self.x - o.x, self.y - o.y)
    }
}

impl AddAssign<Vec2> for Point {
    fn add_assign(&mut self, v: Vec2) {
        self.x += v.x;
        self.y += v.y;
    }
}

impl SubAssign<Vec2> for Point {
    fn sub_assign(&mut self, v: Vec2) {
        self.x -= v.x;
        self.y -= v.y;
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        vec2(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        vec2(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, k: f32) -> Vec2 {
        vec2(self.x * k, self.y * k)
    }
}

impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        vec2(-self.x, -self.y)
    }
}

/// An axis-aligned rectangle: top-left origin plus size. `y` grows downwards.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub const fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { x, y, w, h }
}

impl Rect {
    pub const ZERO: Rect = rect(0.0, 0.0, 0.0, 0.0);

    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        rect(x, y, w, h)
    }

    pub const fn from_origin_size(origin: Point, size: Size) -> Rect {
        rect(origin.x, origin.y, size.w, size.h)
    }

    pub fn from_min_max(min: Point, max: Point) -> Rect {
        rect(min.x, min.y, max.x - min.x, max.y - min.y)
    }

    pub const fn origin(self) -> Point {
        pt(self.x, self.y)
    }

    pub const fn size(self) -> Size {
        size(self.w, self.h)
    }

    pub fn right(self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(self) -> f32 {
        self.y + self.h
    }

    pub fn center(self) -> Point {
        pt(self.x + self.w * 0.5, self.y + self.h * 0.5)
    }

    pub fn is_empty(self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }

    /// Half-open containment: the left and top edges are inside, the right and bottom are not.
    pub fn contains(self, p: Point) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }

    pub fn intersects(self, o: Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }

    pub fn intersection(self, o: Rect) -> Option<Rect> {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        (x1 > x0 && y1 > y0).then(|| rect(x0, y0, x1 - x0, y1 - y0))
    }

    pub fn union(self, o: Rect) -> Rect {
        if self.is_empty() {
            return o;
        }
        if o.is_empty() {
            return self;
        }
        Rect::from_min_max(
            pt(self.x.min(o.x), self.y.min(o.y)),
            pt(self.right().max(o.right()), self.bottom().max(o.bottom())),
        )
    }

    pub fn inflate(self, dx: f32, dy: f32) -> Rect {
        rect(self.x - dx, self.y - dy, self.w + 2.0 * dx, self.h + 2.0 * dy)
    }

    /// Shrinks by `insets`; the size never goes negative.
    pub fn deflate(self, i: Insets) -> Rect {
        rect(
            self.x + i.left,
            self.y + i.top,
            (self.w - i.left - i.right).max(0.0),
            (self.h - i.top - i.bottom).max(0.0),
        )
    }

    pub fn translate(self, v: Vec2) -> Rect {
        rect(self.x + v.x, self.y + v.y, self.w, self.h)
    }

    pub fn with_origin(self, p: Point) -> Rect {
        rect(p.x, p.y, self.w, self.h)
    }

    pub fn with_size(self, s: Size) -> Rect {
        rect(self.x, self.y, s.w, s.h)
    }

    /// Rounds all four edges to whole device pixels at `scale`, so 1 px lines stay crisp.
    pub fn snap(self, scale: f32) -> Rect {
        let x0 = (self.x * scale).round() / scale;
        let y0 = (self.y * scale).round() / scale;
        let x1 = (self.right() * scale).round() / scale;
        let y1 = (self.bottom() * scale).round() / scale;
        rect(x0, y0, x1 - x0, y1 - y0)
    }

    pub fn clamp_point(self, p: Point) -> Point {
        pt(p.x.clamp(self.x, self.right()), p.y.clamp(self.y, self.bottom()))
    }
}

/// Space inside or around a rectangle.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Insets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Insets {
    pub const ZERO: Insets = Insets::all(0.0);

    pub const fn all(v: f32) -> Insets {
        Insets { top: v, right: v, bottom: v, left: v }
    }

    pub const fn symmetric(horizontal: f32, vertical: f32) -> Insets {
        Insets { top: vertical, right: horizontal, bottom: vertical, left: horizontal }
    }

    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Insets {
        Insets { top, right, bottom, left }
    }

    pub fn horizontal(self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}

/// Per-corner radii, clockwise from the top left.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Corners {
    pub tl: f32,
    pub tr: f32,
    pub br: f32,
    pub bl: f32,
}

impl Corners {
    pub const ZERO: Corners = Corners::all(0.0);

    pub const fn all(r: f32) -> Corners {
        Corners { tl: r, tr: r, br: r, bl: r }
    }

    pub const fn new(tl: f32, tr: f32, br: f32, bl: f32) -> Corners {
        Corners { tl, tr, br, bl }
    }

    /// Rounds only the top two corners.
    pub const fn top(r: f32) -> Corners {
        Corners { tl: r, tr: r, br: 0.0, bl: 0.0 }
    }

    /// Rounds only the bottom two corners.
    pub const fn bottom(r: f32) -> Corners {
        Corners { tl: 0.0, tr: 0.0, br: r, bl: r }
    }

    pub fn max(self) -> f32 {
        self.tl.max(self.tr).max(self.br).max(self.bl)
    }

    pub fn to_array(self) -> [f32; 4] {
        [self.tl, self.tr, self.br, self.bl]
    }
}

impl From<f32> for Corners {
    fn from(r: f32) -> Corners {
        Corners::all(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_arithmetic() {
        let a = pt(1.0, 2.0);
        let b = pt(4.0, 6.0);
        assert_eq!(b - a, vec2(3.0, 4.0));
        assert_eq!(a + vec2(1.0, 1.0), pt(2.0, 3.0));
        assert_eq!(a.distance(b), 5.0);
        assert_eq!(a.lerp(b, 0.5), pt(2.5, 4.0));
    }

    #[test]
    fn rect_contains_is_half_open() {
        let r = rect(10.0, 10.0, 20.0, 10.0);
        assert!(r.contains(pt(10.0, 10.0)));
        assert!(r.contains(pt(29.9, 19.9)));
        assert!(!r.contains(pt(30.0, 15.0)));
        assert!(!r.contains(pt(15.0, 20.0)));
        assert!(!r.contains(pt(9.9, 15.0)));
    }

    #[test]
    fn intersection_and_union() {
        let a = rect(0.0, 0.0, 10.0, 10.0);
        let b = rect(5.0, 5.0, 10.0, 10.0);
        assert_eq!(a.intersection(b), Some(rect(5.0, 5.0, 5.0, 5.0)));
        assert_eq!(a.union(b), rect(0.0, 0.0, 15.0, 15.0));
        // touching edges do not intersect
        assert_eq!(a.intersection(rect(10.0, 0.0, 5.0, 5.0)), None);
        assert!(!a.intersects(rect(10.0, 0.0, 5.0, 5.0)));
        // an empty rect is the identity of union
        assert_eq!(a.union(Rect::ZERO), a);
        assert_eq!(Rect::ZERO.union(a), a);
    }

    #[test]
    fn deflate_never_goes_negative() {
        let r = rect(0.0, 0.0, 10.0, 10.0);
        assert_eq!(r.deflate(Insets::all(2.0)), rect(2.0, 2.0, 6.0, 6.0));
        let squeezed = r.deflate(Insets::all(8.0));
        assert_eq!(squeezed.w, 0.0);
        assert_eq!(squeezed.h, 0.0);
        assert!(squeezed.is_empty());
    }

    #[test]
    fn snap_rounds_to_device_pixels() {
        let r = rect(10.3, 20.6, 30.2, 5.1);
        let s = r.snap(2.0);
        // at 2x the edges land on multiples of 0.5
        for edge in [s.x, s.y, s.right(), s.bottom()] {
            assert!(((edge * 2.0) - (edge * 2.0).round()).abs() < 1e-4, "{edge}");
        }
        // a 1 px line stays 1 px wide after snapping
        assert!((rect(10.2, 0.0, 1.0, 10.0).snap(1.0).w - 1.0).abs() < 1e-6);
    }

    #[test]
    fn corners_helpers() {
        assert_eq!(Corners::top(8.0), Corners::new(8.0, 8.0, 0.0, 0.0));
        assert_eq!(Corners::bottom(8.0).max(), 8.0);
        assert_eq!(Corners::from(4.0).to_array(), [4.0; 4]);
    }

    #[test]
    fn clamp_point_stays_inside() {
        let r = rect(0.0, 0.0, 10.0, 10.0);
        assert_eq!(r.clamp_point(pt(-5.0, 20.0)), pt(0.0, 10.0));
    }
}
