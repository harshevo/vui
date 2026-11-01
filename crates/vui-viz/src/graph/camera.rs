//! Where a graph is looked at from: which world point is in the middle of the view, and how many
//! pixels a world unit takes.

use vui_core::{Point, Rect, pt};

pub const MIN_ZOOM: f32 = 0.04;
pub const MAX_ZOOM: f32 = 10.0;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Camera {
    pub center: Point,
    pub zoom: f32,
}

impl Camera {
    pub const HOME: Camera = Camera {
        center: Point { x: 0.0, y: 0.0 },
        zoom: 1.0,
    };

    /// Where world point `p` falls on screen, with `mid` the middle of the view.
    pub fn to_screen(&self, p: Point, mid: Point) -> Point {
        pt(
            mid.x + (p.x - self.center.x) * self.zoom,
            mid.y + (p.y - self.center.y) * self.zoom,
        )
    }

    pub fn to_world(&self, s: Point, mid: Point) -> Point {
        pt(
            self.center.x + (s.x - mid.x) / self.zoom,
            self.center.y + (s.y - mid.y) / self.zoom,
        )
    }

    /// Zooms by `factor` keeping the world point under `s` where it is.
    pub fn zoom_about(&mut self, s: Point, mid: Point, factor: f32) {
        let anchor = self.to_world(s, mid);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.center = pt(
            anchor.x - (s.x - mid.x) / self.zoom,
            anchor.y - (s.y - mid.y) / self.zoom,
        );
    }

    /// Moves the view by `dx, dy` screen pixels, as if the picture were dragged by that much.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.center = pt(
            self.center.x - dx / self.zoom,
            self.center.y - dy / self.zoom,
        );
    }

    /// The view that shows all of `points` inside `area` with `margin` to spare. Never zooms in past 1.5, so a
    /// graph of two nodes is not blown up to fill the screen.
    pub fn fit(points: impl Iterator<Item = Point>, area: Rect, margin: f32) -> Camera {
        let (mut x0, mut y0, mut x1, mut y1) = (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        );
        for p in points {
            (x0, y0, x1, y1) = (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y));
        }
        if x0 > x1 {
            return Camera::HOME;
        }
        let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
        let zoom = ((area.w - margin * 2.0).max(1.0) / w)
            .min((area.h - margin * 2.0).max(1.0) / h)
            .clamp(MIN_ZOOM, 1.5);
        Camera {
            center: pt((x0 + x1) / 2.0, (y0 + y1) / 2.0),
            zoom,
        }
    }

    /// A step of `k` (0 to 1) from this view toward `to`. Zoom moves on a log scale so zooming in and out
    /// feel the same speed.
    pub fn toward(&self, to: Camera, k: f32) -> Camera {
        let zoom = (self.zoom.ln() + (to.zoom.ln() - self.zoom.ln()) * k).exp();
        Camera {
            center: pt(
                self.center.x + (to.center.x - self.center.x) * k,
                self.center.y + (to.center.y - self.center.y) * k,
            ),
            zoom,
        }
    }

    /// Whether this is close enough to `to` that nobody could tell.
    pub fn is_near(&self, to: Camera) -> bool {
        (self.zoom / to.zoom - 1.0).abs() < 0.002
            && (self.center.x - to.center.x).abs() * self.zoom < 0.2
            && (self.center.y - to.center.y).abs() * self.zoom < 0.2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vui_core::rect;

    #[test]
    fn world_and_screen_round_trip() {
        let c = Camera {
            center: pt(30.0, -10.0),
            zoom: 2.5,
        };
        let mid = pt(400.0, 300.0);
        let p = pt(12.0, 77.0);
        let back = c.to_world(c.to_screen(p, mid), mid);
        assert!((back.x - p.x).abs() < 1e-4 && (back.y - p.y).abs() < 1e-4);
        assert_eq!(c.to_screen(c.center, mid), mid);
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer_still() {
        let mut c = Camera {
            center: pt(5.0, 5.0),
            zoom: 1.0,
        };
        let mid = pt(200.0, 150.0);
        let s = pt(320.0, 90.0);
        let world = c.to_world(s, mid);
        c.zoom_about(s, mid, 1.7);
        let after = c.to_screen(world, mid);
        assert!((after.x - s.x).abs() < 1e-3 && (after.y - s.y).abs() < 1e-3);
        assert!((c.zoom - 1.7).abs() < 1e-6);
        c.zoom_about(s, mid, 1e9);
        assert_eq!(c.zoom, MAX_ZOOM);
        c.zoom_about(s, mid, 1e-12);
        assert_eq!(c.zoom, MIN_ZOOM);
    }

    #[test]
    fn panning_moves_the_picture_with_the_pointer() {
        let mut c = Camera {
            center: pt(0.0, 0.0),
            zoom: 2.0,
        };
        let mid = pt(100.0, 100.0);
        let before = c.to_screen(pt(10.0, 10.0), mid);
        c.pan(30.0, -8.0);
        let after = c.to_screen(pt(10.0, 10.0), mid);
        assert_eq!((after.x - before.x, after.y - before.y), (30.0, -8.0));
    }

    #[test]
    fn fit_shows_every_point_with_a_margin_and_is_not_blown_up() {
        let area = rect(0.0, 0.0, 600.0, 400.0);
        let pts = [pt(-300.0, -100.0), pt(500.0, 150.0)];
        let c = Camera::fit(pts.iter().copied(), area, 24.0);
        let mid = area.center();
        for p in pts {
            let s = c.to_screen(p, mid);
            assert!(
                s.x >= 23.9 && s.x <= 576.1 && s.y >= 23.9 && s.y <= 376.1,
                "{s:?}"
            );
        }
        let tiny = Camera::fit([pt(0.0, 0.0), pt(10.0, 10.0)].into_iter(), area, 24.0);
        assert_eq!(tiny.zoom, 1.5);
        assert_eq!(Camera::fit(std::iter::empty(), area, 24.0), Camera::HOME);
    }

    #[test]
    fn easing_closes_the_gap_and_zoom_is_even_in_both_directions() {
        let a = Camera {
            center: pt(0.0, 0.0),
            zoom: 0.5,
        };
        let b = Camera {
            center: pt(100.0, 0.0),
            zoom: 2.0,
        };
        let half = a.toward(b, 0.5);
        assert!(
            (half.zoom - 1.0).abs() < 1e-5,
            "halfway between 0.5 and 2 on a log scale is 1"
        );
        assert_eq!(half.center.x, 50.0);
        let mut c = a;
        for _ in 0..200 {
            c = c.toward(b, 0.2);
        }
        assert!(c.is_near(b));
        assert!(!a.is_near(b));
    }
}
