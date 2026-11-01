//! Scales map data to positions and back, and choose the round numbers a person expects on an axis.

use crate::format::{MONTHS, civil_from_days, days_from_civil};

/// A tick: where it falls in the data and how it is written.
#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    pub value: f64,
    pub label: String,
}

/// A straight-line mapping of a data range onto a pixel range.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Linear {
    pub domain: (f64, f64),
    pub range: (f32, f32),
}

impl Linear {
    pub fn new(domain: (f64, f64), range: (f32, f32)) -> Linear {
        Linear { domain, range }
    }

    pub fn map(&self, v: f64) -> f32 {
        let span = self.domain.1 - self.domain.0;
        let t = if span.abs() < f64::EPSILON {
            0.5
        } else {
            (v - self.domain.0) / span
        };
        self.range.0 + (self.range.1 - self.range.0) * t as f32
    }

    pub fn invert(&self, px: f32) -> f64 {
        let span = self.range.1 - self.range.0;
        let t = if span.abs() < f32::EPSILON {
            0.5
        } else {
            (px - self.range.0) / span
        };
        self.domain.0 + (self.domain.1 - self.domain.0) * f64::from(t)
    }

    /// Round values inside the domain, about `count` of them.
    pub fn ticks(&self, count: usize) -> Vec<f64> {
        nice_ticks(
            self.domain.0.min(self.domain.1),
            self.domain.0.max(self.domain.1),
            count,
        )
    }

    /// Widens the domain to round ends so the axis starts and stops on a tick.
    pub fn nice(mut self, count: usize) -> Linear {
        let (lo, hi) = (
            self.domain.0.min(self.domain.1),
            self.domain.0.max(self.domain.1),
        );
        let step = tick_step(lo, hi, count);
        if step > 0.0 && step.is_finite() {
            let (a, b) = ((lo / step).floor() * step, (hi / step).ceil() * step);
            self.domain = if self.domain.0 <= self.domain.1 {
                (a, b)
            } else {
                (b, a)
            };
        }
        self
    }
}

/// The tick step d3 would choose: 1, 2 or 5 times a power of ten.
pub fn tick_step(lo: f64, hi: f64, count: usize) -> f64 {
    let raw = (hi - lo) / count.max(1) as f64;
    if !raw.is_finite() || raw <= 0.0 {
        return 0.0;
    }
    let power = raw.log10().floor();
    let error = raw / 10f64.powf(power);
    let factor = if error >= 7.071 {
        10.0
    } else if error >= 3.162 {
        5.0
    } else if error >= 1.414 {
        2.0
    } else {
        1.0
    };
    factor * 10f64.powf(power)
}

pub fn nice_ticks(lo: f64, hi: f64, count: usize) -> Vec<f64> {
    let step = tick_step(lo, hi, count);
    if step <= 0.0 {
        return if lo.is_finite() { vec![lo] } else { Vec::new() };
    }
    let first = (lo / step).ceil() as i64;
    let last = (hi / step).floor() as i64;
    // Multiply rather than add so 0.1 + 0.2 style drift never shows in a label.
    (first..=last)
        .map(|i| clean(i as f64 * step, step))
        .collect()
}

fn clean(v: f64, step: f64) -> f64 {
    let digits = (-step.log10().floor()).max(0.0) as i32 + 1;
    let k = 10f64.powi(digits);
    (v * k).round() / k
}

/// A logarithmic mapping: equal ratios take equal space. The domain must be positive.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Log {
    pub domain: (f64, f64),
    pub range: (f32, f32),
}

impl Log {
    pub fn new(domain: (f64, f64), range: (f32, f32)) -> Log {
        Log {
            domain: (domain.0.max(1e-12), domain.1.max(1e-12)),
            range,
        }
    }

    pub fn map(&self, v: f64) -> f32 {
        let (a, b) = (self.domain.0.ln(), self.domain.1.ln());
        let t = if (b - a).abs() < f64::EPSILON {
            0.5
        } else {
            (v.max(1e-12).ln() - a) / (b - a)
        };
        self.range.0 + (self.range.1 - self.range.0) * t as f32
    }

    pub fn invert(&self, px: f32) -> f64 {
        let (a, b) = (self.domain.0.ln(), self.domain.1.ln());
        let span = self.range.1 - self.range.0;
        let t = if span.abs() < f32::EPSILON {
            0.5
        } else {
            f64::from((px - self.range.0) / span)
        };
        (a + (b - a) * t).exp()
    }

