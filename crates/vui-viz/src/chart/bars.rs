//! Bars that share an x: side by side in groups, or stacked on each other, and how they are drawn.

use std::collections::HashMap;

use vui_core::{Color, Corners, Quad, rect};

use super::decimate::extremes;
use super::{Chart, Mark, Panel};
use crate::canvas::Canvas;
use crate::scale::Linear;

/// How the bar layers of the main panel share each x.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum BarLayout {
    /// Each layer draws at the full width, later ones over earlier ones.
    #[default]
    Overlap,
    /// Side by side, one slot per layer.
    Grouped,
    /// On top of each other: positive values grow up from zero, negative ones down.
    Stacked,
}

/// Values at x = 0, 1, 2…, for charts whose x is a list of [categories](Chart::categories).
pub fn indexed(values: &[f64]) -> Vec<(f64, f64)> {
    values.iter().enumerate().map(|(i, v)| (i as f64, *v)).collect()
}

/// Where one stacked bar sits: from `base` to `top`, and whether it is the outermost on its side.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Segment {
    pub base: f64,
    pub top: f64,
    pub outer: bool,
}

/// Which slot of how many a layer's bars take, and the segments when they are stacked.
pub(crate) struct Place<'a> {
    pub slot: usize,
    pub of: usize,
    pub stack: Option<&'a [Segment]>,
}

impl Chart {
    pub fn grouped(mut self) -> Chart {
        self.bar_layout = BarLayout::Grouped;
        self
    }

    pub fn stacked(mut self) -> Chart {
        self.bar_layout = BarLayout::Stacked;
        self
    }

    /// Names the x values 0, 1, 2…; use with [`indexed`].
    pub fn categories<S: Into<String>>(mut self, names: impl IntoIterator<Item = S>) -> Chart {
        self.categories = names.into_iter().map(Into::into).collect();
        self
    }

    /// Candles in green and red.
    pub fn market(mut self) -> Chart {
        self.market = true;
        self
    }

    /// The category named at `x`, when the chart has categories and `x` is one of them.
    pub fn category(&self, x: f64) -> Option<&str> {
        if x.fract() != 0.0 || x < 0.0 {
            return None;
        }
        self.categories.get(x as usize).map(String::as_str)
    }

    /// Bar layers in the main panel, by layer index.
    fn main_bars(&self) -> impl Iterator<Item = (usize, &[(f64, f64)])> {
        self.layers.iter().enumerate().filter_map(|(i, l)| match (&l.mark, l.panel) {
            (Mark::Bars { points, .. }, Panel::Main) => Some((i, &points[..])),
            _ => None,
        })
    }

    /// For a stacked chart, every main-panel bar layer's segments, point by point; empty for the other
    /// layers, and for charts that are not stacked.
    pub(crate) fn stacks(&self) -> Vec<Vec<Segment>> {
        let mut out = vec![Vec::new(); self.layers.len()];
        if self.bar_layout != BarLayout::Stacked {
            return out;
        }
        let mut sums: HashMap<u64, (f64, f64)> = HashMap::new();
        let mut last: HashMap<(u64, bool), (usize, usize)> = HashMap::new();
        for (i, points) in self.main_bars() {
            for (j, &(x, v)) in points.iter().enumerate() {
                let sum = sums.entry(x.to_bits()).or_insert((0.0, 0.0));
                let up = v >= 0.0;
                let side = if up { &mut sum.0 } else { &mut sum.1 };
                let base = *side;
                *side += v;
                out[i].push(Segment { base, top: *side, outer: false });
                if v != 0.0 {
                    last.insert((x.to_bits(), up), (i, j));
                }
            }
        }
        for (i, j) in last.into_values() {
            out[i][j].outer = true;
        }
        out
    }

