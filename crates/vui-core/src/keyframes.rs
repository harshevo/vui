//! Keyframes: a value that runs through a list of stops over a fixed time, once, a few times or
//! for as long as it is shown, like a CSS `@keyframes` animation.

use crate::anim::{Easing, Lerp};

/// How many times keyframes run.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Repeat {
    Times(u32),
    Forever,
}

/// Stops for a value over `duration` seconds. Each stop is at a fraction of the run (0 to 1) with
/// the easing used to reach it from the one before.
///
/// ```
/// use vui_core::{Easing, Keyframes, Repeat};
///
/// // A pulse: up to 1.2 halfway, back by the end, for as long as it is shown.
/// let pulse = Keyframes::new(1.2, 1.0).to(0.5, 1.2, Easing::EaseOut).to(1.0, 1.0, Easing::EaseIn).repeat(Repeat::Forever);
/// assert_eq!(pulse.sample(0.6), 1.2);
/// assert_eq!(pulse.sample(1.2 + 0.6), 1.2, "the second run");
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframes<T: Lerp> {
    duration: f32,
    stops: Vec<(f32, T, Easing)>,
    repeat: Repeat,
    alternate: bool,
}

impl<T: Lerp> Keyframes<T> {
    /// Keyframes `duration` seconds long that start at `from`, run once, and hold the last stop.
    pub fn new(duration: f32, from: T) -> Keyframes<T> {
        Keyframes { duration: duration.max(f32::EPSILON), stops: vec![(0.0, from, Easing::Linear)], repeat: Repeat::Times(1), alternate: false }
    }

    /// Adds a stop at fraction `at` of the run, reached with `easing`. Stops must come in order.
    pub fn to(mut self, at: f32, value: T, easing: Easing) -> Keyframes<T> {
        let last = self.stops.last().map_or(0.0, |s| s.0);
        self.stops.push((at.clamp(last, 1.0), value, easing));
        self
    }

    pub fn repeat(mut self, repeat: Repeat) -> Keyframes<T> {
        self.repeat = repeat;
        self
    }

    /// Runs every other time backwards, so a loop sways instead of jumping back to the start.
    pub fn alternate(mut self) -> Keyframes<T> {
        self.alternate = true;
        self
    }

    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// Whether every run is over `elapsed` seconds in.
    pub fn finished(&self, elapsed: f32) -> bool {
        match self.repeat {
            Repeat::Forever => false,
            Repeat::Times(n) => elapsed >= self.duration * n as f32,
        }
    }

    /// The first stop's value: what reduced motion shows.
    pub fn first(&self) -> T {
        self.stops[0].1
    }

    /// The value `elapsed` seconds in.
    pub fn sample(&self, elapsed: f32) -> T {
        let runs = match self.repeat {
            Repeat::Forever => f32::INFINITY,
            Repeat::Times(n) => n.max(1) as f32,
        };
        let at = (elapsed.max(0.0) / self.duration).min(runs);
        let (run, mut t) = if at >= runs { (runs - 1.0, 1.0) } else { (at.floor(), at.fract()) };
        if self.alternate && run % 2.0 == 1.0 {
            t = 1.0 - t;
        }
        let mut prev = &self.stops[0];
        for stop in &self.stops[1..] {
            if t <= stop.0 {
                let span = stop.0 - prev.0;
                let local = if span > 0.0 { (t - prev.0) / span } else { 1.0 };
                return prev.1.lerp(stop.1, stop.2.apply(local));
            }
            prev = stop;
        }
        prev.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_are_reached_in_order_and_held() {
        let k = Keyframes::new(2.0, 0.0).to(0.5, 10.0, Easing::Linear).to(1.0, 0.0, Easing::Linear);
        assert_eq!(k.sample(0.5), 5.0);
        assert_eq!(k.sample(1.0), 10.0);
        assert_eq!(k.sample(1.5), 5.0);
        assert_eq!(k.sample(9.0), 0.0, "held at the end");
        assert!(k.finished(2.0) && !k.finished(1.9));
    }

    #[test]
    fn repeats_and_alternates() {
        let k = Keyframes::new(1.0, 0.0).to(1.0, 1.0, Easing::Linear).repeat(Repeat::Times(3)).alternate();
        assert!((k.sample(0.25) - 0.25).abs() < 1e-6);
        assert!((k.sample(1.25) - 0.75).abs() < 1e-6, "the second run goes back");
        assert!((k.sample(2.25) - 0.25).abs() < 1e-6);
        assert_eq!(k.sample(10.0), 1.0, "three runs, the last forwards, end at 1");
        let forever = Keyframes::new(1.0, 0.0).to(1.0, 1.0, Easing::Linear).repeat(Repeat::Forever);
        assert!(!forever.finished(1e6));
        assert!((forever.sample(100.5) - 0.5).abs() < 1e-3);
    }
}