    /// Powers of ten, with 2 and 5 between them when the domain spans only a few decades.
    pub fn ticks(&self) -> Vec<f64> {
        let (lo, hi) = (
            self.domain.0.min(self.domain.1),
            self.domain.0.max(self.domain.1),
        );
        let decades = (hi / lo).log10();
        let mut out = Vec::new();
        let mut p = lo.log10().floor() as i32;
        while 10f64.powi(p) <= hi * 1.0001 {
            let base = 10f64.powi(p);
            let mults: &[f64] = if decades < 2.5 {
                &[1.0, 2.0, 5.0]
            } else {
                &[1.0]
            };
            for m in mults {
                let v = base * m;
                if v >= lo * 0.9999 && v <= hi * 1.0001 {
                    out.push(v);
                }
            }
            p += 1;
        }
        out
    }
}

/// Categories laid along an axis, each with a band of space and padding between.
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    pub count: usize,
    pub range: (f32, f32),
    /// Share of each step left empty between bands, 0 to 1.
    pub padding: f32,
}

impl Band {
    pub fn new(count: usize, range: (f32, f32), padding: f32) -> Band {
        Band {
            count,
            range,
            padding: padding.clamp(0.0, 0.95),
        }
    }

    pub fn step(&self) -> f32 {
        (self.range.1 - self.range.0) / self.count.max(1) as f32
    }

    pub fn width(&self) -> f32 {
        self.step().abs() * (1.0 - self.padding)
    }

    /// Where band `i` starts (its lower edge).
    pub fn start(&self, i: usize) -> f32 {
        let step = self.step();
        self.range.0 + step * i as f32 + (step - step.signum() * self.width()) / 2.0
    }

    pub fn center(&self, i: usize) -> f32 {
        self.range.0 + self.step() * (i as f32 + 0.5)
    }

    /// The band under a position, if any.
    pub fn index_at(&self, px: f32) -> Option<usize> {
        let i = ((px - self.range.0) / self.step()).floor();
        (i >= 0.0 && (i as usize) < self.count).then_some(i as usize)
    }
}

// --- time ------------------------------------------------------------------------------------------------

const MIN: f64 = 60.0;
const HOUR: f64 = 3600.0;
const DAY: f64 = 86_400.0;

/// Fixed steps in seconds, then the calendar steps in months.
const STEPS: [f64; 17] = [
    1.0,
    5.0,
    15.0,
    30.0,
    MIN,
    5.0 * MIN,
    15.0 * MIN,
    30.0 * MIN,
    HOUR,
    3.0 * HOUR,
    6.0 * HOUR,
    12.0 * HOUR,
    DAY,
    2.0 * DAY,
    7.0 * DAY,
    14.0 * DAY,
    0.0,
];
const MONTH_STEPS: [i64; 6] = [1, 3, 6, 12, 24, 60];

/// Calendar-aligned ticks over a UTC time span in seconds, labelled for the span's size.
pub fn time_ticks(lo: f64, hi: f64, count: usize) -> Vec<Tick> {
    if !lo.is_finite() || !hi.is_finite() || hi <= lo {
        return Vec::new();
    }
    let target = (hi - lo) / count.max(1) as f64;
    // Short spans use fixed steps.
    if let Some(step) = STEPS
        .iter()
        .copied()
        .filter(|s| *s > 0.0)
        .find(|s| *s >= target * 0.7)
    {
        let first = (lo / step).ceil() as i64;
        let last = (hi / step).floor() as i64;
        return (first..=last)
            .map(|i| {
                let v = i as f64 * step;
                Tick {
                    value: v,
                    label: short_label(v as i64, step),
                }
            })
            .collect();
    }
    // Longer spans step by whole months.
    let months_target = target / (30.44 * DAY);
    let step_months = MONTH_STEPS
        .iter()
        .copied()
        .find(|m| *m as f64 >= months_target * 0.7)
        .unwrap_or_else(|| ((months_target / 12.0).ceil() as i64).max(1) * 12);
    let (y0, m0, _) = civil_from_days((lo / DAY).floor() as i64);
    let mut index = y0 * 12 + (m0 - 1);
    index = index.div_euclid(step_months) * step_months;
    let mut out = Vec::new();
    for _ in 0..400 {
        let (y, m) = (index.div_euclid(12), index.rem_euclid(12) + 1);
        let v = (days_from_civil(y, m, 1) * 86_400) as f64;
        if v > hi {
            break;
        }
        if v >= lo {
            let label = if step_months >= 12 || m == 1 {
                y.to_string()
            } else {
                MONTHS[(m - 1) as usize].to_string()
            };
            out.push(Tick { value: v, label });
        }
        index += step_months;
    }
    out
}

