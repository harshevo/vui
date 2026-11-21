//! Charts: a list of layers over shared axes. Describe the data here; [`draw`] turns it into scene
//! commands and [`probe`] answers what is under a pointer. Nothing in this module knows about windows.

mod bars;
mod decimate;
mod draw;
mod probe;
mod widget;

pub use bars::{BarLayout, indexed};
pub use decimate::extremes;
pub use draw::{Drawn, View, draw, layout};
pub use probe::{Probe, Row, draw_probe, probe};
pub use widget::{ChartView, chart};

use crate::format::Format;
use crate::scale::tick_step;
use crate::style::Ink;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Candle {
    pub x: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Dot {
    pub x: f64,
    pub y: f64,
    /// Any non-negative number; dots are sized by their share of the largest.
    pub size: f64,
}

/// A point with a name: a dot sized like [`Dot`] with its label beside it, put where it does not cover
/// another label or leave the plot. With [`Tag::note`] it is only the words, centred on the point and
/// kept inside the plot: a caption for a corner or a region.
#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
    pub x: f64,
    pub y: f64,
    /// Any non-negative number; dots are sized by their share of the largest.
    pub size: f64,
    pub label: String,
    pub dot: bool,
}

impl Tag {
    pub fn new(x: f64, y: f64, label: &str) -> Tag {
        Tag {
            x,
            y,
            size: 1.0,
            label: label.to_string(),
            dot: true,
        }
    }

    pub fn size(mut self, size: f64) -> Tag {
        self.size = if size.is_finite() { size.max(0.0) } else { 0.0 };
        self
    }

    /// Words without a dot.
    pub fn note(mut self) -> Tag {
        self.dot = false;
        self
    }
}

/// A box in data space: from `x0` to `x1` (which may be infinite, to run off the right edge) and
/// from `low` to `high`. A zone with `low == high` is a level: a line across that stretch.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Zone {
    pub x0: f64,
    pub x1: f64,
    pub low: f64,
    pub high: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    Line {
        points: Vec<(f64, f64)>,
        step: bool,
        dashed: bool,
    },
    Area {
        points: Vec<(f64, f64)>,
        base: f64,
    },
    Bars {
        points: Vec<(f64, f64)>,
        base: f64,
    },
    Dots(Vec<Dot>),
    /// Labelled points.
    Tags(Vec<Tag>),
    /// The stretch between a low and a high at each x: a normal range, a fan of outcomes.
    Band {
        /// `(x, low, high)`.
        rows: Vec<(f64, f64, f64)>,
    },
    Candles(Vec<Candle>),
    /// Boxes and levels over the data, such as price zones on a market chart.
    Zones(Vec<Zone>),
}

/// Which strip of the chart a layer sits in.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Panel {
    Main,
    /// A short strip under the main one with its own vertical scale, for volume.
    Strip,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub name: String,
    pub mark: Mark,
    pub ink: Ink,
    pub panel: Panel,
    pub format: Option<Format>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    pub layers: Vec<Layer>,
    /// The horizontal axis counts seconds since 1970 and is labelled with dates.
    pub time: bool,
    pub y_format: Format,
    /// Keep zero on the vertical axis even when the data stays far from it.
    pub zero: bool,
    /// How bar layers in the main panel share each x.
    pub bar_layout: BarLayout,
    /// Names for x = 0, 1, 2…: the horizontal axis and the probe show these instead of numbers.
    pub categories: Vec<String>,
    /// Candles rise in the theme's green and fall in its red, instead of hollow and solid ink.
    pub market: bool,
    /// A vertical range to show whatever the data does: 0 to 1 for shares, a square for a scatter.
    pub y_fixed: Option<(f64, f64)>,
    pub x_fixed: Option<(f64, f64)>,
    /// What the axes measure, written at the top left and the bottom right; empty for nothing.
    pub x_title: String,
    pub y_title: String,
}

impl Default for Chart {
    fn default() -> Chart {
        Chart::new()
    }
}

impl Chart {
    pub fn new() -> Chart {
        Chart {
            layers: Vec::new(),
            time: false,
            y_format: Format::Auto,
            zero: false,
            bar_layout: BarLayout::Overlap,
            categories: Vec::new(),
            market: false,
            y_fixed: None,
            x_fixed: None,
            x_title: String::new(),
            y_title: String::new(),
        }
    }

    pub fn time(mut self) -> Chart {
        self.time = true;
        self
    }

    pub fn y_format(mut self, format: Format) -> Chart {
        self.y_format = format;
        self
    }

    pub fn from_zero(mut self) -> Chart {
        self.zero = true;
        self
    }

