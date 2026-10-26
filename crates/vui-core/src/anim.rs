//! Motion: springs, easing and tweens.
//!
//! Springs are the default for anything that can be interrupted (hover, press, the notch growing into
//! a panel) because they keep their velocity when the target changes. Tweens are for fixed-length
//! fades. Both are stepped by the runtime with the frame's `dt`; nothing here reads a clock, so
//! animation is deterministic in tests.

use crate::color::Color;
use crate::geom::{Point, Rect, Size, pt, rect, size};

/// A value that can be interpolated.
pub trait Lerp: Copy {
    fn lerp(self, to: Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(self, to: f32, t: f32) -> f32 {
        self + (to - self) * t
    }
}

impl Lerp for Color {
    fn lerp(self, to: Color, t: f32) -> Color {
        Color::lerp(self, to, t)
    }
}

impl Lerp for Point {
    fn lerp(self, to: Point, t: f32) -> Point {
        pt(self.x.lerp(to.x, t), self.y.lerp(to.y, t))
    }
}

impl Lerp for Size {
    fn lerp(self, to: Size, t: f32) -> Size {
        size(self.w.lerp(to.w, t), self.h.lerp(to.h, t))
    }
}

impl Lerp for Rect {
    fn lerp(self, to: Rect, t: f32) -> Rect {
        rect(self.x.lerp(to.x, t), self.y.lerp(to.y, t), self.w.lerp(to.w, t), self.h.lerp(to.h, t))
    }
}

/// A damped spring: `mass * a = -stiffness * (x - target) - damping * v`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Spring {
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
}

impl Spring {
    /// Hover, press and selection: quick with a hint of give.
    pub const SNAPPY: Spring = Spring::new(420.0, 34.0, 1.0);
    /// Layout moves such as a list reordering.
    pub const SMOOTH: Spring = Spring::new(220.0, 28.0, 1.0);
    /// The notch growing into the ask panel: slower and softer.
    pub const MORPH: Spring = Spring::new(160.0, 22.0, 1.0);

    pub const fn new(stiffness: f32, damping: f32, mass: f32) -> Spring {
        Spring { stiffness, damping, mass }
    }

    /// The fastest spring that does not overshoot.
    pub fn critical(stiffness: f32) -> Spring {
        Spring::new(stiffness, 2.0 * stiffness.sqrt(), 1.0)
    }

    /// 1.0 is critically damped, below 1.0 overshoots, above 1.0 creeps.
    pub fn damping_ratio(&self) -> f32 {
        self.damping / (2.0 * (self.stiffness * self.mass).sqrt())
    }

    /// Advances `(x, v)` towards `target` by `dt` seconds. Sub-stepped so a long frame cannot make the
    /// integration unstable.
    pub fn step(&self, x: &mut f32, v: &mut f32, target: f32, dt: f32) {
        const MAX_STEP: f32 = 1.0 / 240.0;
        let mut left = dt.clamp(0.0, 0.25);
        while left > 0.0 {
            let h = left.min(MAX_STEP);
            let a = (-self.stiffness * (*x - target) - self.damping * *v) / self.mass;
            *v += a * h;
            *x += *v * h;
            left -= h;
        }
    }
}

/// An animated `f32` that chases a target with a spring.
#[derive(Copy, Clone, Debug)]
pub struct SpringValue {
    value: f32,
    velocity: f32,
    target: f32,
    pub spring: Spring,
    /// Within this distance of the target, and slower than `rest_velocity`, the value snaps and stops.
    pub rest_delta: f32,
    pub rest_velocity: f32,
}