fn short_label(ts: i64, step: f64) -> String {
    let (y, m, d) = civil_from_days(ts.div_euclid(86_400));
    let secs = ts.rem_euclid(86_400);
    if step >= DAY {
        if d == 1 {
            MONTHS[(m - 1) as usize].to_string()
                + &if m == 1 {
                    format!(" {y}")
                } else {
                    String::new()
                }
        } else {
            format!("{d} {}", MONTHS[(m - 1) as usize])
        }
    } else if secs == 0 {
        format!("{d} {}", MONTHS[(m - 1) as usize])
    } else if step >= MIN {
        format!("{:02}:{:02}", secs / 3600, secs % 3600 / 60)
    } else {
        format!(
            "{:02}:{:02}:{:02}",
            secs / 3600,
            secs % 3600 / 60,
            secs % 60
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::timestamp;

    #[test]
    fn linear_maps_and_inverts() {
        let s = Linear::new((0.0, 100.0), (10.0, 210.0));
        assert_eq!(s.map(50.0), 110.0);
        assert!((s.invert(110.0) - 50.0).abs() < 1e-9);
        let flipped = Linear::new((0.0, 1.0), (100.0, 0.0));
        assert_eq!(flipped.map(0.25), 75.0, "a y axis runs upward on screen");
        assert_eq!(
            Linear::new((5.0, 5.0), (0.0, 10.0)).map(5.0),
            5.0,
            "a flat domain sits in the middle"
        );
    }

    #[test]
    fn ticks_are_round_numbers_without_float_noise() {
        assert_eq!(nice_ticks(0.0, 1.0, 5), vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0]);
        assert_eq!(nice_ticks(0.0, 97.0, 5), vec![0.0, 20.0, 40.0, 60.0, 80.0]);
        assert_eq!(
            nice_ticks(-3.0, 3.0, 6),
            vec![-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0]
        );
        assert_eq!(
            nice_ticks(1_000.0, 5_000.0, 4),
            vec![1000.0, 2000.0, 3000.0, 4000.0, 5000.0]
        );
        assert_eq!(nice_ticks(2.0, 2.0, 5), vec![2.0]);
        let tiny = nice_ticks(0.0, 0.0003, 3);
        assert!(
            tiny.iter().all(|t| (t * 1e4).round() / 1e4 == *t),
            "{tiny:?}"
        );
    }

    #[test]
    fn nice_widens_to_ticks() {
        let s = Linear::new((3.2, 97.0), (0.0, 100.0)).nice(5);
        assert_eq!(s.domain, (0.0, 100.0));
    }

    #[test]
    fn log_scales_space_ratios_evenly() {
        let s = Log::new((1.0, 1000.0), (0.0, 300.0));
        assert!((s.map(10.0) - 100.0).abs() < 1e-3 && (s.map(100.0) - 200.0).abs() < 1e-3);
        assert!((s.invert(200.0) / 100.0 - 1.0).abs() < 1e-4);
        assert_eq!(s.ticks(), vec![1.0, 10.0, 100.0, 1000.0]);
        assert_eq!(
            Log::new((1.0, 20.0), (0.0, 1.0)).ticks(),
            vec![1.0, 2.0, 5.0, 10.0, 20.0]
        );
    }

    #[test]
    fn bands_centre_their_categories() {
        let b = Band::new(4, (0.0, 400.0), 0.2);
        assert_eq!(b.step(), 100.0);
        assert_eq!(b.width(), 80.0);
        assert_eq!(b.start(1), 110.0);
        assert_eq!(b.center(1), 150.0);
        assert_eq!(b.index_at(399.0), Some(3));
        assert_eq!(b.index_at(-1.0), None);
    }

    #[test]
    fn time_ticks_follow_the_calendar() {
        let year = time_ticks(
            timestamp(2023, 1, 1, 0, 0, 0) as f64,
            timestamp(2024, 1, 1, 0, 0, 0) as f64,
            6,
        );
        let labels: Vec<&str> = year.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(
            labels,
            ["2023", "Apr", "Jul", "Oct", "2024"],
            "quarters, with the year where it turns"
        );
        assert!(year.windows(2).all(|w| w[0].value < w[1].value));
        let decade = time_ticks(
            timestamp(2010, 1, 1, 0, 0, 0) as f64,
            timestamp(2024, 1, 1, 0, 0, 0) as f64,
            7,
        );
        assert!(
            decade.iter().all(|t| t.label.len() == 4),
            "years only: {decade:?}"
        );
        let day = time_ticks(
            timestamp(2024, 3, 12, 0, 0, 0) as f64,
            timestamp(2024, 3, 13, 0, 0, 0) as f64,
            6,
        );
        assert_eq!(day[0].label, "12 Mar");
        assert!(day[1].label.contains(':'), "{:?}", day[1]);
        assert!(time_ticks(5.0, 5.0, 4).is_empty());
    }
}