    fn push(mut self, name: &str, mark: Mark, ink: Ink, panel: Panel) -> Chart {
        self.layers.push(Layer {
            name: name.to_string(),
            mark,
            ink,
            panel,
            format: None,
        });
        self
    }

    pub fn line(self, name: &str, points: &[(f64, f64)]) -> Chart {
        self.push(
            name,
            Mark::Line {
                points: clean(points),
                step: false,
                dashed: false,
            },
            Ink::Auto,
            Panel::Main,
        )
    }

    pub fn step(self, name: &str, points: &[(f64, f64)]) -> Chart {
        self.push(
            name,
            Mark::Line {
                points: clean(points),
                step: true,
                dashed: false,
            },
            Ink::Auto,
            Panel::Main,
        )
    }

    pub fn area(self, name: &str, points: &[(f64, f64)]) -> Chart {
        self.push(
            name,
            Mark::Area {
                points: clean(points),
                base: 0.0,
            },
            Ink::Auto,
            Panel::Main,
        )
    }

    pub fn bars(self, name: &str, points: &[(f64, f64)]) -> Chart {
        self.push(
            name,
            Mark::Bars {
                points: clean(points),
                base: 0.0,
            },
            Ink::Auto,
            Panel::Main,
        )
    }

    /// Bars in the strip under the chart, in a quiet ink.
    pub fn volume(self, name: &str, points: &[(f64, f64)]) -> Chart {
        self.push(
            name,
            Mark::Bars {
                points: clean(points),
                base: 0.0,
            },
            Ink::Soft,
            Panel::Strip,
        )
    }

    pub fn dots(self, name: &str, dots: &[Dot]) -> Chart {
        let mut dots: Vec<Dot> = dots
            .iter()
            .copied()
            .filter(|d| d.x.is_finite() && d.y.is_finite())
            .collect();
        dots.sort_by(|a, b| a.x.total_cmp(&b.x));
        self.push(name, Mark::Dots(dots), Ink::Auto, Panel::Main)
    }

    /// Labelled points, such as stocks on a scatter. Points that are not numbers are left out.
    pub fn tags(self, name: &str, tags: &[Tag]) -> Chart {
        let mut tags: Vec<Tag> = tags
            .iter()
            .filter(|t| t.x.is_finite() && t.y.is_finite())
            .cloned()
            .collect();
        tags.sort_by(|a, b| a.x.total_cmp(&b.x));
        self.push(name, Mark::Tags(tags), Ink::Auto, Panel::Main)
    }

    /// The stretch from `low` to `high` at each x, as `(x, low, high)`; the ends may be given either
    /// way round. It reads as a wash with faint edges, so lines drawn over it stay clear.
    pub fn band(self, name: &str, rows: &[(f64, f64, f64)]) -> Chart {
        let mut rows: Vec<(f64, f64, f64)> = rows
            .iter()
            .filter(|r| r.0.is_finite() && r.1.is_finite() && r.2.is_finite())
            .map(|r| (r.0, r.1.min(r.2), r.1.max(r.2)))
            .collect();
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        self.push(name, Mark::Band { rows }, Ink::Auto, Panel::Main)
    }

    /// Shows the main panel's vertical range as `low` to `high`, whatever the data does.
    pub fn y_range(mut self, low: f64, high: f64) -> Chart {
        if low.is_finite() && high.is_finite() && low != high {
            self.y_fixed = Some((low.min(high), low.max(high)));
        }
        self
    }

    /// Says what the horizontal axis measures, under its right end.
    pub fn x_title(mut self, title: &str) -> Chart {
        self.x_title = title.to_string();
        self
    }

    /// Says what the vertical axis measures, over its top end.
    pub fn y_title(mut self, title: &str) -> Chart {
        self.y_title = title.to_string();
        self
    }

    /// Shows the horizontal range as `low` to `high`, whatever the data does.
    pub fn x_range(mut self, low: f64, high: f64) -> Chart {
        if low.is_finite() && high.is_finite() && low != high {
            self.x_fixed = Some((low.min(high), low.max(high)));
        }
        self
    }

    pub fn candles(self, name: &str, rows: &[Candle]) -> Chart {
        let mut rows: Vec<Candle> = rows
            .iter()
            .copied()
            .filter(|c| {
                [c.x, c.open, c.high, c.low, c.close]
                    .iter()
                    .all(|v| v.is_finite())
            })
            .collect();
        rows.sort_by(|a, b| a.x.total_cmp(&b.x));
        self.push(name, Mark::Candles(rows), Ink::Strong, Panel::Main)
    }