impl SpringValue {
    pub fn new(value: f32, spring: Spring) -> SpringValue {
        SpringValue { value, velocity: 0.0, target: value, spring, rest_delta: 0.002, rest_velocity: 0.02 }
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn target(&self) -> f32 {
        self.target
    }

    pub fn velocity(&self) -> f32 {
        self.velocity
    }

    /// Starts moving to `target`, keeping the current velocity.
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Jumps without animating.
    pub fn snap_to(&mut self, value: f32) {
        self.value = value;
        self.target = value;
        self.velocity = 0.0;
    }

    /// Gives the value a kick, e.g. for a flick.
    pub fn set_velocity(&mut self, v: f32) {
        self.velocity = v;
    }

    pub fn is_settled(&self) -> bool {
        (self.value - self.target).abs() <= self.rest_delta && self.velocity.abs() <= self.rest_velocity
    }

    /// Steps by `dt` seconds. Returns true while it is still moving.
    pub fn step(&mut self, dt: f32) -> bool {
        if self.is_settled() {
            self.value = self.target;
            self.velocity = 0.0;
            return false;
        }
        self.spring.step(&mut self.value, &mut self.velocity, self.target, dt);
        if self.is_settled() {
            self.value = self.target;
            self.velocity = 0.0;
            return false;
        }
        true
    }
}

/// Timing curves for tweens.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Easing {
    Linear,
    EaseIn,
    /// Starts fast and settles: the default for fades and hovers.
    EaseOut,
    EaseInOut,
    /// A CSS-style cubic bezier through (0,0), (x1,y1), (x2,y2), (1,1).
    CubicBezier(f32, f32, f32, f32),
}

impl Easing {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t * t,
            Easing::EaseOut => 1.0 - (1.0 - t).powi(3),
            Easing::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Easing::CubicBezier(x1, y1, x2, y2) => cubic_bezier(x1, y1, x2, y2, t),
        }
    }
}

/// Solves x(s) = t for the bezier parameter by bisection, then returns y(s).
fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    let bez = |a: f32, b: f32, s: f32| {
        let u = 1.0 - s;
        3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
    };
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..32 {
        let mid = (lo + hi) * 0.5;
        if bez(x1, x2, mid) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bez(y1, y2, (lo + hi) * 0.5)
}

/// A fixed-duration interpolation between two values.
#[derive(Copy, Clone, Debug)]
pub struct Tween<T: Lerp> {
    from: T,
    to: T,
    duration: f32,
    elapsed: f32,
    pub easing: Easing,
}

impl<T: Lerp> Tween<T> {
    pub fn new(from: T, to: T, duration: f32, easing: Easing) -> Tween<T> {
        Tween { from, to, duration: duration.max(f32::EPSILON), elapsed: 0.0, easing }
    }

    /// A tween that is already finished at `value`.
    pub fn at_rest(value: T) -> Tween<T> {
        Tween { from: value, to: value, duration: f32::EPSILON, elapsed: f32::EPSILON, easing: Easing::Linear }
    }

    pub fn value(&self) -> T {
        self.from.lerp(self.to, self.easing.apply(self.elapsed / self.duration))
    }

    pub fn target(&self) -> T {
        self.to
    }

    /// Restarts from the current value towards `to`.
    pub fn retarget(&mut self, to: T, duration: f32) {
        self.from = self.value();
        self.to = to;
        self.duration = duration.max(f32::EPSILON);
        self.elapsed = 0.0;
    }

    pub fn is_finished(&self) -> bool {
        self.elapsed >= self.duration
    }

