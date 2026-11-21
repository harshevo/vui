//! A histogram: how many things fall in each slice of a range, drawn as bars with the chosen part
//! of the range lit and the rest dimmed. It sits over a range slider, so the slider's thumbs choose
//! from a picture of the data rather than from bare numbers: the bars are inset by half a thumb on
//! each side, which lines a bar up with the slider position of the values it counts.
//!
//! [`Histogram::of`] bins a list of numbers, leaving out the missing ones and folding the extreme
//! few into the end bins so one outlier cannot flatten the rest.

mod widget;

#[cfg(test)]
mod tests;

pub use widget::{HistogramView, histogram};

use vui_core::{Rect, rect};

/// Half a slider thumb: how far in from each side the bars start.
pub const INSET: f32 = 8.0;
pub const HEIGHT: f32 = 56.0;
const GAP: f32 = 2.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Histogram {
    /// How many fell in each bin, left to right.
    pub counts: Vec<u32>,
    pub min: f64,
    pub max: f64,
    /// The lit part of the range.
    pub low: f64,
    pub high: f64,
    /// A line at a value: where the stock being looked at stands among the rest.
    pub marker: Option<f64>,
}

impl Histogram {
    /// Bins `values` into `bins` slices between `min` and `max`. A value outside goes in the nearest
    /// end bin; one that is not a number is skipped. The whole range is lit.
    pub fn new(values: &[f64], bins: usize, min: f64, max: f64) -> Histogram {
        let bins = bins.clamp(1, 200);
        let (min, max) = if min.is_finite() && max.is_finite() && max > min { (min, max) } else { (0.0, 1.0) };
        let mut counts = vec![0u32; bins];
        for v in values.iter().filter(|v| v.is_finite()) {
            let at = ((v - min) / (max - min) * bins as f64).floor();
            counts[at.clamp(0.0, bins as f64 - 1.0) as usize] += 1;
        }
        Histogram { counts, min, max, low: min, high: max, marker: None }
    }

    /// Bins `values` over the range between their 2nd and 98th percentile, so a handful of absurd
    /// P/Es cannot squash everyone else into one bar. With nothing to bin, a unit range.
    pub fn of(values: &[f64], bins: usize) -> Histogram {
        let mut v: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
        if v.is_empty() {
            return Histogram::new(&[], bins, 0.0, 1.0);
        }
        v.sort_by(f64::total_cmp);
        let at = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
        let (mut lo, mut hi) = (at(0.02), at(0.98));
        if hi <= lo {
            (lo, hi) = (v[0], v[v.len() - 1]);
        }
        if hi <= lo {
            (lo, hi) = (lo - 0.5, hi + 0.5);
        }
        Histogram::new(&v, bins, lo, hi)
    }

    /// The part to light.
    pub fn lit(mut self, low: f64, high: f64) -> Histogram {
        self.low = low.min(high);
        self.high = high.max(low);
        self
    }

    pub fn marker(mut self, value: Option<f64>) -> Histogram {
        self.marker = value.filter(|v| v.is_finite());
        self
    }

    pub fn total(&self) -> u32 {
        self.counts.iter().sum()
    }

    /// How many fall in bins whose middle is inside the lit part.
    pub fn lit_count(&self) -> u32 {
        (0..self.counts.len()).filter(|i| self.is_lit(*i)).map(|i| self.counts[i]).sum()
    }

    /// The range bin `i` covers.
    pub fn bin_range(&self, i: usize) -> (f64, f64) {
        let w = (self.max - self.min) / self.counts.len() as f64;
        (self.min + w * i as f64, self.min + w * (i + 1) as f64)
    }

    fn is_lit(&self, i: usize) -> bool {
        let (a, b) = self.bin_range(i);
        let mid = (a + b) / 2.0;
        mid >= self.low && mid <= self.high
    }

    /// Where along `area` the value `v` is, 0 to 1.
    pub fn fraction(&self, v: f64) -> f32 {
        (((v - self.min) / (self.max - self.min)) as f32).clamp(0.0, 1.0)
    }
}

/// The room the bars are drawn in: `area` less the inset at each side.
pub fn plot(area: Rect) -> Rect {
    rect(area.x + INSET, area.y, (area.w - INSET * 2.0).max(0.0), area.h)
}

/// Bar `i`'s rectangle: as wide as its slice less a gap, standing on the bottom of `area`, as tall as
/// its share of the biggest count (a bin that is not empty is at least 2 points tall).
pub fn bar(h: &Histogram, area: Rect, i: usize) -> Rect {
    let p = plot(area);
    let n = h.counts.len() as f32;
    let top = h.counts.iter().copied().max().unwrap_or(0).max(1) as f32;
    let w = p.w / n;
    let height = if h.counts[i] == 0 { 0.0 } else { (area.h * h.counts[i] as f32 / top).max(2.0).min(area.h) };
    rect(p.x + w * i as f32 + GAP / 2.0, area.bottom() - height, (w - GAP).max(1.0), height)
}

/// The bin under horizontal position `x`, if it is over the bars at all.
pub fn bin_at(h: &Histogram, area: Rect, x: f32) -> Option<usize> {
    let p = plot(area);
    if x < p.x || x > p.right() || p.w <= 0.0 {
        return None;
    }
    Some((((x - p.x) / p.w * h.counts.len() as f32) as usize).min(h.counts.len() - 1))
}