    /// Boxes (or, with `low == high`, levels) behind nothing and over the grid, washed in the
    /// layer's ink with an outline. They count towards the vertical range where they are in view.
    pub fn zones(self, name: &str, zones: &[Zone]) -> Chart {
        let mut zones: Vec<Zone> = zones
            .iter()
            .copied()
            .filter(|z| {
                z.x0.is_finite() && !z.x1.is_nan() && z.low.is_finite() && z.high.is_finite()
            })
            .map(|z| Zone {
                low: z.low.min(z.high),
                high: z.low.max(z.high),
                ..z
            })
            .collect();
        zones.sort_by(|a, b| a.x0.total_cmp(&b.x0));
        self.push(name, Mark::Zones(zones), Ink::Accent, Panel::Main)
    }

    /// Sets how the most recently added layer is painted.
    pub fn ink(mut self, ink: Ink) -> Chart {
        if let Some(l) = self.layers.last_mut() {
            l.ink = ink;
        }
        self
    }

    pub fn dashed(mut self) -> Chart {
        if let Some(Layer {
            mark: Mark::Line { dashed, .. },
            ..
        }) = self.layers.last_mut()
        {
            *dashed = true;
        }
        self
    }

    /// Sets how the most recently added layer's values are written in the probe.
    pub fn format(mut self, format: Format) -> Chart {
        if let Some(l) = self.layers.last_mut() {
            l.format = Some(format);
        }
        self
    }

    pub fn has_strip(&self) -> bool {
        self.layers.iter().any(|l| l.panel == Panel::Strip)
    }

    /// The horizontal range of all the data, widened by half a bar or candle at each end (and a little for
    /// dots) so those marks are not cut in half.
    pub fn x_extent(&self) -> (f64, f64) {
        if let Some(fixed) = self.x_fixed {
            return fixed;
        }
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        let mut dotted = Vec::new();
        for l in &self.layers {
            if let Some((a, b)) = l.mark.x_bounds() {
                if l.mark.is_thick() {
                    let pad = l.mark.spacing() / 2.0;
                    lo = lo.min(a - pad);
                    hi = hi.max(b + pad);
                } else {
                    if matches!(l.mark, Mark::Dots(_) | Mark::Tags(_)) {
                        dotted.push((a, b));
                    }
                    lo = lo.min(a);
                    hi = hi.max(b);
                }
            }
        }
        // A dot at the edge of the data would be cut in half: leave a margin, a fraction of the whole.
        if lo <= hi {
            let pad = (hi - lo) * 0.04;
            for (a, b) in dotted {
                lo = lo.min(a - pad);
                hi = hi.max(b + pad);
            }
        }
        if lo > hi {
            (0.0, 1.0)
        } else if lo == hi {
            (lo - 1.0, hi + 1.0)
        } else {
            (lo, hi)
        }
    }

    /// The narrowest window worth zooming to: a few of the finest layer's steps.
    pub fn min_span(&self) -> f64 {
        let finest = self
            .layers
            .iter()
            .map(|l| l.mark.spacing())
            .fold(f64::INFINITY, f64::min);
        if finest.is_finite() {
            finest * 4.0
        } else {
            1.0
        }
    }

    /// The vertical range a panel needs to show every mark whose x falls inside `x`, with a little
    /// room above and below and round numbers at the ends.
    pub fn y_extent(&self, panel: Panel, x: (f64, f64), ticks: usize) -> (f64, f64) {
        if let Some(fixed) = self.y_fixed.filter(|_| panel == Panel::Main) {
            return fixed;
        }
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        let mut anchored = self.zero && panel == Panel::Main;
        let stacks = self.stacks();
        for (i, l) in self
            .layers
            .iter()
            .enumerate()
            .filter(|(_, l)| l.panel == panel)
        {
            let bounds = match stacks.get(i).filter(|s| !s.is_empty()) {
                Some(stack) => bars::stack_bounds(&l.mark, stack, x),
                None => l.mark.y_bounds(x),
            };
            if let Some((a, b)) = bounds {
                lo = lo.min(a);
                hi = hi.max(b);
            }
            if let Some(base) = l.mark.base() {
                lo = lo.min(base);
                hi = hi.max(base);
                anchored = true;
            }
        }
        if lo > hi {
            return (0.0, 1.0);
        }
        if lo == hi {
            let room = if lo == 0.0 { 1.0 } else { lo.abs() * 0.05 };
            lo -= room;
            hi += room;
        }
        let pad = (hi - lo) * 0.06;
        let top = hi + pad;
        let bottom = if anchored && lo >= 0.0 {
            lo.min(0.0)
        } else {
            lo - pad
        };
        let step = tick_step(bottom, top, ticks);
        if step > 0.0 && step.is_finite() {
            ((bottom / step).floor() * step, (top / step).ceil() * step)
        } else {
            (bottom, top)
        }
    }
}