    /// Steps by `dt` seconds. Returns true while it is still running.
    pub fn step(&mut self, dt: f32) -> bool {
        self.elapsed = (self.elapsed + dt.max(0.0)).min(self.duration);
        !self.is_finished()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(v: &mut SpringValue, seconds: f32) -> Vec<f32> {
        let mut samples = vec![];
        let mut t = 0.0;
        while t < seconds {
            v.step(1.0 / 120.0);
            samples.push(v.value());
            t += 1.0 / 120.0;
        }
        samples
    }

    #[test]
    fn critical_spring_does_not_overshoot() {
        let mut v = SpringValue::new(0.0, Spring::critical(300.0));
        v.set_target(1.0);
        let samples = run(&mut v, 2.0);
        assert!(samples.iter().all(|s| *s <= 1.0 + 1e-3), "overshot: {:?}", samples.iter().cloned().fold(0.0, f32::max));
        assert!(v.is_settled());
        assert_eq!(v.value(), 1.0);
    }

    #[test]
    fn underdamped_spring_overshoots_then_settles() {
        let mut v = SpringValue::new(0.0, Spring::new(300.0, 8.0, 1.0));
        v.set_target(1.0);
        let samples = run(&mut v, 4.0);
        assert!(samples.iter().cloned().fold(0.0, f32::max) > 1.05);
        assert!(v.is_settled());
    }

    #[test]
    fn presets_settle_within_a_second() {
        for (name, s) in [("snappy", Spring::SNAPPY), ("smooth", Spring::SMOOTH), ("morph", Spring::MORPH)] {
            let mut v = SpringValue::new(0.0, s);
            v.set_target(100.0);
            let mut t = 0.0;
            while v.step(1.0 / 120.0) {
                t += 1.0 / 120.0;
                assert!(t < 1.5, "{name} still moving after {t}s");
            }
            assert_eq!(v.value(), 100.0);
        }
    }

    #[test]
    fn step_size_does_not_change_the_outcome_much() {
        let mut a = SpringValue::new(0.0, Spring::SNAPPY);
        let mut b = SpringValue::new(0.0, Spring::SNAPPY);
        a.set_target(1.0);
        b.set_target(1.0);
        for _ in 0..30 {
            a.step(1.0 / 60.0);
        }
        for _ in 0..60 {
            b.step(1.0 / 120.0);
        }
        assert!((a.value() - b.value()).abs() < 0.01, "{} vs {}", a.value(), b.value());
    }

    #[test]
    fn retargeting_keeps_velocity() {
        let mut v = SpringValue::new(0.0, Spring::SMOOTH);
        v.set_target(1.0);
        for _ in 0..10 {
            v.step(1.0 / 60.0);
        }
        let speed = v.velocity();
        assert!(speed > 0.0);
        v.set_target(0.0);
        assert_eq!(v.velocity(), speed, "interrupting must not reset velocity");
    }

    #[test]
    fn settled_spring_reports_no_motion() {
        let mut v = SpringValue::new(5.0, Spring::SNAPPY);
        assert!(!v.step(1.0 / 60.0));
        v.snap_to(7.0);
        assert!(v.is_settled());
    }

    #[test]
    fn easing_endpoints_and_monotonic() {
        for e in [Easing::Linear, Easing::EaseIn, Easing::EaseOut, Easing::EaseInOut, Easing::CubicBezier(0.25, 0.1, 0.25, 1.0)] {
            assert!(e.apply(0.0).abs() < 1e-3, "{e:?}");
            assert!((e.apply(1.0) - 1.0).abs() < 1e-3, "{e:?}");
            let mut last = 0.0;
            for i in 0..=100 {
                let y = e.apply(i as f32 / 100.0);
                assert!(y + 1e-4 >= last, "{e:?} not monotonic at {i}");
                last = y;
            }
        }
        // ease-out is ahead of linear at the midpoint
        assert!(Easing::EaseOut.apply(0.5) > 0.5);
    }

    #[test]
    fn tween_runs_to_completion() {
        let mut t = Tween::new(0.0f32, 10.0, 0.15, Easing::Linear);
        assert_eq!(t.value(), 0.0);
        assert!(t.step(0.05));
        assert!((t.value() - 10.0 / 3.0).abs() < 1e-3);
        assert!(t.step(0.05));
        assert!(!t.step(0.05));
        assert_eq!(t.value(), 10.0);
        assert!(t.is_finished());
    }

    #[test]
    fn tween_retarget_starts_from_current_value() {
        let mut t = Tween::new(0.0f32, 10.0, 1.0, Easing::Linear);
        t.step(0.5);
        t.retarget(0.0, 1.0);
        assert!((t.value() - 5.0).abs() < 1e-3);
        t.step(1.0);
        assert_eq!(t.value(), 0.0);
    }

    #[test]
    fn colours_and_rects_lerp() {
        let c = Color::BLACK.lerp(Color::WHITE, 0.5);
        assert!((c.g - 0.5).abs() < 1e-6);
        let r = rect(0.0, 0.0, 10.0, 10.0).lerp(rect(10.0, 10.0, 20.0, 20.0), 0.5);
        assert_eq!(r, rect(5.0, 5.0, 15.0, 15.0));
    }
}