    pub(crate) fn place<'a>(&self, layer: usize, stacks: &'a [Vec<Segment>]) -> Place<'a> {
        let stack = stacks.get(layer).filter(|s| !s.is_empty()).map(|s| &s[..]);
        if self.bar_layout != BarLayout::Grouped || self.layers[layer].panel != Panel::Main {
            return Place { slot: 0, of: 1, stack };
        }
        let group: Vec<usize> = self.main_bars().map(|(i, _)| i).collect();
        let slot = group.iter().position(|i| *i == layer).unwrap_or(0);
        Place { slot, of: group.len().max(1), stack }
    }
}

/// The vertical range of a stacked layer's segments whose x falls inside `x`.
pub(crate) fn stack_bounds(mark: &Mark, stack: &[Segment], x: (f64, f64)) -> Option<(f64, f64)> {
    let Mark::Bars { points, .. } = mark else { return None };
    let slot = mark.spacing();
    points
        .iter()
        .zip(stack)
        .filter(|(p, _)| p.0 >= x.0 - slot && p.0 <= x.1 + slot)
        .fold(None, |acc: Option<(f64, f64)>, (_, s)| {
            let (a, b) = (s.base.min(s.top), s.base.max(s.top));
            Some(acc.map_or((a, b), |(lo, hi)| (lo.min(a), hi.max(b))))
        })
}

/// How many pixels one data unit of x takes.
pub(crate) fn per_unit(x: &Linear) -> f32 {
    let span = (x.domain.1 - x.domain.0).abs();
    if span == 0.0 { 1.0 } else { ((x.range.1 - x.range.0).abs() as f64 / span) as f32 }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_bars(
    canvas: &mut Canvas,
    points: &[(f64, f64)],
    all: &[(f64, f64)],
    spacing: f64,
    base: f64,
    place: &Place,
    x: &Linear,
    y: &Linear,
    color: Color,
) {
    let slot = spacing as f32 * per_unit(x);
    // Too dense to tell apart: keep the extremes, which is what the eye would see anyway.
    if slot < 1.5 && place.stack.is_none() && place.of == 1 {
        let thin = extremes(points, x);
        return draw_bars(canvas, &thin, &thin, spacing, base, &Place { stack: None, ..*place }, x, y, color);
    }
    let (width, offset) = if place.of > 1 {
        let group = (slot * 0.8).min(40.0 * place.of as f32);
        let gap = if group / place.of as f32 > 8.0 { 2.0 } else { 0.0 };
        let width = ((group - gap * (place.of - 1) as f32) / place.of as f32).clamp(1.0, 40.0);
        let k = place.slot as f32 - (place.of - 1) as f32 / 2.0;
        (width, k * (width + gap))
    } else {
        ((slot * 0.64).clamp(1.0, 40.0), 0.0)
    };
    let radius = if width >= 5.0 { 3.0f32.min(width / 4.0) } else { 0.0 };
    // A stacked layer draws a part of `all`: line its segments up with the points it was given.
    let first = all.partition_point(|p| p.0 < points.first().map_or(0.0, |p| p.0));
    for (j, &(px, v)) in points.iter().enumerate() {
        let segment = place.stack.and_then(|s| s.get(first + j));
        let (from, to, round) = match segment {
            Some(s) => (s.base, s.top, s.outer),
            None => (base, v, true),
        };
        let (a, b) = (y.map(from), y.map(to));
        let (mut y0, mut y1) = (a.min(b), a.max(b));
        // Stacked segments are parted by a hairline gap, so neighbours of one colour stay apart.
        if segment.is_some() && y1 - y0 > 3.0 && !round {
            if to >= from { y0 += 1.0 } else { y1 -= 1.0 }
        }
        let r = if round { radius } else { 0.0 };
        let corners = if to >= from {
            Corners { tl: r, tr: r, br: 0.0, bl: 0.0 }
        } else {
            Corners { tl: 0.0, tr: 0.0, br: r, bl: r }
        };
        let cx = x.map(px) + offset;
        canvas.quad(Quad::new(rect(cx - width / 2.0, y0, width, (y1 - y0).max(1.0))).fill(color).radii(corners));
    }
}