impl Mark {
    /// Marks with a width in data units: they need room either side of their x.
    fn is_thick(&self) -> bool {
        matches!(self, Mark::Bars { .. } | Mark::Candles(_))
    }

    fn base(&self) -> Option<f64> {
        match self {
            Mark::Area { base, .. } | Mark::Bars { base, .. } => Some(*base),
            _ => None,
        }
    }

    pub(crate) fn xs(&self) -> Vec<f64> {
        match self {
            Mark::Line { points, .. } | Mark::Area { points, .. } | Mark::Bars { points, .. } => {
                points.iter().map(|p| p.0).collect()
            }
            Mark::Dots(d) => d.iter().map(|d| d.x).collect(),
            Mark::Tags(t) => t.iter().map(|t| t.x).collect(),
            Mark::Band { rows } => rows.iter().map(|r| r.0).collect(),
            Mark::Candles(c) => c.iter().map(|c| c.x).collect(),
            Mark::Zones(z) => z.iter().map(|z| z.x0).collect(),
        }
    }

    pub(crate) fn x_bounds(&self) -> Option<(f64, f64)> {
        let (first, last) = match self {
            Mark::Line { points, .. } | Mark::Area { points, .. } | Mark::Bars { points, .. } => {
                (points.first()?.0, points.last()?.0)
            }
            Mark::Dots(d) => (d.first()?.x, d.last()?.x),
            Mark::Tags(t) => (t.first()?.x, t.last()?.x),
            Mark::Band { rows } => (rows.first()?.0, rows.last()?.0),
            Mark::Candles(c) => (c.first()?.x, c.last()?.x),
            Mark::Zones(z) => {
                let last = z
                    .iter()
                    .map(|z| if z.x1.is_finite() { z.x1 } else { z.x0 })
                    .fold(f64::NEG_INFINITY, f64::max);
                (z.first()?.x0, last)
            }
        };
        Some((first, last))
    }

    /// The smallest gap between neighbouring x values: how wide a bar or candle may be.
    pub(crate) fn spacing(&self) -> f64 {
        // Zones have no step of their own; they leave zooming to the data under them.
        if matches!(self, Mark::Zones(_)) {
            return f64::INFINITY;
        }
        let xs = self.xs();
        let gap = xs
            .windows(2)
            .map(|w| w[1] - w[0])
            .filter(|g| *g > 0.0)
            .fold(f64::INFINITY, f64::min);
        if gap.is_finite() { gap } else { 1.0 }
    }

    /// The vertical range of the marks that fall inside `x`, counting the one just outside each side so
    /// a line entering the view is not cut short.
    pub(crate) fn y_bounds(&self, x: (f64, f64)) -> Option<(f64, f64)> {
        let range = |ys: &mut dyn Iterator<Item = f64>| {
            ys.fold(None, |acc: Option<(f64, f64)>, y| {
                Some(acc.map_or((y, y), |(a, b)| (a.min(y), b.max(y))))
            })
        };
        match self {
            Mark::Line { points, .. } | Mark::Area { points, .. } | Mark::Bars { points, .. } => {
                range(&mut within(points, |p| p.0, x).iter().map(|p| p.1))
            }
            Mark::Dots(d) => range(&mut within(d, |d| d.x, x).iter().map(|d| d.y)),
            Mark::Tags(t) => range(&mut within(t, |t| t.x, x).iter().map(|t| t.y)),
            Mark::Band { rows } => {
                range(&mut within(rows, |r| r.0, x).iter().flat_map(|r| [r.1, r.2]))
            }
            Mark::Candles(c) => {
                range(&mut within(c, |c| c.x, x).iter().flat_map(|c| [c.high, c.low]))
            }
            Mark::Zones(z) => range(
                &mut z
                    .iter()
                    .filter(|z| z.x0 <= x.1 && z.x1 >= x.0)
                    .flat_map(|z| [z.low, z.high]),
            ),
        }
    }
}

/// The items of an x-sorted slice whose key falls inside `x`, plus one neighbour either side.
pub(crate) fn within<T>(items: &[T], key: impl Fn(&T) -> f64, x: (f64, f64)) -> &[T] {
    let start = items.partition_point(|t| key(t) < x.0).saturating_sub(1);
    let end = (items.partition_point(|t| key(t) <= x.1) + 1).min(items.len());
    &items[start.min(end)..end]
}

/// Drops points that are not numbers and puts the rest in x order, which every mark relies on.
fn clean(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut points: Vec<(f64, f64)> = points
        .iter()
        .copied()
        .filter(|p| p.0.is_finite() && p.1.is_finite())
        .collect();
    if !points.windows(2).all(|w| w[0].0 <= w[1].0) {
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    points
}

#[cfg(test)]
mod tests;
